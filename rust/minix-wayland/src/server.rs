//! # Wayland Server — Connection Management and Message Dispatch
//!
//! The Wayland server listens for client connections, manages protocol
//! objects per connection, dispatches incoming requests to the appropriate
//! handler, and sends events back to clients.
//!
//! ## Architecture
//!
//! ```text
//! WaylandServer
//!   ├── connections: Vec<ClientConnection>
//!   │     └── each connection has:
//!   │         ├── transport (Unix socket / MINIX IPC)
//!   │         ├── registry (ObjectRegistry — per-connection)
//!   │         └── pending_events (events to send)
//!   ├── globals: GlobalRegistry (shared across connections)
//!   └── compositor: &mut Compositor (minix-compositor)
//! ```

use alloc::rc::Rc;
use alloc::vec::Vec;
use alloc::boxed::Box;
use core::cell::RefCell;

use minix_compositor::compositor::Compositor;
use minix_compositor::surface::Surface as McSurface;
use minix_input::InputEvent;

use crate::protocol::*;
use crate::shell::Shell;
use crate::tiling::TilingLayout;
use crate::floating::FloatingManager;
use crate::workspace::WorkspaceManager;
use crate::decorator::Decorator;
use crate::keybindings::{KeyBindings, ModMask, KeyAction};
use crate::panel::{Panel, PanelAction};
use crate::transport::{self, WaylandTransport};
use crate::wire::{WaylandMessage, Arg, ArgType, Fixed};

/// Error type for server operations.
#[derive(Debug)]
pub enum ServerError {
    TransportError(transport::TransportError),
    UnknownObject(u32),
    UnknownRequest { object_id: u32, interface: alloc::string::String, opcode: u16 },
    ProtocolError { object_id: u32, code: u32, message: &'static str },
    NoSuchGlobal(u32),
    Internal(&'static str),
}

/// A connected Wayland client.
pub struct ClientConnection {
    /// The transport for this connection.
    pub transport: Box<dyn WaylandTransport>,
    /// Per-connection object registry.
    pub registry: ObjectRegistry,
    /// Pending events to send to this client.
    pending_events: Vec<WaylandMessage>,
    /// Whether this connection is alive.
    pub alive: bool,
    /// Index in WaylandServer::connections (set on accept).
    pub conn_idx: usize,
}

impl ClientConnection {
    pub fn new(transport: Box<dyn WaylandTransport>, conn_idx: usize) -> Self {
        let mut registry = ObjectRegistry::new();
        // wl_registry is per-connection
        registry.register_with_id(WL_REGISTRY_ID, iface::WL_REGISTRY, 1);

        Self {
            transport,
            registry,
            pending_events: Vec::new(),
            alive: true,
            conn_idx,
        }
    }

    /// Queue an event to be sent to this client.
    pub fn send_event(&mut self, msg: WaylandMessage) {
        self.pending_events.push(msg);
    }

    /// Flush all pending events to the transport.
    pub fn flush(&mut self) -> Result<(), ServerError> {
        for event in self.pending_events.drain(..) {
            self.transport.send(&event)
                .map_err(ServerError::TransportError)?;
        }
        self.transport.flush().map_err(ServerError::TransportError)
    }

    /// Read the next request from this client.
    pub fn read_request(&mut self) -> Result<WaylandMessage, ServerError> {
        self.transport.receive().map_err(ServerError::TransportError)
    }

    /// Check if there are pending requests from this client.
    pub fn has_request(&self) -> bool {
        self.transport.has_pending()
    }

    /// Destroy an object (and all child objects) for this client.
    pub fn destroy_object(&mut self, id: u32) {
        self.registry.remove(id);
    }
}

// ════════════════════════════════════════════════════════════════
// Request Handlers
// ════════════════════════════════════════════════════════════════

/// Trait for handling Wayland requests.
///
/// Each protocol interface implements this trait to handle incoming
/// requests from clients.
pub trait RequestHandler {
    /// Handle a request on the given object.
    fn handle_request(
        &mut self,
        conn: &mut ClientConnection,
        msg: &WaylandMessage,
    ) -> Result<(), ServerError>;
}

/// Dispatch a Wayland message to the appropriate handler.
///
/// Looks up the object in the connection's registry, identifies its
/// interface, and dispatches to the right handler function.
pub fn dispatch_message(
    conn: &mut ClientConnection,
    msg: &WaylandMessage,
    handlers: &mut RequestDispatcher,
) -> Result<(), ServerError> {
    // Clone the interface name before dispatch to avoid borrow conflict.
    // `get()` borrows conn.registry immutably, dispatch needs conn mutably.
    let interface: alloc::string::String;
    let opcode: u16;
    {
        let obj = conn.registry.get(msg.object_id)
            .ok_or(ServerError::UnknownObject(msg.object_id))?;
        interface = obj.interface.clone();
        opcode = msg.opcode;
    }

    handlers.dispatch(conn, msg, &interface, opcode)
}

/// Central dispatcher that holds all protocol handlers.
pub struct RequestDispatcher {
    display_handler: Option<Box<dyn FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError>>>,
    registry_handler: Option<Box<dyn FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError>>>,
    compositor_handler: Option<Box<dyn FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError>>>,
    surface_handler: Option<Box<dyn FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError>>>,
    region_handler: Option<Box<dyn FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError>>>,
    seat_handler: Option<Box<dyn FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError>>>,
    pointer_handler: Option<Box<dyn FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError>>>,
    keyboard_handler: Option<Box<dyn FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError>>>,
    xdg_wm_base_handler: Option<Box<dyn FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError>>>,
    xdg_surface_handler: Option<Box<dyn FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError>>>,
    xdg_toplevel_handler: Option<Box<dyn FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError>>>,
    xdg_popup_handler: Option<Box<dyn FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError>>>,
    xdg_positioner_handler: Option<Box<dyn FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError>>>,
    shm_handler: Option<Box<dyn FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError>>>,
    buffer_handler: Option<Box<dyn FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError>>>,
    data_device_manager_handler: Option<Box<dyn FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError>>>,
    data_device_handler: Option<Box<dyn FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError>>>,
    data_source_handler: Option<Box<dyn FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError>>>,
    data_offer_handler: Option<Box<dyn FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError>>>,
}

impl RequestDispatcher {
    pub fn new() -> Self {
        Self {
            display_handler: None,
            registry_handler: None,
            compositor_handler: None,
            surface_handler: None,
            region_handler: None,
            seat_handler: None,
            pointer_handler: None,
            keyboard_handler: None,
            xdg_wm_base_handler: None,
            xdg_surface_handler: None,
            xdg_toplevel_handler: None,
            xdg_popup_handler: None,
            xdg_positioner_handler: None,
            shm_handler: None,
            buffer_handler: None,
            data_device_manager_handler: None,
            data_device_handler: None,
            data_source_handler: None,
            data_offer_handler: None,
        }
    }

    pub fn on_display(&mut self, f: impl FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError> + 'static) {
        self.display_handler = Some(Box::new(f));
    }

    pub fn on_registry(&mut self, f: impl FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError> + 'static) {
        self.registry_handler = Some(Box::new(f));
    }

    pub fn on_compositor(&mut self, f: impl FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError> + 'static) {
        self.compositor_handler = Some(Box::new(f));
    }

    pub fn on_surface(&mut self, f: impl FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError> + 'static) {
        self.surface_handler = Some(Box::new(f));
    }

    pub fn on_region(&mut self, f: impl FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError> + 'static) {
        self.region_handler = Some(Box::new(f));
    }

    pub fn on_seat(&mut self, f: impl FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError> + 'static) {
        self.seat_handler = Some(Box::new(f));
    }

    pub fn on_pointer(&mut self, f: impl FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError> + 'static) {
        self.pointer_handler = Some(Box::new(f));
    }

    pub fn on_keyboard(&mut self, f: impl FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError> + 'static) {
        self.keyboard_handler = Some(Box::new(f));
    }

    pub fn on_xdg_wm_base(&mut self, f: impl FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError> + 'static) {
        self.xdg_wm_base_handler = Some(Box::new(f));
    }

    pub fn on_xdg_surface(&mut self, f: impl FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError> + 'static) {
        self.xdg_surface_handler = Some(Box::new(f));
    }

    pub fn on_xdg_toplevel(&mut self, f: impl FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError> + 'static) {
        self.xdg_toplevel_handler = Some(Box::new(f));
    }

    pub fn on_xdg_popup(&mut self, f: impl FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError> + 'static) {
        self.xdg_popup_handler = Some(Box::new(f));
    }

    pub fn on_xdg_positioner(&mut self, f: impl FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError> + 'static) {
        self.xdg_positioner_handler = Some(Box::new(f));
    }

    pub fn on_shm(&mut self, f: impl FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError> + 'static) {
        self.shm_handler = Some(Box::new(f));
    }

    pub fn on_data_device_manager(&mut self, f: impl FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError> + 'static) {
        self.data_device_manager_handler = Some(Box::new(f));
    }

    pub fn on_data_device(&mut self, f: impl FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError> + 'static) {
        self.data_device_handler = Some(Box::new(f));
    }

    pub fn on_data_source(&mut self, f: impl FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError> + 'static) {
        self.data_source_handler = Some(Box::new(f));
    }

    pub fn on_data_offer(&mut self, f: impl FnMut(&mut ClientConnection, &WaylandMessage) -> Result<(), ServerError> + 'static) {
        self.data_offer_handler = Some(Box::new(f));
    }

    /// Dispatch a message to the appropriate handler.
    fn dispatch(
        &mut self,
        conn: &mut ClientConnection,
        msg: &WaylandMessage,
        interface: &str,
        opcode: u16,
    ) -> Result<(), ServerError> {
        match interface {
            iface::WL_DISPLAY => {
                self.display_handler.as_mut()
                    .ok_or(ServerError::UnknownRequest { object_id: msg.object_id, interface: alloc::string::String::from(interface), opcode })?
                    (conn, msg)
            }
            iface::WL_REGISTRY => {
                self.registry_handler.as_mut()
                    .ok_or(ServerError::UnknownRequest { object_id: msg.object_id, interface: alloc::string::String::from(interface), opcode })?
                    (conn, msg)
            }
            iface::WL_COMPOSITOR => {
                self.compositor_handler.as_mut()
                    .ok_or(ServerError::UnknownRequest { object_id: msg.object_id, interface: alloc::string::String::from(interface), opcode })?
                    (conn, msg)
            }
            iface::WL_SURFACE => {
                self.surface_handler.as_mut()
                    .ok_or(ServerError::UnknownRequest { object_id: msg.object_id, interface: alloc::string::String::from(interface), opcode })?
                    (conn, msg)
            }
            iface::WL_REGION => {
                self.region_handler.as_mut()
                    .ok_or(ServerError::UnknownRequest { object_id: msg.object_id, interface: alloc::string::String::from(interface), opcode })?
                    (conn, msg)
            }
            iface::WL_SEAT => {
                self.seat_handler.as_mut()
                    .ok_or(ServerError::UnknownRequest { object_id: msg.object_id, interface: alloc::string::String::from(interface), opcode })?
                    (conn, msg)
            }
            iface::WL_POINTER => {
                self.pointer_handler.as_mut()
                    .ok_or(ServerError::UnknownRequest { object_id: msg.object_id, interface: alloc::string::String::from(interface), opcode })?
                    (conn, msg)
            }
            iface::WL_KEYBOARD => {
                self.keyboard_handler.as_mut()
                    .ok_or(ServerError::UnknownRequest { object_id: msg.object_id, interface: alloc::string::String::from(interface), opcode })?
                    (conn, msg)
            }
            iface::XDG_WM_BASE => {
                self.xdg_wm_base_handler.as_mut()
                    .ok_or(ServerError::UnknownRequest { object_id: msg.object_id, interface: alloc::string::String::from(interface), opcode })?
                    (conn, msg)
            }
            iface::XDG_SURFACE => {
                self.xdg_surface_handler.as_mut()
                    .ok_or(ServerError::UnknownRequest { object_id: msg.object_id, interface: alloc::string::String::from(interface), opcode })?
                    (conn, msg)
            }
            iface::XDG_TOPLEVEL => {
                self.xdg_toplevel_handler.as_mut()
                    .ok_or(ServerError::UnknownRequest { object_id: msg.object_id, interface: alloc::string::String::from(interface), opcode })?
                    (conn, msg)
            }
            iface::XDG_POPUP => {
                self.xdg_popup_handler.as_mut()
                    .ok_or(ServerError::UnknownRequest { object_id: msg.object_id, interface: alloc::string::String::from(interface), opcode })?
                    (conn, msg)
            }
            iface::XDG_POSITIONER => {
                self.xdg_positioner_handler.as_mut()
                    .ok_or(ServerError::UnknownRequest { object_id: msg.object_id, interface: alloc::string::String::from(interface), opcode })?
                    (conn, msg)
            }
            iface::WL_SHM => {
                self.shm_handler.as_mut()
                    .ok_or(ServerError::UnknownRequest { object_id: msg.object_id, interface: alloc::string::String::from(interface), opcode })?
                    (conn, msg)
            }
            iface::WL_DATA_DEVICE_MANAGER => {
                self.data_device_manager_handler.as_mut()
                    .ok_or(ServerError::UnknownRequest { object_id: msg.object_id, interface: alloc::string::String::from(interface), opcode })?
                    (conn, msg)
            }
            iface::WL_DATA_DEVICE => {
                self.data_device_handler.as_mut()
                    .ok_or(ServerError::UnknownRequest { object_id: msg.object_id, interface: alloc::string::String::from(interface), opcode })?
                    (conn, msg)
            }
            iface::WL_DATA_SOURCE => {
                self.data_source_handler.as_mut()
                    .ok_or(ServerError::UnknownRequest { object_id: msg.object_id, interface: alloc::string::String::from(interface), opcode })?
                    (conn, msg)
            }
            iface::WL_DATA_OFFER => {
                self.data_offer_handler.as_mut()
                    .ok_or(ServerError::UnknownRequest { object_id: msg.object_id, interface: alloc::string::String::from(interface), opcode })?
                    (conn, msg)
            }
            // For interfaces without explicit handlers, return error
            _ => Err(ServerError::UnknownRequest { object_id: msg.object_id, interface: alloc::string::String::from(interface), opcode })
        }
    }
}

// ════════════════════════════════════════════════════════════════
// Wayland Server
// ════════════════════════════════════════════════════════════════

/// The main Wayland compositor server.
///
/// Manages client connections, advertises globals, and dispatches
/// messages to the appropriate handlers. Holds a shared reference
/// to the `minix_compositor::Compositor` via `Rc<RefCell<>>` so
/// that protocol handlers can create and update surfaces.
/// Represents a (connection_index, object_id) pair for tracking focus.
#[derive(Clone)]
pub struct FocusTarget {
    /// Index into WaylandServer::connections.
    pub conn_idx: usize,
    /// The wl_surface object ID that has focus.
    pub surface_id: u32,
}

/// Tracks the current seat state for input handling.
pub struct SeatState {
    /// Cursor position on the output.
    pub pointer_x: i32,
    pub pointer_y: i32,
    /// The surface that currently has pointer focus, if any.
    pub pointer_focus: Option<FocusTarget>,
    /// The client-side serial for the last pointer event.
    pub pointer_serial: u32,
    /// The surface that currently has keyboard focus, if any.
    pub keyboard_focus: Option<FocusTarget>,
    /// The client-side serial for the last keyboard event.
    pub keyboard_serial: u32,
    /// Global input serial counter (incremented on each input event).
    pub serial: u32,
}

impl SeatState {
    pub fn new() -> Self {
        Self {
            pointer_x: 0,
            pointer_y: 0,
            pointer_focus: None,
            pointer_serial: 0,
            keyboard_focus: None,
            keyboard_serial: 0,
            serial: 1,
        }
    }

    /// Get the next input serial.
    pub fn next_serial(&mut self) -> u32 {
        let s = self.serial;
        self.serial = self.serial.wrapping_add(1);
        s
    }
}

/// The main Wayland compositor server.
///
/// Manages client connections, advertises globals, and dispatches
/// messages to the appropriate handlers. Holds a shared reference
/// to the `minix_compositor::Compositor` via `Rc<RefCell<>>` so
/// that protocol handlers can create and update surfaces.
pub struct WaylandServer {
    /// All connected clients.
    pub connections: Vec<ClientConnection>,
    /// Globals advertised to clients.
    pub globals: GlobalRegistry,
    /// Request dispatcher with registered handlers.
    pub dispatcher: RequestDispatcher,
    /// Shared reference to the software compositor.
    pub compositor: Rc<RefCell<Compositor>>,
    /// Seat state (pointer position, focus tracking, serials).
    pub seat_state: SeatState,
    /// Data device state (clipboard selection tracking).
    pub data_device_state: Option<Rc<RefCell<DataDeviceState>>>,
    /// Desktop shell — manages windows, z-order, and window operations.
    pub shell: Rc<RefCell<Shell>>,
    /// Tiling layout engine for window arrangement (None = floating-only mode).
    pub tiling: Option<TilingLayout>,
    /// Floating window manager — drag, resize, snapping (Rc for handler access).
    pub floating: Option<Rc<RefCell<FloatingManager>>>,
    /// Shared cursor position (updated in process_input_event, read from handlers).
    pub cursor_pos: Rc<RefCell<(i32, i32)>>,
    /// Workspace manager — virtual desktops.
    pub workspace: Option<Rc<RefCell<WorkspaceManager>>>,
    /// Window decorations (title bars, buttons).
    pub decorator: Option<Rc<RefCell<Decorator>>>,
    /// Keyboard shortcuts (Lua-конфигурируемые горячие клавиши).
    pub keybindings: Option<KeyBindings>,
    /// System panel / status bar.
    pub panel: Option<Rc<RefCell<Panel>>>,
}

impl WaylandServer {
    /// Create a new Wayland server.
    pub fn new(compositor: Rc<RefCell<Compositor>>) -> Self {
        let mut globals = GlobalRegistry::new();

        // Register built-in globals
        globals.add(iface::WL_COMPOSITOR, 4);
        globals.add(iface::WL_SHM, 1);
        globals.add(iface::WL_SEAT, 5);
        globals.add(iface::WL_DATA_DEVICE_MANAGER, 3);
        globals.add(iface::XDG_WM_BASE, 6);

        let shell = Rc::new(RefCell::new(Shell::new()));

        // Clone compositor BEFORE struct init (field `compositor` moves it)
        let comp_for_panel = compositor.clone();

        Self {
            connections: Vec::new(),
            globals,
            dispatcher: RequestDispatcher::new(),
            compositor,
            seat_state: SeatState::new(),
            data_device_state: None,
            shell,
            tiling: Some(TilingLayout::new(800, 600)),
            floating: Some(Rc::new(RefCell::new(FloatingManager::new(800, 600)))),
            cursor_pos: Rc::new(RefCell::new((0, 0))),
            workspace: Some(Rc::new(RefCell::new(WorkspaceManager::default_workspaces()))),
            decorator: Some(Rc::new(RefCell::new(Decorator::new()))),
            keybindings: Some(KeyBindings::default()),
            panel: {
                let mut comp = comp_for_panel.borrow_mut();
                let names = alloc::vec![
                    alloc::string::String::from("1"),
                    alloc::string::String::from("2"),
                    alloc::string::String::from("3"),
                    alloc::string::String::from("4"),
                ];
                let p = Panel::create(&mut *comp, 800, &names);
                Some(Rc::new(RefCell::new(p)))
            },
        }
    }

    /// Accept a new client connection.
    pub fn accept(&mut self, transport: Box<dyn WaylandTransport>) -> &mut ClientConnection {
        let conn_idx = self.connections.len();
        let mut conn = ClientConnection::new(transport, conn_idx);

        // Send wl_display events to set up the connection
        // wl_registry.global events for each global
        for global in self.globals.all() {
            conn.send_event(WaylandMessage::new(WL_REGISTRY_ID, WlRegistry::GLOBAL)
                .arg_uint(global.name)
                .arg_string(global.interface)
                .arg_uint(global.version));
        }

        self.connections.push(conn);
        self.connections.last_mut().unwrap()
    }

    /// Process one iteration: read messages from all clients and dispatch.
    pub fn tick(&mut self) {
        let mut to_remove = Vec::new();

        for (idx, conn) in self.connections.iter_mut().enumerate() {
            if !conn.alive {
                to_remove.push(idx);
                continue;
            }

            // Read all pending requests
            while conn.has_request() {
                match conn.read_request() {
                    Ok(msg) => {
                        if let Err(e) = dispatch_message(conn, &msg, &mut self.dispatcher) {
                            // On protocol error, send wl_display.error and disconnect
                            conn.send_event(WaylandMessage::new(WL_DISPLAY_ID, WlDisplay::ERROR)
                                .arg_object(msg.object_id)
                                .arg_uint(match &e {
                                    ServerError::ProtocolError { code, .. } => *code,
                                    _ => 1, // generic error
                                })
                                .arg_string(match &e {
                                    ServerError::ProtocolError { message, .. } => message,
                                    _ => "internal error",
                                }));
                            conn.alive = false;
                        }
                    }
                    Err(_) => {
                        conn.alive = false;
                    }
                }
            }

            // Flush pending events
            let _ = conn.flush();
        }

        // Remove dead connections (reverse order)
        for idx in to_remove.into_iter().rev() {
            self.connections.remove(idx);
        }

        // Recalculate tiling layout after processing messages
        self.recalculate_layout();

        // Refresh window decorations (re-render dirty title bars)
        if let Some(ref deco) = self.decorator {
            let windows = self.shell_windows();
            let active = self.seat_state.keyboard_focus.as_ref()
                .and_then(|f| {
                    self.shell.borrow()
                        .find_by_surface(f.surface_id)
                        .map(|w| w.xdg_toplevel_id)
                });
            let mut comp = self.compositor.borrow_mut();
            deco.borrow_mut().refresh(&mut *comp, &windows, active);
        }

        // Refresh panel (re-render status bar)
        if let Some(ref panel) = self.panel {
            let mut p = panel.borrow_mut();
            // Update workspace info from the workspace manager
            if let Some(ref ws) = self.workspace {
                let ws = ws.borrow();
                p.set_active_workspace(ws.current_index());
                // Update window titles for current workspace
                let titles: alloc::vec::Vec<alloc::string::String> = self.shell_windows().iter()
                    .filter(|w| w.visible && ws.current_window_ids().contains(&w.xdg_toplevel_id))
                    .map(|w| w.title.clone())
                    .filter(|t| !t.is_empty())
                    .collect();
                p.set_window_titles(&titles);
            }
            let mut comp = self.compositor.borrow_mut();
            p.render(&mut *comp);
        }
    }

    /// Send an event to all clients.
    pub fn broadcast(&mut self, msg: WaylandMessage) {
        for conn in &mut self.connections {
            conn.send_event(msg.clone());
        }
    }

    /// Tick and broadcast to all clients.
    pub fn tick_and_broadcast(&mut self, frame_msg: Option<WaylandMessage>) {
        self.tick();
        if let Some(msg) = frame_msg {
            self.broadcast(msg);
        }
    }

    /// Recalculate the tiling layout for all visible, non-floating windows
    /// on the current workspace.
    ///
    /// Sorts windows by z-order, assigns master/stack positions,
    /// updates Shell window positions, compositor surface positions,
    /// and sends `xdg_toplevel.configure` events for resized windows.
    /// Floating windows and windows on non-current workspaces are excluded.
    pub fn recalculate_layout(&mut self) {
        if self.tiling.is_none() {
            return;
        }

        let tiling = self.tiling.as_ref().unwrap().clone();

        // Collect visible, non-floating window info sorted by z_order (ascending)
        // Only include windows on the current workspace
        let mut window_data: Vec<(u32, usize, u32, i32, i32, i32)>;
        {
            let shell = self.shell.borrow();
            let floating_ids: Vec<u32> = self.floating.as_ref()
                .map(|fl| fl.borrow().floating_ids().to_vec())
                .unwrap_or_default();
            let current_ws_ids: Vec<u32> = self.workspace.as_ref()
                .map(|ws| ws.borrow().current_window_ids().to_vec())
                .unwrap_or_default();
            let mut sorted: Vec<&crate::shell::WindowInfo> = shell.all_windows()
                .iter()
                .filter(|w| {
                    w.visible
                    && !floating_ids.contains(&w.xdg_toplevel_id)
                    && current_ws_ids.contains(&w.xdg_toplevel_id)
                })
                .collect();
            sorted.sort_by_key(|w| w.z_order);

            window_data = sorted.iter().map(|w| {
                (w.xdg_toplevel_id, w.conn_idx, w.surface_id, w.width, w.height, w.z_order)
            }).collect();
        }

        let count = window_data.len();
        if count == 0 {
            return;
        }

        let layouts = tiling.calculate(count);

        for ((xdg_id, conn_idx, surface_id, old_w, old_h, _z), layout) in
            window_data.iter().zip(layouts.iter())
        {
            // Update shell window position/size
            self.shell.borrow_mut().set_position(*xdg_id, layout.x, layout.y);
            self.shell.borrow_mut().set_size(*xdg_id, layout.width, layout.height);

            // Update compositor surface position
            {
                let mut comp = self.compositor.borrow_mut();
                if let Some(s) = comp.get_surface(*surface_id as u64) {
                    s.x = layout.x;
                    s.y = layout.y;
                    s.mark_dirty();
                }
            }

            // Update decoration position/size
            if let Some(ref deco) = self.decorator {
                let mut d = deco.borrow_mut();
                let mut comp_ref = self.compositor.borrow_mut();
                // Use the window z_order from shell
                let z = self.shell.borrow()
                    .find_by_toplevel(*xdg_id)
                    .map(|w| w.z_order)
                    .unwrap_or(0);
                d.update_deco_position(
                    &mut *comp_ref, *xdg_id,
                    layout.x, layout.y,
                    layout.width as u32, z,
                );
            }

            // Send XdgToplevel::CONFIGURE if size changed
            if *old_w != layout.width || *old_h != layout.height {
                let serial = next_serial();
                if let Some(conn) = self.connections.get_mut(*conn_idx) {
                    conn.send_event(WaylandMessage::new(*xdg_id, XdgToplevel::CONFIGURE)
                        .arg_int(layout.width)
                        .arg_int(layout.height)
                        .arg_array(alloc::vec![]));
                }
            }
        }
    }

    /// Number of connected clients.
    pub fn client_count(&self) -> usize {
        self.connections.len()
    }

    /// Close a window by sending xdg_toplevel.close event to its client.
    ///
    /// Returns true if the window was found and the event was sent.
    pub fn shell_close_window(&mut self, xdg_toplevel_id: u32) -> bool {
        let info = {
            let shell = self.shell.borrow();
            shell.find_by_toplevel(xdg_toplevel_id).map(|w| (w.conn_idx, w.xdg_toplevel_id))
        };

        if let Some((conn_idx, toplevel_id)) = info {
            if let Some(conn) = self.connections.get_mut(conn_idx) {
                conn.send_event(WaylandMessage::new(toplevel_id, XdgToplevel::CLOSE));
                return true;
            }
        }
        false
    }

    /// Raise a window to the top of the z-order.
    ///
    /// Updates the Shell's z-order, the compositor surface's z_order,
    /// and the decoration surface's z_order.
    /// Returns true if the window was found and raised.
    pub fn shell_raise_window(&mut self, xdg_toplevel_id: u32) -> bool {
        let info = {
            let shell = self.shell.borrow();
            shell.find_by_toplevel(xdg_toplevel_id).map(|w| (w.surface_id, w.x, w.y, w.width))
        };

        if let Some((sid, wx, wy, ww)) = info {
            let z = {
                let mut shell = self.shell.borrow_mut();
                shell.raise_window(xdg_toplevel_id)
            };
            // Update the compositor surface's z_order
            let mut comp = self.compositor.borrow_mut();
            if let Some(s) = comp.get_surface(sid as u64) {
                s.z_order = z;
            }
            drop(comp);
            // Update the decoration surface's z_order
            if let Some(ref deco) = self.decorator {
                let mut d = deco.borrow_mut();
                let mut comp = self.compositor.borrow_mut();
                d.update_deco_position(&mut *comp, xdg_toplevel_id, wx, wy, ww as u32, z);
            }
            true
        } else {
            false
        }
    }

    /// Get the number of shell windows.
    pub fn shell_window_count(&self) -> usize {
        self.shell.borrow().window_count()
    }

    /// Get info about all shell windows.
    pub fn shell_windows(&self) -> alloc::vec::Vec<crate::shell::WindowInfo> {
        self.shell.borrow().all_windows().to_vec()
    }

    /// Switch to a different workspace.
    ///
    /// Hides all windows on the old workspace, shows all windows on the
    /// new workspace, recalculates the tiling layout, and clears keyboard
    /// focus (the user must click a window on the new workspace to focus it).
    ///
    /// Returns true if the workspace was switched.
    pub fn switch_workspace(&mut self, index: usize) -> bool {
        let result = {
            let mut ws = match self.workspace.as_ref() {
                Some(ws) => ws.borrow_mut(),
                None => return false,
            };
            ws.switch_to(index)
        };

        let result = match result {
            Some(r) => r,
            None => return false, // same workspace or invalid
        };

        // Hide all windows from the old workspace
        for &win_id in &result.to_hide {
            // Check if window still exists (it may have been destroyed)
            self.shell.borrow_mut().set_visible(win_id, false);
            // Update compositor surface visibility
            let sid = self.shell.borrow().find_by_toplevel(win_id).map(|w| w.surface_id as u64);
            if let Some(sid) = sid {
                let mut comp = self.compositor.borrow_mut();
                if let Some(s) = comp.get_surface(sid) {
                    s.visible = false;
                }
            }
        }

        // Show all windows on the new workspace
        for &win_id in &result.to_show {
            let should_show = self.shell.borrow()
                .find_by_toplevel(win_id)
                .map(|w| !w.minimized)
                .unwrap_or(false);
            if should_show {
                self.shell.borrow_mut().set_visible(win_id, true);
                let sid = self.shell.borrow().find_by_toplevel(win_id).map(|w| w.surface_id as u64);
                if let Some(sid) = sid {
                    let mut comp = self.compositor.borrow_mut();
                    if let Some(s) = comp.get_surface(sid) {
                        s.visible = true;
                    }
                }
            }
        }

        // Clear keyboard focus (user must click to focus a window on new workspace)
        self.seat_state.keyboard_focus = None;
        self.seat_state.pointer_focus = None;

        // Recalculate layout for the new workspace
        self.recalculate_layout();

        true
    }

    /// Switch to the next workspace (wrapping).
    pub fn switch_workspace_next(&mut self) -> bool {
        // Compute current index and workspace count in one scope
        let (cur, count) = match self.workspace.as_ref() {
            Some(ws) => {
                let ws = ws.borrow();
                (ws.current_index(), ws.workspace_count())
            }
            None => return false,
        };
        let next = if cur + 1 >= count { 0 } else { cur + 1 };
        self.switch_workspace(next)
    }

    /// Switch to the previous workspace (wrapping).
    pub fn switch_workspace_prev(&mut self) -> bool {
        let (cur, count) = match self.workspace.as_ref() {
            Some(ws) => {
                let ws = ws.borrow();
                (ws.current_index(), ws.workspace_count())
            }
            None => return false,
        };
        let prev = if cur == 0 { count - 1 } else { cur - 1 };
        self.switch_workspace(prev)
    }
}

// ════════════════════════════════════════════════════════════════
// Default Handlers (built-in protocol implementations)
// ════════════════════════════════════════════════════════════════

/// Set up default handlers for the core Wayland protocol (wl_display, wl_registry).
pub fn setup_default_handlers(server: &mut WaylandServer) {
    // wl_display handler
    server.dispatcher.on_display(move |conn, msg| {
        match msg.opcode {
            WlDisplay::SYNC => {
                // wl_display.sync: decode args, create wl_callback, send done
                let mut msg = msg.clone();
                let _ = msg.decode_args_with(&[ArgType::NewId]);
                let callback_id = msg.args.first()
                    .and_then(|a| if let Arg::NewId(id) = a { Some(*id) } else { None })
                    .unwrap_or(0);
                if callback_id > 0 {
                    conn.registry.register_with_id(callback_id, iface::WL_CALLBACK, 1);
                    conn.send_event(WaylandMessage::new(callback_id, WlCallback::DONE).arg_uint(0));
                }
                Ok(())
            }
            WlDisplay::GET_REGISTRY => {
                Ok(())
            }
            _ => Err(ServerError::UnknownRequest { 
                object_id: msg.object_id, interface: alloc::string::String::from(iface::WL_DISPLAY), opcode: msg.opcode 
            })
        }
    });

    // wl_registry handler
    server.dispatcher.on_registry(move |conn, msg| {
        match msg.opcode {
            WlRegistry::BIND => {
                // wl_registry.bind(name, id, interface, version)
                // Args: name(uint), id(new_id), interface(string), version(uint)
                // The new_id already carries the interface and version from the client
                let mut msg = msg.clone();
                let _ = msg.decode_args_with(&[ArgType::Uint, ArgType::NewId, ArgType::String, ArgType::Uint]);
                // Extract name and interface
                if msg.args.len() >= 3 {
                    let _name = if let Arg::Uint(n) = msg.args[0] { n } else { 0 };
                    let id = if let Arg::NewId(n) = msg.args[1] { n } else { 0 };
                    let iface_name = if let Arg::String(Some(s)) = &msg.args[2] { s.as_str() } else { "" };
                    let _version = if let Arg::Uint(v) = msg.args.get(3).unwrap_or(&Arg::Uint(1)) { *v } else { 1 };

                    if id > 0 && !iface_name.is_empty() {
                        conn.registry.register_with_id(id, iface_name, 1);
                    }
                }
                Ok(())
            }
            _ => Err(ServerError::UnknownRequest { 
                object_id: msg.object_id, interface: alloc::string::String::from(iface::WL_REGISTRY), opcode: msg.opcode 
            })
        }
    });
}

/// Set up compositor protocol handlers.
///
/// Registers handlers for:
/// - wl_compositor: create_surface, create_region
/// - wl_surface: destroy, attach, damage, frame, commit, set_opaque_region,
///   set_input_region, set_buffer_transform, set_buffer_scale, damage_buffer, offset
/// - wl_region: destroy, add, subtract
pub fn setup_compositor_handlers(server: &mut WaylandServer) {
    let comp = server.compositor.clone();

    // wl_compositor handler
    server.dispatcher.on_compositor(move |conn, msg| {
        let mut msg = msg.clone();
        let _ = msg.decode_args_with(&[ArgType::NewId]);
        let new_id = msg.args.first()
            .and_then(|a| if let Arg::NewId(id) = a { Some(*id) } else { None })
            .unwrap_or(0);

        match msg.opcode {
            WlCompositor::CREATE_SURFACE => {
                if new_id == 0 {
                    return Err(ServerError::Internal("invalid new_id for create_surface"));
                }
                // Register as wl_surface in the connection's registry
                conn.registry.register_with_id(new_id, iface::WL_SURFACE, 4);
                Ok(())
            }
            WlCompositor::CREATE_REGION => {
                if new_id == 0 {
                    return Err(ServerError::Internal("invalid new_id for create_region"));
                }
                conn.registry.register_with_id(new_id, iface::WL_REGION, 1);
                Ok(())
            }
            _ => Err(ServerError::UnknownRequest {
                object_id: msg.object_id, interface: alloc::string::String::from(iface::WL_COMPOSITOR), opcode: msg.opcode
            })
        }
    });

    // wl_surface handler
    let comp2 = comp.clone();
    server.dispatcher.on_surface(move |conn, msg| {
        let opcode = msg.opcode;
        let mut msg = msg.clone();
        let mut comp = comp2.borrow_mut();

        match opcode {
            WlSurface::DESTROY => {
                // Remove surface from compositor
                let surface_id = msg.object_id as u64;
                comp.remove_surface(surface_id);
                conn.destroy_object(msg.object_id);
                Ok(())
            }
            WlSurface::ATTACH => {
                // wl_surface.attach(buffer, x, y)
                let _ = msg.decode_args_with(&[ArgType::Object, ArgType::Int, ArgType::Int]);
                let surface_id = msg.object_id as u64;
                // Create surface if it doesn't exist yet
                if comp.get_surface(surface_id).is_none() {
                    let mut surface = McSurface::new(1, 1, 0, 0);
                    surface.id = surface_id; // match Wayland object ID
                    comp.add_surface(surface);
                }
                if let Some(s) = comp.get_surface(surface_id) {
                    s.mark_dirty();
                }
                Ok(())
            }
            WlSurface::DAMAGE => {
                // wl_surface.damage(x, y, w, h)
                let _ = msg.decode_args_with(&[ArgType::Int, ArgType::Int, ArgType::Int, ArgType::Int]);
                let surface_id = msg.object_id as u64;
                if let Some(s) = comp.get_surface(surface_id) {
                    s.mark_dirty();
                }
                Ok(())
            }
            WlSurface::FRAME => {
                // wl_surface.frame(callback)
                let _ = msg.decode_args_with(&[ArgType::NewId]);
                let callback_id = msg.args.first()
                    .and_then(|a| if let Arg::NewId(id) = a { Some(*id) } else { None })
                    .unwrap_or(0);
                if callback_id > 0 {
                    conn.registry.register_with_id(callback_id, iface::WL_CALLBACK, 1);
                    // Send wl_callback.done immediately (synchronous compositor)
                    conn.send_event(WaylandMessage::new(callback_id, WlCallback::DONE).arg_uint(0));
                }
                Ok(())
            }
            WlSurface::COMMIT => {
                // wl_surface.commit — apply pending state, mark dirty
                let surface_id = msg.object_id as u64;
                // Ensure surface exists in compositor with matching ID
                if comp.get_surface(surface_id).is_none() {
                    let mut surface = McSurface::new(1, 1, 0, 0);
                    surface.id = surface_id; // match Wayland object ID
                    comp.add_surface(surface);
                }
                if let Some(s) = comp.get_surface(surface_id) {
                    s.mark_dirty();
                }
                comp.mark_all_dirty();
                Ok(())
            }
            WlSurface::SET_OPAQUE_REGION => {
                // wl_surface.set_opaque_region(region)
                let _ = msg.decode_args_with(&[ArgType::Object]);
                Ok(())
            }
            WlSurface::SET_INPUT_REGION => {
                // wl_surface.set_input_region(region)
                let _ = msg.decode_args_with(&[ArgType::Object]);
                Ok(())
            }
            WlSurface::SET_BUFFER_TRANSFORM => {
                // wl_surface.set_buffer_transform(transform)
                let _ = msg.decode_args_with(&[ArgType::Int]);
                Ok(())
            }
            WlSurface::SET_BUFFER_SCALE => {
                // wl_surface.set_buffer_scale(scale)
                let _ = msg.decode_args_with(&[ArgType::Int]);
                Ok(())
            }
            WlSurface::DAMAGE_BUFFER => {
                // wl_surface.damage_buffer(x, y, w, h)
                let _ = msg.decode_args_with(&[ArgType::Int, ArgType::Int, ArgType::Int, ArgType::Int]);
                let surface_id = msg.object_id as u64;
                if let Some(s) = comp.get_surface(surface_id) {
                    s.mark_dirty();
                }
                Ok(())
            }
            WlSurface::OFFSET => {
                // wl_surface.offset(x, y)
                let _ = msg.decode_args_with(&[ArgType::Int, ArgType::Int]);
                let surface_id = msg.object_id as u64;
                if msg.args.len() >= 2 {
                    let x = if let Arg::Int(v) = msg.args[0] { v } else { 0 };
                    let y = if let Arg::Int(v) = msg.args[1] { v } else { 0 };
                    if let Some(s) = comp.get_surface(surface_id) {
                        s.x = x;
                        s.y = y;
                        s.mark_dirty();
                    }
                }
                Ok(())
            }
            _ => Err(ServerError::UnknownRequest {
                object_id: msg.object_id, interface: alloc::string::String::from(iface::WL_SURFACE), opcode
            })
        }
    });

    // wl_region handler
    server.dispatcher.on_region(move |conn, msg| {
        let mut msg = msg.clone();

        match msg.opcode {
            WlRegion::DESTROY => {
                conn.destroy_object(msg.object_id);
                Ok(())
            }
            WlRegion::ADD => {
                let _ = msg.decode_args_with(&[ArgType::Int, ArgType::Int, ArgType::Int, ArgType::Int]);
                Ok(())
            }
            WlRegion::SUBTRACT => {
                let _ = msg.decode_args_with(&[ArgType::Int, ArgType::Int, ArgType::Int, ArgType::Int]);
                Ok(())
            }
            _ => Err(ServerError::UnknownRequest {
                object_id: msg.object_id, interface: alloc::string::String::from(iface::WL_REGION), opcode: msg.opcode
            })
        }
    });
}

// ── Data structs for xdg_shell state ────────────────────────────────

/// State stored on a wl_surface that has an xdg_surface role.
pub struct WlSurfaceRoleData {
    /// The xdg_surface object ID that wraps this wl_surface (0 if none).
    pub xdg_surface_id: u32,
}

/// State for an xdg_toplevel object.
#[derive(Clone)]
pub struct XdgToplevelData {
    pub title: alloc::string::String,
    pub app_id: alloc::string::String,
    pub min_width: i32,
    pub min_height: i32,
    pub max_width: i32,
    pub max_height: i32,
    pub maximized: bool,
    pub fullscreen: bool,
    pub minimized: bool,
    pub wl_surface_id: u32,
    pub parent: Option<u32>,
}

impl XdgToplevelData {
    pub fn new(wl_surface_id: u32) -> Self {
        Self {
            title: alloc::string::String::new(),
            app_id: alloc::string::String::new(),
            min_width: 0,
            min_height: 0,
            max_width: 0,
            max_height: 0,
            maximized: false,
            fullscreen: false,
            minimized: false,
            wl_surface_id,
            parent: None,
        }
    }
}

/// State for an xdg_positioner object.
pub struct XdgPositionerData {
    pub width: i32,
    pub height: i32,
    pub anchor_rect_x: i32,
    pub anchor_rect_y: i32,
    pub anchor_rect_w: i32,
    pub anchor_rect_h: i32,
    pub anchor: u32,
    pub gravity: u32,
    pub constraint_adjustment: u32,
    pub offset_x: i32,
    pub offset_y: i32,
}

impl XdgPositionerData {
    pub fn new() -> Self {
        Self {
            width: 0, height: 0,
            anchor_rect_x: 0, anchor_rect_y: 0,
            anchor_rect_w: 0, anchor_rect_h: 0,
            anchor: 0, gravity: 0, constraint_adjustment: 0,
            offset_x: 0, offset_y: 0,
        }
    }
}

/// Global configure serial counter.
static CONFIGURE_SERIAL: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(1);

fn next_serial() -> u32 {
    CONFIGURE_SERIAL.fetch_add(1, core::sync::atomic::Ordering::Relaxed)
}

/// Set up xdg_shell protocol handlers.
///
/// Registers handlers for:
/// - xdg_wm_base: destroy, create_positioner, get_xdg_surface, pong
/// - xdg_surface: destroy, get_toplevel, get_popup, set_window_geometry, ack_configure
/// - xdg_toplevel: all 14 requests (title, app_id, min/max, etc.)
/// - xdg_popup: destroy, grab, reposition
/// - xdg_positioner: set_size, set_anchor_rect, set_anchor, set_gravity, etc.
pub fn setup_xdg_shell_handlers(server: &mut WaylandServer) {
    // Clone comp BEFORE any move closures consume it
    let comp = server.compositor.clone();
    let comp2 = comp.clone();  // for xdg_surface
    let comp3 = comp.clone();  // for xdg_toplevel

    // Clone shell for use in closures
    let shell2 = server.shell.clone();  // for xdg_surface
    let shell3 = server.shell.clone();  // for xdg_toplevel

    // Clone floating manager and cursor position for MOVE/RESIZE handlers
    let floating = server.floating.clone();
    let cursor_pos = server.cursor_pos.clone();

    // Clone workspace manager for window lifecycle handlers
    let workspace2 = server.workspace.clone();  // for xdg_surface
    let workspace3 = server.workspace.clone();  // for xdg_toplevel

    // Clone decorator for window decoration lifecycle
    let decorator2 = server.decorator.clone();  // for xdg_surface
    let decorator3 = server.decorator.clone();  // for xdg_toplevel

    // ── xdg_wm_base handler ────────────────────────────────────────
    server.dispatcher.on_xdg_wm_base(move |conn, msg| {
        let mut msg = msg.clone();

        match msg.opcode {
            XdgWmBase::DESTROY => {
                conn.destroy_object(msg.object_id);
                Ok(())
            }
            XdgWmBase::CREATE_POSITIONER => {
                let _ = msg.decode_args_with(&[ArgType::NewId]);
                let new_id = msg.args.first()
                    .and_then(|a| if let Arg::NewId(id) = a { Some(*id) } else { None })
                    .unwrap_or(0);
                if new_id > 0 {
                    conn.registry.register_with_id_and_data(
                        new_id, iface::XDG_POSITIONER, 1,
                        XdgPositionerData::new(),
                    );
                }
                Ok(())
            }
            XdgWmBase::GET_XDG_SURFACE => {
                let _ = msg.decode_args_with(&[ArgType::NewId, ArgType::Object]);
                let new_id = msg.args.first()
                    .and_then(|a| if let Arg::NewId(id) = a { Some(*id) } else { None })
                    .unwrap_or(0);
                let surface_id = msg.args.get(1)
                    .and_then(|a| if let Arg::Object(id) = a { Some(*id) } else { None })
                    .unwrap_or(0);

                if new_id > 0 && surface_id > 0 {
                    conn.registry.register_with_id_and_data(
                        new_id, iface::XDG_SURFACE, 1,
                        surface_id,
                    );
                    let serial = next_serial();
                    conn.send_event(WaylandMessage::new(new_id, XdgSurface::CONFIGURE)
                        .arg_uint(serial));
                }
                Ok(())
            }
            XdgWmBase::PONG => Ok(()),
            _ => Err(ServerError::UnknownRequest {
                object_id: msg.object_id, interface: alloc::string::String::from(iface::XDG_WM_BASE), opcode: msg.opcode
            })
        }
    });

    // ── xdg_surface handler ────────────────────────────────────────
    server.dispatcher.on_xdg_surface(move |conn, msg| {
        let mut msg = msg.clone();
        let mut comp = comp2.borrow_mut();

        match msg.opcode {
            XdgSurface::DESTROY => {
                conn.destroy_object(msg.object_id);
                Ok(())
            }
            XdgSurface::GET_TOPLEVEL => {
                let _ = msg.decode_args_with(&[ArgType::NewId]);
                let new_id = msg.args.first()
                    .and_then(|a| if let Arg::NewId(id) = a { Some(*id) } else { None })
                    .unwrap_or(0);
                let wl_surface_id = conn.registry.get_data::<u32>(msg.object_id)
                    .copied().unwrap_or(0);

                if new_id > 0 {
                    let data = XdgToplevelData::new(wl_surface_id);
                    conn.registry.register_with_id_and_data(
                        new_id, iface::XDG_TOPLEVEL, 1, data,
                    );
                    let surface_id = wl_surface_id as u64;
                    if surface_id > 0 && comp.get_surface(surface_id).is_none() {
                        let mut surface = McSurface::new(800, 600, 0, 0);
                        surface.id = surface_id;
                        comp.add_surface(surface);
                    }

                    // Register window in shell and workspace
                    shell2.borrow_mut().add_window(
                        conn.conn_idx, new_id, wl_surface_id,
                        alloc::string::String::new(),
                        alloc::string::String::new(),
                        800, 600,
                    );
                    if let Some(ref ws) = workspace2 {
                        ws.borrow_mut().add_window(new_id);
                    }

                    // Create window decoration (title bar)
                    if let Some(ref deco) = decorator2 {
                        let xdg_id = new_id;
                        let win_sid = wl_surface_id as u64;
                        deco.borrow_mut().create_deco(
                            &mut *comp, xdg_id, win_sid,
                            0, 0, 800, 0, "", true,
                        );
                    }

                    let serial = next_serial();
                    conn.send_event(WaylandMessage::new(new_id, XdgToplevel::CONFIGURE)
                        .arg_int(800).arg_int(600)
                        .arg_array(alloc::vec![]));
                }
                Ok(())
            }
            XdgSurface::GET_POPUP => {
                let _ = msg.decode_args_with(&[ArgType::NewId, ArgType::Object, ArgType::Object]);
                let new_id = msg.args.first()
                    .and_then(|a| if let Arg::NewId(id) = a { Some(*id) } else { None })
                    .unwrap_or(0);
                if new_id > 0 {
                    conn.registry.register_with_id(new_id, iface::XDG_POPUP, 1);
                }
                Ok(())
            }
            XdgSurface::SET_WINDOW_GEOMETRY => {
                let _ = msg.decode_args_with(&[ArgType::Int, ArgType::Int, ArgType::Int, ArgType::Int]);
                Ok(())
            }
            XdgSurface::ACK_CONFIGURE => {
                let _ = msg.decode_args_with(&[ArgType::Uint]);
                Ok(())
            }
            _ => Err(ServerError::UnknownRequest {
                object_id: msg.object_id, interface: alloc::string::String::from(iface::XDG_SURFACE), opcode: msg.opcode
            })
        }
    });

    // ── xdg_toplevel handler ────────────────────────────────────────
    server.dispatcher.on_xdg_toplevel(move |conn, msg| {
        let mut msg = msg.clone();

        match msg.opcode {
            XdgToplevel::DESTROY => {
                let surface_id = conn.registry.get_data::<XdgToplevelData>(msg.object_id)
                    .map(|d| d.wl_surface_id).unwrap_or(0);
                let mut comp = comp3.borrow_mut();
                if surface_id > 0 {
                    comp.remove_surface(surface_id as u64);
                }
                drop(comp);
                // Remove from shell, floating, workspace, and decorator
                shell3.borrow_mut().remove_window(msg.object_id);
                if let Some(ref fl) = floating {
                    fl.borrow_mut().remove_window(msg.object_id);
                }
                if let Some(ref ws) = workspace3 {
                    ws.borrow_mut().remove_window(msg.object_id);
                }
                if let Some(ref deco) = decorator3 {
                    let mut comp = comp3.borrow_mut();
                    deco.borrow_mut().remove_deco(&mut *comp, msg.object_id);
                }
                conn.destroy_object(msg.object_id);
                Ok(())
            }
            XdgToplevel::SET_PARENT => {
                // xdg_toplevel.set_parent(parent)
                let _ = msg.decode_args_with(&[ArgType::Object]);
                let parent = msg.args.first()
                    .and_then(|a| if let Arg::Object(id) = a { Some(*id) } else { None });
                // Object ID 0 means "no parent" (null)
                let parent = parent.filter(|&id| id > 0);
                if let Some(data) = conn.registry.get_data_mut::<XdgToplevelData>(msg.object_id) {
                    data.parent = parent;
                }
                Ok(())
            }
            XdgToplevel::SET_TITLE => {
                // xdg_toplevel.set_title(title)
                let _ = msg.decode_args_with(&[ArgType::String]);
                let title = msg.args.first()
                    .and_then(|a| if let Arg::String(Some(s)) = a { Some(s.clone()) } else { None });
                if let Some(title) = title {
                    if let Some(data) = conn.registry.get_data_mut::<XdgToplevelData>(msg.object_id) {
                        data.title = title.clone();
                    }
                    shell3.borrow_mut().set_title(msg.object_id, title);
                    // Mark decoration dirty so it re-renders with new title
                    if let Some(ref d) = decorator3 {
                        d.borrow_mut().mark_dirty(msg.object_id);
                    }
                }
                Ok(())
            }
            XdgToplevel::SET_APP_ID => {
                // xdg_toplevel.set_app_id(app_id)
                let _ = msg.decode_args_with(&[ArgType::String]);
                let app_id = msg.args.first()
                    .and_then(|a| if let Arg::String(Some(s)) = a { Some(s.clone()) } else { None });
                if let Some(app_id) = app_id {
                    if let Some(data) = conn.registry.get_data_mut::<XdgToplevelData>(msg.object_id) {
                        data.app_id = app_id.clone();
                    }
                    shell3.borrow_mut().set_app_id(msg.object_id, app_id);
                }
                Ok(())
            }
            XdgToplevel::SHOW_WINDOW_MENU => {
                // xdg_toplevel.show_window_menu(seat, serial, x, y)
                Ok(())
            }
            XdgToplevel::MOVE => {
                // xdg_toplevel.move(seat, serial) — interactive move
                // Start a move grab: on next motion the window will be dragged
                let _ = msg.decode_args_with(&[ArgType::Object, ArgType::Uint]);
                let _serial = msg.args.get(1)
                    .and_then(|a| if let Arg::Uint(s) = a { Some(*s) } else { None })
                    .unwrap_or(0);
                // Get window position from shell and cursor from shared state
                let cursor = *cursor_pos.borrow();
                if let Some((wx, wy)) = shell3.borrow()
                    .find_by_toplevel(msg.object_id)
                    .map(|w| (w.x, w.y))
                {
                    if let Some(ref fl) = floating {
                        fl.borrow_mut().start_drag(
                            msg.object_id, wx, wy, cursor.0, cursor.1, _serial,
                        );
                    }
                }
                Ok(())
            }
            XdgToplevel::RESIZE => {
                // xdg_toplevel.resize(seat, serial, edges) — interactive resize
                let _ = msg.decode_args_with(&[ArgType::Object, ArgType::Uint, ArgType::Uint]);
                let _serial = msg.args.get(1)
                    .and_then(|a| if let Arg::Uint(s) = a { Some(*s) } else { None })
                    .unwrap_or(0);
                let edges = msg.args.get(2)
                    .and_then(|a| if let Arg::Uint(e) = a { Some(*e) } else { None })
                    .unwrap_or(0);
                let cursor = *cursor_pos.borrow();
                if let Some((wx, wy, ww, wh)) = shell3.borrow()
                    .find_by_toplevel(msg.object_id)
                    .map(|w| (w.x, w.y, w.width, w.height))
                {
                    let resize_edges = crate::floating::ResizeEdges::from_xdg(edges);
                    if let Some(ref fl) = floating {
                        fl.borrow_mut().start_resize(
                            msg.object_id, resize_edges,
                            wx, wy, ww, wh,
                            cursor.0, cursor.1, _serial,
                        );
                    }
                }
                Ok(())
            }
            XdgToplevel::SET_MIN_SIZE => {
                // xdg_toplevel.set_min_size(width, height)
                let _ = msg.decode_args_with(&[ArgType::Int, ArgType::Int]);
                if msg.args.len() >= 2 {
                    let w = if let Arg::Int(v) = msg.args[0] { v } else { 0 };
                    let h = if let Arg::Int(v) = msg.args[1] { v } else { 0 };
                    if let Some(data) = conn.registry.get_data_mut::<XdgToplevelData>(msg.object_id) {
                        data.min_width = w;
                        data.min_height = h;
                    }
                }
                Ok(())
            }
            XdgToplevel::SET_MAX_SIZE => {
                // xdg_toplevel.set_max_size(width, height)
                let _ = msg.decode_args_with(&[ArgType::Int, ArgType::Int]);
                if msg.args.len() >= 2 {
                    let w = if let Arg::Int(v) = msg.args[0] { v } else { 0 };
                    let h = if let Arg::Int(v) = msg.args[1] { v } else { 0 };
                    if let Some(data) = conn.registry.get_data_mut::<XdgToplevelData>(msg.object_id) {
                        data.max_width = w;
                        data.max_height = h;
                    }
                }
                Ok(())
            }
            XdgToplevel::SET_MAXIMIZED => {
                if let Some(data) = conn.registry.get_data_mut::<XdgToplevelData>(msg.object_id) {
                    data.maximized = true;
                }
                shell3.borrow_mut().set_maximized(msg.object_id, true);
                Ok(())
            }
            XdgToplevel::UNSET_MAXIMIZED => {
                if let Some(data) = conn.registry.get_data_mut::<XdgToplevelData>(msg.object_id) {
                    data.maximized = false;
                }
                shell3.borrow_mut().set_maximized(msg.object_id, false);
                Ok(())
            }
            XdgToplevel::SET_FULLSCREEN => {
                if let Some(data) = conn.registry.get_data_mut::<XdgToplevelData>(msg.object_id) {
                    data.fullscreen = true;
                }
                shell3.borrow_mut().set_fullscreen(msg.object_id, true);
                Ok(())
            }
            XdgToplevel::UNSET_FULLSCREEN => {
                if let Some(data) = conn.registry.get_data_mut::<XdgToplevelData>(msg.object_id) {
                    data.fullscreen = false;
                }
                shell3.borrow_mut().set_fullscreen(msg.object_id, false);
                Ok(())
            }
            XdgToplevel::SET_MINIMIZED => {
                if let Some(data) = conn.registry.get_data_mut::<XdgToplevelData>(msg.object_id) {
                    data.minimized = true;
                }
                shell3.borrow_mut().set_minimized(msg.object_id, true);
                Ok(())
            }
            _ => Err(ServerError::UnknownRequest {
                object_id: msg.object_id, interface: alloc::string::String::from(iface::XDG_TOPLEVEL), opcode: msg.opcode
            })
        }
    });

    // ── xdg_popup handler ───────────────────────────────────────────
    server.dispatcher.on_xdg_popup(|conn, msg| {
        let mut msg = msg.clone();

        match msg.opcode {
            XdgPopup::DESTROY => {
                conn.destroy_object(msg.object_id);
                Ok(())
            }
            XdgPopup::GRAB => {
                // xdg_popup.grab(seat, serial)
                Ok(())
            }
            XdgPopup::REPOSITION => {
                // xdg_popup.reposition(positioner_token)
                Ok(())
            }
            _ => Err(ServerError::UnknownRequest {
                object_id: msg.object_id, interface: alloc::string::String::from(iface::XDG_POPUP), opcode: msg.opcode
            })
        }
    });

    // ── xdg_positioner handler ──────────────────────────────────────
    server.dispatcher.on_xdg_positioner(|conn, msg| {
        let mut msg = msg.clone();

        match msg.opcode {
            XdgPositioner::DESTROY => {
                conn.destroy_object(msg.object_id);
                Ok(())
            }
            XdgPositioner::SET_SIZE => {
                let _ = msg.decode_args_with(&[ArgType::Int, ArgType::Int]);
                if msg.args.len() >= 2 {
                    let w = if let Arg::Int(v) = msg.args[0] { v } else { 0 };
                    let h = if let Arg::Int(v) = msg.args[1] { v } else { 0 };
                    if let Some(data) = conn.registry.get_data_mut::<XdgPositionerData>(msg.object_id) {
                        data.width = w;
                        data.height = h;
                    }
                }
                Ok(())
            }
            XdgPositioner::SET_ANCHOR_RECT => {
                let _ = msg.decode_args_with(&[ArgType::Int, ArgType::Int, ArgType::Int, ArgType::Int]);
                if msg.args.len() >= 4 {
                    let x = if let Arg::Int(v) = msg.args[0] { v } else { 0 };
                    let y = if let Arg::Int(v) = msg.args[1] { v } else { 0 };
                    let w = if let Arg::Int(v) = msg.args[2] { v } else { 0 };
                    let h = if let Arg::Int(v) = msg.args[3] { v } else { 0 };
                    if let Some(data) = conn.registry.get_data_mut::<XdgPositionerData>(msg.object_id) {
                        data.anchor_rect_x = x;
                        data.anchor_rect_y = y;
                        data.anchor_rect_w = w;
                        data.anchor_rect_h = h;
                    }
                }
                Ok(())
            }
            XdgPositioner::SET_ANCHOR => {
                let _ = msg.decode_args_with(&[ArgType::Uint]);
                if let Some(anchor) = msg.args.first().and_then(|a| if let Arg::Uint(v) = a { Some(*v) } else { None }) {
                    if let Some(data) = conn.registry.get_data_mut::<XdgPositionerData>(msg.object_id) {
                        data.anchor = anchor;
                    }
                }
                Ok(())
            }
            XdgPositioner::SET_GRAVITY => {
                let _ = msg.decode_args_with(&[ArgType::Uint]);
                if let Some(g) = msg.args.first().and_then(|a| if let Arg::Uint(v) = a { Some(*v) } else { None }) {
                    if let Some(data) = conn.registry.get_data_mut::<XdgPositionerData>(msg.object_id) {
                        data.gravity = g;
                    }
                }
                Ok(())
            }
            XdgPositioner::SET_CONSTRAINT_ADJUSTMENT => {
                let _ = msg.decode_args_with(&[ArgType::Uint]);
                if let Some(ca) = msg.args.first().and_then(|a| if let Arg::Uint(v) = a { Some(*v) } else { None }) {
                    if let Some(data) = conn.registry.get_data_mut::<XdgPositionerData>(msg.object_id) {
                        data.constraint_adjustment = ca;
                    }
                }
                Ok(())
            }
            XdgPositioner::SET_OFFSET => {
                let _ = msg.decode_args_with(&[ArgType::Int, ArgType::Int]);
                if msg.args.len() >= 2 {
                    let x = if let Arg::Int(v) = msg.args[0] { v } else { 0 };
                    let y = if let Arg::Int(v) = msg.args[1] { v } else { 0 };
                    if let Some(data) = conn.registry.get_data_mut::<XdgPositionerData>(msg.object_id) {
                        data.offset_x = x;
                        data.offset_y = y;
                    }
                }
                Ok(())
            }
            XdgPositioner::SET_REACTIVE => {
                Ok(())
            }
            XdgPositioner::SET_PARENT_SIZE => {
                Ok(())
            }
            XdgPositioner::SET_PARENT_CONFIGURE => {
                Ok(())
            }
            _ => Err(ServerError::UnknownRequest {
                object_id: msg.object_id, interface: alloc::string::String::from(iface::XDG_POSITIONER), opcode: msg.opcode
            })
        }
    });
}

/// Set up seat protocol handlers.
///
/// Registers handlers for:
/// - wl_seat: get_pointer, get_keyboard, get_touch, release
/// - wl_pointer: set_cursor, release
/// - wl_keyboard: release
///
/// The seat handler sends `capabilities` and `name` events on every
/// request (slightly redundant but correct for MVP).
pub fn setup_seat_handlers(server: &mut WaylandServer) {
    // ── wl_seat handler ─────────────────────────────────────────────
    server.dispatcher.on_seat(|conn, msg| {
        // Send capabilities and name (safe to repeat — client ignores duplicates)
        conn.send_event(WaylandMessage::new(msg.object_id, WlSeat::CAPABILITIES)
            .arg_uint(1 | 2 | 4)); // pointer=1, keyboard=2, touch=4
        conn.send_event(WaylandMessage::new(msg.object_id, WlSeat::NAME)
            .arg_string("default"));

        let mut msg = msg.clone();

        match msg.opcode {
            WlSeat::GET_POINTER => {
                let _ = msg.decode_args_with(&[ArgType::NewId]);
                let new_id = msg.args.first()
                    .and_then(|a| if let Arg::NewId(id) = a { Some(*id) } else { None })
                    .unwrap_or(0);
                if new_id > 0 {
                    conn.registry.register_with_id(new_id, iface::WL_POINTER, 5);
                }
                Ok(())
            }
            WlSeat::GET_KEYBOARD => {
                let _ = msg.decode_args_with(&[ArgType::NewId]);
                let new_id = msg.args.first()
                    .and_then(|a| if let Arg::NewId(id) = a { Some(*id) } else { None })
                    .unwrap_or(0);
                if new_id > 0 {
                    // Send keymap (for MVP, use a dummy empty keymap)
                    conn.send_event(WaylandMessage::new(new_id, WlKeyboard::KEYMAP)
                        .arg_uint(0)   // keymap format: 0 = no_keymap
                        .arg_fd(0)     // fd: not used
                        .arg_uint(0)); // size: 0
                    conn.send_event(WaylandMessage::new(new_id, WlKeyboard::REPEAT_INFO)
                        .arg_int(0)    // rate: 0 (no repeat)
                        .arg_int(0));  // delay: 0
                    conn.registry.register_with_id(new_id, iface::WL_KEYBOARD, 5);
                }
                Ok(())
            }
            WlSeat::GET_TOUCH => {
                let _ = msg.decode_args_with(&[ArgType::NewId]);
                let new_id = msg.args.first()
                    .and_then(|a| if let Arg::NewId(id) = a { Some(*id) } else { None })
                    .unwrap_or(0);
                if new_id > 0 {
                    conn.registry.register_with_id(new_id, iface::WL_TOUCH, 5);
                }
                Ok(())
            }
            WlSeat::RELEASE => {
                conn.destroy_object(msg.object_id);
                Ok(())
            }
            _ => Err(ServerError::UnknownRequest {
                object_id: msg.object_id, interface: alloc::string::String::from(iface::WL_SEAT), opcode: msg.opcode
            })
        }
    });

    // ── wl_pointer handler ────────────────────────────────────────────
    server.dispatcher.on_pointer(|conn, msg| {
        match msg.opcode {
            WlPointer::SET_CURSOR => {
                // wl_pointer.set_cursor(serial, surface, hotspot_x, hotspot_y)
                // For MVP, we store the cursor info (could be used by compositor)
                let _ = msg.clone().decode_args_with(&[ArgType::Uint, ArgType::Object, ArgType::Int, ArgType::Int]);
                Ok(())
            }
            WlPointer::RELEASE => {
                conn.destroy_object(msg.object_id);
                Ok(())
            }
            _ => Err(ServerError::UnknownRequest {
                object_id: msg.object_id, interface: alloc::string::String::from(iface::WL_POINTER), opcode: msg.opcode
            })
        }
    });

    // ── wl_keyboard handler ────────────────────────────────────────────
    server.dispatcher.on_keyboard(|conn, msg| {
        match msg.opcode {
            WlKeyboard::RELEASE => {
                conn.destroy_object(msg.object_id);
                Ok(())
            }
            _ => Err(ServerError::UnknownRequest {
                object_id: msg.object_id, interface: alloc::string::String::from(iface::WL_KEYBOARD), opcode: msg.opcode
            })
        }
    });
}

// ── Data structures for data device state ───────────────────────────

/// State stored on a wl_data_source object.
pub struct DataSourceState {
    /// Connection index that owns this source.
    pub conn_idx: usize,
    /// List of advertised MIME types.
    pub mime_types: Vec<alloc::string::String>,
}

impl DataSourceState {
    pub fn new(conn_idx: usize) -> Self {
        Self { conn_idx, mime_types: Vec::new() }
    }
}

/// Tracks the current clipboard/data-device state for the compositor.
#[derive(Clone)]
pub struct DataDeviceState {
    /// The connection index that owns the current selection (clipboard), if any.
    pub selection_conn: Option<usize>,
    /// The wl_data_source object ID for the current selection.
    pub selection_source_id: Option<u32>,
    /// Incrementing serial for selection changes.
    pub serial: u32,
}

impl DataDeviceState {
    pub fn new() -> Self {
        Self {
            selection_conn: None,
            selection_source_id: None,
            serial: 1,
        }
    }

    pub fn next_serial(&mut self) -> u32 {
        let s = self.serial;
        self.serial = self.serial.wrapping_add(1);
        s
    }
}

/// Set up data device protocol handlers (clipboard).
///
/// Registers handlers for:
/// - wl_data_device_manager: create_data_source, get_data_device
/// - wl_data_device: start_drag, set_selection, release
/// - wl_data_source: offer, destroy
/// - wl_data_offer: accept, receive, destroy, finish, set_actions
///
/// For MVP, this supports clipboard (set_selection → selection) with
/// drag-and-drop stubs.
pub fn setup_data_device_handlers(server: &mut WaylandServer) {
    let data_state = Rc::new(RefCell::new(DataDeviceState::new()));
    let data_state2 = data_state.clone();

    // ── wl_data_device_manager handler ────────────────────────────────
    server.dispatcher.on_data_device_manager(move |conn, msg| {
        let mut msg = msg.clone();
        let _ = msg.decode_args_with(&[ArgType::NewId]);
        let new_id = msg.args.first()
            .and_then(|a| if let Arg::NewId(id) = a { Some(*id) } else { None })
            .unwrap_or(0);

        match msg.opcode {
            WlDataDeviceManager::CREATE_DATA_SOURCE => {
                if new_id > 0 {
                    conn.registry.register_with_id_and_data(
                        new_id, iface::WL_DATA_SOURCE, 3,
                        DataSourceState::new(conn.conn_idx),
                    );
                }
                Ok(())
            }
            WlDataDeviceManager::GET_DATA_DEVICE => {
                if new_id > 0 {
                    conn.registry.register_with_id(new_id, iface::WL_DATA_DEVICE, 3);
                }
                Ok(())
            }
            _ => Err(ServerError::UnknownRequest {
                object_id: msg.object_id, interface: alloc::string::String::from(iface::WL_DATA_DEVICE_MANAGER), opcode: msg.opcode
            })
        }
    });

    // ── wl_data_device handler ────────────────────────────────────────
    let dstate = data_state;
    server.dispatcher.on_data_device(move |conn, msg| {
        match msg.opcode {
            WlDataDevice::SET_SELECTION => {
                // wl_data_device.set_selection(source, serial)
                let mut msg = msg.clone();
                let _ = msg.decode_args_with(&[ArgType::Object, ArgType::Uint]);
                let source_id = msg.args.first()
                    .and_then(|a| if let Arg::Object(id) = a { Some(*id) } else { None })
                    .unwrap_or(0);

                // Update global selection state
                let mut ds = dstate.borrow_mut();
                let _serial = ds.next_serial();
                if source_id > 0 {
                    // Source must be on this connection (client created it on its own connection)
                    ds.selection_conn = Some(conn.conn_idx);
                    ds.selection_source_id = Some(source_id);
                } else {
                    ds.selection_conn = None;
                    ds.selection_source_id = None;
                }
                drop(ds);

                Ok(())
            }
            WlDataDevice::START_DRAG => {
                // Drag-and-drop: stub for MVP
                Ok(())
            }
            WlDataDevice::RELEASE => {
                conn.destroy_object(msg.object_id);
                Ok(())
            }
            _ => Err(ServerError::UnknownRequest {
                object_id: msg.object_id, interface: alloc::string::String::from(iface::WL_DATA_DEVICE), opcode: msg.opcode
            })
        }
    });

    // ── wl_data_source handler ────────────────────────────────────────
    server.dispatcher.on_data_source(move |conn, msg| {
        match msg.opcode {
            WlDataSource::OFFER => {
                // wl_data_source.offer(mime_type)
                let mut msg = msg.clone();
                let _ = msg.decode_args_with(&[ArgType::String]);
                if let Some(mime) = msg.args.first()
                    .and_then(|a| if let Arg::String(s) = a { s.clone() } else { None })
                {
                    if let Some(data) = conn.registry.get_data_mut::<DataSourceState>(msg.object_id) {
                        data.mime_types.push(mime);
                    }
                }
                Ok(())
            }
            WlDataSource::DESTROY => {
                conn.destroy_object(msg.object_id);
                Ok(())
            }
            _ => Err(ServerError::UnknownRequest {
                object_id: msg.object_id, interface: alloc::string::String::from(iface::WL_DATA_SOURCE), opcode: msg.opcode
            })
        }
    });

    // ── wl_data_offer handler ────────────────────────────────────────
    server.dispatcher.on_data_offer(|conn, msg| {
        match msg.opcode {
            WlDataOffer::ACCEPT => {
                // wl_data_offer.accept(serial, mime_type)
                Ok(())
            }
            WlDataOffer::RECEIVE => {
                // wl_data_offer.receive(mime_type, fd)
                // For MVP, trigger wl_data_source.send on the source client
                let mut msg = msg.clone();
                let _ = msg.decode_args_with(&[ArgType::String, ArgType::Fd]);
                let mime_type = msg.args.first()
                    .and_then(|a| if let Arg::String(Some(s)) = a { Some(s.clone()) } else { None });
                let _fd = msg.args.get(1)
                    .and_then(|a| if let Arg::Fd(fd) = a { Some(*fd) } else { None });
                if let Some(mime) = mime_type {
                    // Walk connections to find the source client and send send event
                    // For MVP: just accept the receive request
                }
                Ok(())
            }
            WlDataOffer::DESTROY => {
                conn.destroy_object(msg.object_id);
                Ok(())
            }
            WlDataOffer::FINISH => Ok(()),
            WlDataOffer::SET_ACTIONS => Ok(()),
            _ => Err(ServerError::UnknownRequest {
                object_id: msg.object_id, interface: alloc::string::String::from(iface::WL_DATA_OFFER), opcode: msg.opcode
            })
        }
    });

    // Store data device state on the server so update_keyboard_focus can use it
    server.data_device_state = Some(data_state2);
}

/// Helper to send the current clipboard selection to a newly focused client.
pub(crate) fn send_selection_to_client(server: &mut WaylandServer, conn_idx: usize) {
    let ds = server.data_device_state.as_ref().map(|d| d.borrow().clone());
    let (sel_conn, sel_source_id) = match ds {
        Some(ref s) => (s.selection_conn, s.selection_source_id),
        None => return,
    };        // If there's no selection, send NULL selection
    let source_conn_idx = match sel_conn {
        Some(idx) if idx < server.connections.len() => idx,
        _ => {
            // Send NULL selection
            if let Some(conn) = server.connections.get_mut(conn_idx) {
                let dd_id = conn.registry.get_by_interface(iface::WL_DATA_DEVICE)
                    .first().map(|d| d.id);
                if let Some(did) = dd_id {
                    conn.send_event(WaylandMessage::new(did, WlDataDevice::SELECTION)
                        .arg_object(0)); // NULL = no selection
                }
            }
            return;
        }
    };

    let source_id = match sel_source_id {
        Some(id) => id,
        None => return,
    };

    // Get MIME types from the source
    let mime_types = server.connections.get(source_conn_idx)
        .and_then(|c| c.registry.get_data::<DataSourceState>(source_id))
        .map(|ds| ds.mime_types.clone())
        .unwrap_or_default();

    if mime_types.is_empty() {
        // No MIME types advertised — send NULL
        if let Some(conn) = server.connections.get_mut(conn_idx) {
            let dd_id = conn.registry.get_by_interface(iface::WL_DATA_DEVICE)
                .first().map(|d| d.id);
            if let Some(did) = dd_id {
                conn.send_event(WaylandMessage::new(did, WlDataDevice::SELECTION)
                    .arg_object(0));
            }
        }
        return;
    }

    // Create a wl_data_offer for the requesting client
    let dd_id = server.connections.get(conn_idx)
        .and_then(|c| c.registry.get_by_interface(iface::WL_DATA_DEVICE).first().map(|d| d.id));
    if let Some(did) = dd_id {
        if let Some(conn) = server.connections.get_mut(conn_idx) {
            // Create a new wl_data_offer (auto-assigned ID for MVP)
            let offer_id = conn.registry.register(iface::WL_DATA_OFFER, 3);

            // Send DATA_OFFER event to create the offer
            conn.send_event(WaylandMessage::new(did, WlDataDevice::DATA_OFFER)
                .arg_new_id(offer_id));

            // Send OFFER events for each MIME type
            for mime in &mime_types {
                conn.send_event(WaylandMessage::new(offer_id, WlDataOffer::OFFER)
                    .arg_string(mime));
            }

            // Send SELECTION event with the offer
            conn.send_event(WaylandMessage::new(did, WlDataDevice::SELECTION)
                .arg_object(offer_id));
        }
    }
}

// ════════════════════════════════════════════════════════════════
// Update WaylandServer to include data_device_state
// ════════════════════════════════════════════════════════════════
// We add the data_device_state field inline in the struct definition above.

impl WaylandServer {
    /// Process a single minix-input event and dispatch to clients.
    ///
    /// Handles pointer motion, button presses, scroll wheel, and keyboard
    /// events. Updates seat state (focus, cursor position, serials).
    /// Also handles floating window drag/resize operations.
    pub fn process_input_event(&mut self, event: &InputEvent) {
        let serial = self.seat_state.next_serial();

        // Update shared cursor position for handler closures
        match event {
            InputEvent::MouseMotion { x, y, .. }
            | InputEvent::MouseButton { x, y, .. } => {
                *self.cursor_pos.borrow_mut() = (*x, *y);
            }
            _ => {}
        }

        match event {
            InputEvent::MouseMotion { x, y, .. } => {
                let old_x = self.seat_state.pointer_x;
                let old_y = self.seat_state.pointer_y;
                self.seat_state.pointer_x = *x;
                self.seat_state.pointer_y = *y;

                if old_x == *x && old_y == *y {
                    return;
                }

                // ── Handle floating drag/resize ─────────────────────
                if let Some(ref fl) = self.floating {
                    let mut fl = fl.borrow_mut();

                    // Handle active drag (move)
                    if fl.drag.is_some() {
                        if let Some((new_x, new_y)) = fl.on_drag_motion(*x, *y) {
                            let drag_id = fl.drag.as_ref().map(|d| d.xdg_toplevel_id).unwrap_or(0);
                            // Update shell window position
                            self.shell.borrow_mut().set_position(drag_id, new_x, new_y);
                            // Update compositor surface position
                            if let Some(win) = self.shell.borrow().find_by_toplevel(drag_id) {
                                let surface_id = win.surface_id as u64;
                                let mut comp = self.compositor.borrow_mut();
                                if let Some(s) = comp.get_surface(surface_id) {
                                    s.x = new_x;
                                    s.y = new_y;
                                    s.mark_dirty();
                                }
                            }
                        }
                        return;
                    }

                    // Handle active resize
                    if fl.resize.is_some() {
                        if let Some((new_x, new_y, new_w, new_h)) = fl.on_resize_motion(*x, *y) {
                            let resize_id = fl.resize.as_ref().map(|r| r.xdg_toplevel_id).unwrap_or(0);
                            // Update shell window position/size
                            self.shell.borrow_mut().set_position(resize_id, new_x, new_y);
                            self.shell.borrow_mut().set_size(resize_id, new_w, new_h);
                            // Update compositor surface
                            if let Some(win) = self.shell.borrow().find_by_toplevel(resize_id) {
                                let surface_id = win.surface_id as u64;
                                let mut comp = self.compositor.borrow_mut();
                                if let Some(s) = comp.get_surface(surface_id) {
                                    s.x = new_x;
                                    s.y = new_y;
                                    s.mark_dirty();
                                }
                            }
                            // Send XdgToplevel::CONFIGURE with new size
                            if let Some(win) = self.shell.borrow().find_by_toplevel(resize_id) {
                                if let Some(conn) = self.connections.get_mut(win.conn_idx) {
                                    conn.send_event(WaylandMessage::new(resize_id, XdgToplevel::CONFIGURE)
                                        .arg_int(new_w).arg_int(new_h)
                                        .arg_array(alloc::vec![]));
                                }
                            }
                        }
                        return;
                    }
                }

                // Find which surface is under the cursor and update pointer focus
                self.update_pointer_focus(serial);

                // Send wl_pointer.motion to the client with pointer focus
                if let Some(ref focus) = self.seat_state.pointer_focus {
                    let ptr_id = self.connections.get(focus.conn_idx)
                        .and_then(|c| c.registry.get_by_interface(iface::WL_POINTER).first().map(|p| p.id));
                    if let Some(pid) = ptr_id {
                        if let Some(conn) = self.connections.get_mut(focus.conn_idx) {
                            // Convert to surface-local coordinates
                            let mut comp = self.compositor.borrow_mut();
                            let (lx, ly) = if let Some(s) = comp.get_surface(focus.surface_id as u64) {
                                (*x - s.x, *y - s.y)
                            } else {
                                (*x, *y)
                            };
                            drop(comp);
                            conn.send_event(WaylandMessage::new(pid, WlPointer::MOTION)
                                .arg_uint(serial)
                                .arg_fixed(Fixed::from_int(lx))
                                .arg_fixed(Fixed::from_int(ly)));
                            conn.send_event(WaylandMessage::new(pid, WlPointer::FRAME));
                        }
                    }
                }
            }
            InputEvent::MouseButton { button, pressed, x, y, .. } => {
                self.seat_state.pointer_x = *x;
                self.seat_state.pointer_y = *y;

                // ── End floating drag/resize on button release ──────
                if !*pressed {
                    if let Some(ref fl) = self.floating {
                        let mut fl = fl.borrow_mut();
                        if fl.drag.is_some() {
                            fl.end_drag();
                        }
                        if fl.resize.is_some() {
                            fl.end_resize();
                        }
                    }
                }

                // On button press
                if *pressed {
                    // Handle panel clicks (before decoration clicks — panel is on top)
                    if let Some(ref panel) = self.panel {
                        let action = panel.borrow().handle_click(*x, *y);
                        match action {
                            PanelAction::SwitchWorkspace(idx) => {
                                self.switch_workspace(idx);
                                return;
                            }
                            PanelAction::None => {}
                        }
                    }
                    // Handle decoration clicks (close/minimize/maximize/title bar drag)
                    if self.handle_title_bar_click(*x, *y) {
                        return;
                    }
                    // Set keyboard focus to the surface under cursor
                    self.update_keyboard_focus(serial);
                }

                // Update pointer focus
                self.update_pointer_focus(serial);

                // Send wl_pointer.button to the client with pointer focus
                if let Some(ref focus) = self.seat_state.pointer_focus {
                    let ptr_id = self.connections.get(focus.conn_idx)
                        .and_then(|c| c.registry.get_by_interface(iface::WL_POINTER).first().map(|p| p.id));
                    if let Some(pid) = ptr_id {
                        if let Some(conn) = self.connections.get_mut(focus.conn_idx) {
                            conn.send_event(WaylandMessage::new(pid, WlPointer::BUTTON)
                                .arg_uint(serial)
                                .arg_uint(serial)
                                .arg_uint(match button {
                                    minix_input::MouseButton::Left => 272,
                                    minix_input::MouseButton::Right => 273,
                                    minix_input::MouseButton::Middle => 274,
                                    minix_input::MouseButton::Back => 275,
                                    minix_input::MouseButton::Forward => 276,
                                    minix_input::MouseButton::Other(n) => *n as u32,
                                })
                                .arg_uint(if *pressed { 1 } else { 0 }));
                            conn.send_event(WaylandMessage::new(pid, WlPointer::FRAME));
                        }
                    }
                }
            }
            InputEvent::MouseWheel { delta_y, .. } => {
                if let Some(ref focus) = self.seat_state.pointer_focus {
                    let ptr_id = self.connections.get(focus.conn_idx)
                        .and_then(|c| c.registry.get_by_interface(iface::WL_POINTER).first().map(|p| p.id));
                    if let Some(pid) = ptr_id {
                        if let Some(conn) = self.connections.get_mut(focus.conn_idx) {
                            conn.send_event(WaylandMessage::new(pid, WlPointer::AXIS)
                                .arg_uint(serial)
                                .arg_uint(0)  // vertical
                                .arg_fixed(Fixed::from_f64(*delta_y as f64)));
                            conn.send_event(WaylandMessage::new(pid, WlPointer::FRAME));
                        }
                    }
                }
            }
            InputEvent::Keyboard { key, pressed, modifiers } => {
                // ── Handle compositor keyboard shortcuts (only on press) ──
                if *pressed {
                    if let Some(ref kb) = self.keybindings {
                        let mod_mask = ModMask::from_input_modifiers(*modifiers);
                        if let Some(action) = kb.lookup(mod_mask, *key) {
                            self.handle_keybind_action(action.clone());
                            return; // don't forward to client
                        }
                    }
                }

                // Forward to the focused client
                if let Some(ref focus) = self.seat_state.keyboard_focus {
                    let kbd_id = self.connections.get(focus.conn_idx)
                        .and_then(|c| c.registry.get_by_interface(iface::WL_KEYBOARD).first().map(|p| p.id));
                    if let Some(kid) = kbd_id {
                        if let Some(conn) = self.connections.get_mut(focus.conn_idx) {
                            let keycode = *key as u32;
                            conn.send_event(WaylandMessage::new(kid, WlKeyboard::KEY)
                                .arg_uint(serial)
                                .arg_uint(serial)
                                .arg_uint(keycode)
                                .arg_uint(if *pressed { 1 } else { 0 }));
                        }
                    }
                }
            }
            InputEvent::Touch { .. } => {
                // Touch not yet implemented
            }
            InputEvent::Frame => {
                // Frame boundary — nothing to do (events are flushed on tick)
            }
        }
    }

    /// Execute a keyboard shortcut action.
    fn handle_keybind_action(&mut self, action: KeyAction) {
        match action {
            KeyAction::LaunchTerminal => {
                // Stub: launch a terminal app when MINIX has a terminal compositor app.
                // For now, this is a no-op placeholder.
            }
            KeyAction::CloseWindow => {
                self.close_focused_window();
            }
            KeyAction::SwitchWorkspaceNext => {
                self.switch_workspace_next();
            }
            KeyAction::SwitchWorkspacePrev => {
                self.switch_workspace_prev();
            }
            KeyAction::ToggleFloating => {
                self.toggle_focused_window_floating();
            }
            KeyAction::ToggleMaximize => {
                self.toggle_focused_window_maximized();
            }
            KeyAction::LaunchApp(_app) => {
                // Stub: launch a named app when we have applications.
            }
            KeyAction::RunCommand(_cmd) => {
                // Stub: execute a shell command.
            }
        }
    }

    /// Close the currently focused window, if any.
    fn close_focused_window(&mut self) {
        let xdg_id = self.seat_state.keyboard_focus.as_ref()
            .and_then(|f| {
                self.shell.borrow()
                    .find_by_surface(f.surface_id)
                    .map(|w| w.xdg_toplevel_id)
            });
        if let Some(id) = xdg_id {
            let _ = self.shell_close_window(id);
        }
    }

    /// Toggle the focused window between floating and tiling mode.
    fn toggle_focused_window_floating(&mut self) {
        let target = self.seat_state.keyboard_focus.as_ref()
            .and_then(|f| {
                self.shell.borrow()
                    .find_by_surface(f.surface_id)
                    .map(|w| w.xdg_toplevel_id)
            });
        if let Some(xdg_id) = target {
            if let Some(ref fl) = self.floating {
                let mut fl = fl.borrow_mut();
                if fl.is_floating(xdg_id) {
                    fl.set_floating(xdg_id, false);
                } else {
                    fl.set_floating(xdg_id, true);
                }
            }
            self.recalculate_layout();
        }
    }

    /// Toggle the focused window between maximized and normal state.
    fn toggle_focused_window_maximized(&mut self) {
        let target = self.seat_state.keyboard_focus.as_ref()
            .and_then(|f| {
                self.shell.borrow()
                    .find_by_surface(f.surface_id)
                    .map(|w| (w.xdg_toplevel_id, w.maximized))
            });
        if let Some((xdg_id, is_max)) = target {
            self.shell.borrow_mut().set_maximized(xdg_id, !is_max);
            self.recalculate_layout();
        }
    }

    /// Handle a click on a window decoration (title bar buttons or drag).
    ///
    /// Returns true if the click was handled and should not be forwarded
    /// to the client as a wl_pointer.button event.
    fn handle_title_bar_click(&mut self, x: i32, y: i32) -> bool {
        if self.decorator.is_none() {
            return false;
        }

        let target = {
            let shell = self.shell.borrow();
            match shell.topmost_at(x, y) {
                Some(win) if win.visible => (win.xdg_toplevel_id, win.width, win.x, win.y),
                _ => return false,
            }
        };
        let (xdg_id, width, wx, wy) = target;

        // Check if click is on the title bar (not on window content)
        if !Decorator::is_on_title_bar(y, wy) {
            return false;
        }

        // Check close button
        if Decorator::is_on_close_button(x, y, wx, wy, width) {
            let _ = self.shell_close_window(xdg_id);
            return true;
        }

        // Check minimize button
        if Decorator::is_on_minimize_button(x, y, wx, wy, width) {
            self.shell.borrow_mut().set_minimized(xdg_id, true);
            // Hide the compositor surface
            if let Some(win) = self.shell.borrow().find_by_toplevel(xdg_id) {
                let mut comp = self.compositor.borrow_mut();
                if let Some(s) = comp.get_surface(win.surface_id as u64) {
                    s.visible = false;
                }
            }
            self.recalculate_layout();
            return true;
        }

        // Check maximize button
        if Decorator::is_on_maximize_button(x, y, wx, wy, width) {
            let is_max = self.shell.borrow()
                .find_by_toplevel(xdg_id)
                .map(|w| w.maximized)
                .unwrap_or(false);
            self.shell.borrow_mut().set_maximized(xdg_id, !is_max);
            self.recalculate_layout();
            return true;
        }

        // Click on title bar (not on any button) → start floating drag
        let drag = {
            let shell = self.shell.borrow();
            shell.find_by_toplevel(xdg_id).map(|w| (w.x, w.y))
        };
        if let Some((wx2, wy2)) = drag {
            if let Some(ref fl) = self.floating {
                let cursor = *self.cursor_pos.borrow();
                fl.borrow_mut().start_drag(xdg_id, wx2, wy2, cursor.0, cursor.1, 0);
            }
        }
        true
    }

    /// Process a batch of input events.
    pub fn process_input_events(&mut self, events: &[InputEvent]) {
        for event in events {
            self.process_input_event(event);
        }
    }

    /// Update pointer focus: check what surface is under the cursor.
    fn update_pointer_focus(&mut self, serial: u32) {
        let x = self.seat_state.pointer_x;
        let y = self.seat_state.pointer_y;
        let mut comp = self.compositor.borrow_mut();

        // Find the topmost surface under the cursor (highest z_order that contains the point)
        let mut found_surface: Option<u64> = None;
        let mut found_conn: Option<usize> = None;
        let mut found_z: i32 = i32::MIN;

        // Walk through connections to find which one owns which surface
        for (conn_idx, conn) in self.connections.iter().enumerate() {
            for obj in conn.registry.iter() {
                if obj.interface != iface::WL_SURFACE && obj.interface != iface::XDG_TOPLEVEL {
                    continue;
                }
                let surface_id = obj.id as u64;
                if let Some(s) = comp.get_surface(surface_id) {
                    if s.visible && x >= s.x && x < s.right() && y >= s.y && y < s.bottom() && s.z_order > found_z {
                        found_surface = Some(surface_id);
                        found_conn = Some(conn_idx);
                        found_z = s.z_order;
                    }
                }
            }
        }

        drop(comp);

        let new_focus = found_surface.and_then(|sid| {
            found_conn.map(|ci| FocusTarget { conn_idx: ci, surface_id: sid as u32 })
        });

        // Check if focus changed
        if self.seat_state.pointer_focus.as_ref().map(|f| (f.conn_idx, f.surface_id))
            != new_focus.as_ref().map(|f| (f.conn_idx, f.surface_id))
        {
            // Send leave to old focus
            if let Some(ref old) = self.seat_state.pointer_focus {
                let ptr_id = self.connections.get(old.conn_idx)
                    .and_then(|c| c.registry.get_by_interface(iface::WL_POINTER).first().map(|p| p.id));
                if let Some(pid) = ptr_id {
                    if let Some(conn) = self.connections.get_mut(old.conn_idx) {
                        conn.send_event(WaylandMessage::new(pid, WlPointer::LEAVE)
                            .arg_uint(serial)
                            .arg_object(old.surface_id));
                    }
                }
            }

            // Send enter to new focus
            if let Some(ref new) = new_focus {
                let ptr_id = self.connections.get(new.conn_idx)
                    .and_then(|c| c.registry.get_by_interface(iface::WL_POINTER).first().map(|p| p.id));
                if let Some(pid) = ptr_id {
                    if let Some(conn) = self.connections.get_mut(new.conn_idx) {
                        let mut comp = self.compositor.borrow_mut();
                        let (lx, ly) = if let Some(s) = comp.get_surface(new.surface_id as u64) {
                            (x - s.x, y - s.y)
                        } else {
                            (x, y)
                        };
                        drop(comp);
                        conn.send_event(WaylandMessage::new(pid, WlPointer::ENTER)
                            .arg_uint(serial)
                            .arg_object(new.surface_id)
                            .arg_fixed(Fixed::from_int(lx))
                            .arg_fixed(Fixed::from_int(ly)));
                        conn.send_event(WaylandMessage::new(pid, WlPointer::FRAME));
                    }
                }
            }

            self.seat_state.pointer_focus = new_focus;
            self.seat_state.pointer_serial = serial;
        }
    }

    /// Update keyboard focus: set to the current pointer focus target.
    fn update_keyboard_focus(&mut self, serial: u32) {
        let new_focus = self.seat_state.pointer_focus.clone();

        // Check if focus changed
        if self.seat_state.keyboard_focus.as_ref().map(|f| (f.conn_idx, f.surface_id))
            != new_focus.as_ref().map(|f| (f.conn_idx, f.surface_id))
        {
            // Send leave to old focus
            if let Some(ref old) = self.seat_state.keyboard_focus {
                let kbd_id = self.connections.get(old.conn_idx)
                    .and_then(|c| c.registry.get_by_interface(iface::WL_KEYBOARD).first().map(|p| p.id));
                if let Some(kid) = kbd_id {
                    if let Some(conn) = self.connections.get_mut(old.conn_idx) {
                        conn.send_event(WaylandMessage::new(kid, WlKeyboard::LEAVE)
                            .arg_uint(serial)
                            .arg_object(old.surface_id));
                    }
                }
            }

            // Send enter to new focus
            if let Some(ref new) = new_focus {
                // Raise the window in the shell
                let surf_id = new.surface_id;
                let shell = self.shell.borrow();
                if let Some(win) = shell.find_by_surface(surf_id) {
                    let toplevel_id = win.xdg_toplevel_id;
                    drop(shell);
                    self.shell_raise_window(toplevel_id);
                } else {
                    drop(shell);
                }

                let kbd_id = self.connections.get(new.conn_idx)
                    .and_then(|c| c.registry.get_by_interface(iface::WL_KEYBOARD).first().map(|p| p.id));
                if let Some(kid) = kbd_id {
                    if let Some(conn) = self.connections.get_mut(new.conn_idx) {
                        conn.send_event(WaylandMessage::new(kid, WlKeyboard::ENTER)
                            .arg_uint(serial)
                            .arg_object(new.surface_id)
                            .arg_array(alloc::vec![])); // keys: empty array
                    }
                }
            }

            // Send clipboard selection to newly focused client
            if let Some(ref new) = new_focus {
                send_selection_to_client(self, new.conn_idx);
            }

            // Mark all decorations dirty (active/inactive state changed)
            if let Some(ref deco) = self.decorator {
                deco.borrow_mut().mark_all_dirty();
            }
            self.seat_state.keyboard_focus = new_focus;
            self.seat_state.keyboard_serial = serial;
        }
    }
}

// ════════════════════════════════════════════════════════════════
// Tests
// ════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::create_transport_pair;

    fn new_server() -> WaylandServer {
        let comp = Rc::new(RefCell::new(Compositor::new(800, 600)));
        let mut server = WaylandServer::new(comp);
        setup_default_handlers(&mut server);
        setup_compositor_handlers(&mut server);
        setup_xdg_shell_handlers(&mut server);
        setup_seat_handlers(&mut server);
        server
    }

    #[test]
    fn server_creation() {
        let comp = Rc::new(RefCell::new(Compositor::new(800, 600)));
        let server = WaylandServer::new(comp);
        assert_eq!(server.client_count(), 0);
        assert_eq!(server.globals.all().len(), 5); // compositor, shm, seat, data_device_manager, xdg_wm_base
    }

    #[test]
    fn accept_client() {
        let mut server = new_server();
        let (s_tx, _c_tx) = create_transport_pair();
        server.accept(Box::new(s_tx));
        assert_eq!(server.client_count(), 1);
    }

    #[test]
    fn client_receives_globals_on_connect() {
        let mut server = new_server();
        let (s_tx, mut c_tx) = create_transport_pair();
        {
            let conn = server.accept(Box::new(s_tx));
            conn.flush().unwrap();
        }

        for _ in 0..5 {  // 5 globals: compositor, shm, seat, data_device_manager, xdg_wm_base
            let msg = c_tx.receive().unwrap();
            assert_eq!(msg.object_id, WL_REGISTRY_ID);
            assert_eq!(msg.opcode, WlRegistry::GLOBAL);
        }
    }

    #[test]
    fn client_sync_request() {
        let mut server = new_server();
        let (s_tx, mut c_tx) = create_transport_pair();
        {
            let conn = server.accept(Box::new(s_tx));
            conn.flush().unwrap();
        }
        for _ in 0..5 { let _ = c_tx.receive().unwrap(); }

        // Client sends wl_display.sync request with callback ID 100
        let sync_msg = WaylandMessage::new(WL_DISPLAY_ID, WlDisplay::SYNC)
            .arg_new_id(100);
        c_tx.send(&sync_msg).unwrap();

        server.tick();

        // Client should receive wl_callback.done
        let response = c_tx.receive().unwrap();
        assert_eq!(response.object_id, 100);
        assert_eq!(response.opcode, WlCallback::DONE);
    }

    #[test]
    fn multiple_clients() {
        let mut server = new_server();
        let (s1, _c1) = create_transport_pair();
        let (s2, _c2) = create_transport_pair();

        server.accept(Box::new(s1));
        server.accept(Box::new(s2));

        assert_eq!(server.client_count(), 2);
    }

    #[test]
    fn broadcast_event() {
        let mut server = new_server();
        let (s_tx, mut c_tx) = create_transport_pair();
        {
            let conn = server.accept(Box::new(s_tx));
            conn.flush().unwrap();
        }

        for _ in 0..5 { let _ = c_tx.receive().unwrap(); }

        server.broadcast(WaylandMessage::new(42, 7));
        server.connections[0].flush().unwrap();

        let received = c_tx.receive().unwrap();
        assert_eq!(received.object_id, 42);
        assert_eq!(received.opcode, 7);
    }

    #[test]
    fn dispatching_unknown_object() {
        let mut server = new_server();
        let (s_tx, mut c_tx) = create_transport_pair();
        server.accept(Box::new(s_tx));

        c_tx.send(&WaylandMessage::new(9999, 0)).unwrap();
        server.tick();

        assert!(server.connections.is_empty() || !server.connections[0].alive);
    }

    #[test]
    fn create_surface_via_registry_bind() {
        let mut server = new_server();
        let (s_tx, mut c_tx) = create_transport_pair();
        {
            let conn = server.accept(Box::new(s_tx));
            conn.flush().unwrap();
        }
        for _ in 0..5 { let _ = c_tx.receive().unwrap(); }

        // Client binds to wl_compositor (global name 1)
        let bind_msg = WaylandMessage::new(WL_REGISTRY_ID, WlRegistry::BIND)
            .arg_uint(1)       // name of wl_compositor global
            .arg_new_id(3)     // client wants ID 3 for the compositor object
            .arg_string("wl_compositor")
            .arg_uint(4);      // version
        c_tx.send(&bind_msg).unwrap();
        server.tick();

        // Now create a surface via the compositor object (ID 3)
        let create_surface = WaylandMessage::new(3, WlCompositor::CREATE_SURFACE)
            .arg_new_id(4);    // new wl_surface ID = 4
        c_tx.send(&create_surface).unwrap();
        server.tick();

        // Check that the surface was registered in connection's registry
        assert!(server.connections[0].registry.get(4).is_some());
        assert_eq!(server.connections[0].registry.get(4).unwrap().interface, iface::WL_SURFACE);
    }

    #[test]
    fn create_and_commit_surface() {
        let mut server = new_server();
        let (s_tx, mut c_tx) = create_transport_pair();
        {
            let conn = server.accept(Box::new(s_tx));
            conn.flush().unwrap();
        }
        for _ in 0..5 { let _ = c_tx.receive().unwrap(); }

        // Bind compositor at ID 3
        c_tx.send(&WaylandMessage::new(WL_REGISTRY_ID, WlRegistry::BIND)
            .arg_uint(1).arg_new_id(3).arg_string("wl_compositor").arg_uint(4)).unwrap();
        server.tick();

        // Create surface at ID 4
        c_tx.send(&WaylandMessage::new(3, WlCompositor::CREATE_SURFACE)
            .arg_new_id(4)).unwrap();
        server.tick();

        // Attach and commit
        c_tx.send(&WaylandMessage::new(4, WlSurface::ATTACH)
            .arg_object(0).arg_int(0).arg_int(0)).unwrap();
        server.tick();

        c_tx.send(&WaylandMessage::new(4, WlSurface::COMMIT)).unwrap();
        server.tick();

        // Surface should now exist in the compositor
        assert!(server.connections[0].registry.get(4).is_some());
        let mut comp = server.compositor.borrow_mut();
        assert!(comp.get_surface(4).is_some(), "surface should exist in compositor");
    }

    #[test]
    fn create_region() {
        let mut server = new_server();
        let (s_tx, mut c_tx) = create_transport_pair();
        {
            let conn = server.accept(Box::new(s_tx));
            conn.flush().unwrap();
        }
        for _ in 0..5 { let _ = c_tx.receive().unwrap(); }

        // Bind compositor at ID 3
        c_tx.send(&WaylandMessage::new(WL_REGISTRY_ID, WlRegistry::BIND)
            .arg_uint(1).arg_new_id(3).arg_string("wl_compositor").arg_uint(4)).unwrap();
        server.tick();

        // Create region at ID 5
        c_tx.send(&WaylandMessage::new(3, WlCompositor::CREATE_REGION)
            .arg_new_id(5)).unwrap();
        server.tick();

        // Add rectangle to region
        c_tx.send(&WaylandMessage::new(5, WlRegion::ADD)
            .arg_int(10).arg_int(10).arg_int(100).arg_int(100)).unwrap();
        server.tick();

        assert!(server.connections[0].registry.get(5).is_some());
        assert_eq!(server.connections[0].registry.get(5).unwrap().interface, iface::WL_REGION);
    }

    #[test]
    fn surface_frame_callback() {
        let mut server = new_server();
        let (s_tx, mut c_tx) = create_transport_pair();
        {
            let conn = server.accept(Box::new(s_tx));
            conn.flush().unwrap();
        }
        for _ in 0..5 { let _ = c_tx.receive().unwrap(); }

        // Bind compositor at ID 3, create surface at ID 4
        c_tx.send(&WaylandMessage::new(WL_REGISTRY_ID, WlRegistry::BIND)
            .arg_uint(1).arg_new_id(3).arg_string("wl_compositor").arg_uint(4)).unwrap();
        server.tick();
        c_tx.send(&WaylandMessage::new(3, WlCompositor::CREATE_SURFACE)
            .arg_new_id(4)).unwrap();
        server.tick();

        // Request frame callback
        c_tx.send(&WaylandMessage::new(4, WlSurface::FRAME)
            .arg_new_id(100)).unwrap();
        server.tick();

        // Should receive wl_callback.done
        let response = c_tx.receive().unwrap();
        assert_eq!(response.object_id, 100);
        assert_eq!(response.opcode, WlCallback::DONE);
    }
}
