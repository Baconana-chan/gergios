//! # Wayland Protocol Definitions
//!
//! Protocol constants (opcodes, events) and object management for
//! the core Wayland protocols (wl_display, wl_compositor, xdg_shell,
//! wl_seat, wl_shm).
//!
//! These constants are derived from the Wayland XML protocol
//! specifications:
//! - wayland.xml (core protocol, version 1)
//! - xdg-shell.xml (stable xdg_shell, version 6)
//!
//! ## Object ID Convention
//!
//! - 1: wl_display (singleton, created by the server)
//! - 2: wl_registry (per-connection, created by server)
//! - 3+: dynamically allocated for all other objects

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::fmt;

// ════════════════════════════════════════════════════════════════
// Protocol Constants
// ════════════════════════════════════════════════════════════════

/// wl_display singleton object ID.
pub const WL_DISPLAY_ID: u32 = 1;
/// wl_registry object ID (per-connection).
pub const WL_REGISTRY_ID: u32 = 2;

/// Next available object ID for dynamic allocation.
pub const FIRST_DYNAMIC_ID: u32 = 3;

// ── wl_display opcodes ────────────────────────────────────────

impl WlDisplay {
    /// wl_display.error event
    pub const ERROR: u16 = 0;
    /// wl_display.delete_id event
    pub const DELETE_ID: u16 = 1;

    // Requests (client → server)
    /// wl_display.sync request (returns wl_callback)
    pub const SYNC: u16 = 0;
    /// wl_display.get_registry request (returns wl_registry)
    pub const GET_REGISTRY: u16 = 1;
}

// ── wl_registry opcodes ───────────────────────────────────────

impl WlRegistry {
    /// wl_registry.global event
    pub const GLOBAL: u16 = 0;
    /// wl_registry.global_remove event
    pub const GLOBAL_REMOVE: u16 = 1;

    // Requests
    /// wl_registry.bind request
    pub const BIND: u16 = 0;
}

// ── wl_callback opcodes ───────────────────────────────────────

impl WlCallback {
    /// wl_callback.done event
    pub const DONE: u16 = 0;
}

// ── wl_compositor opcodes ─────────────────────────────────────

impl WlCompositor {
    /// wl_compositor.create_surface request
    pub const CREATE_SURFACE: u16 = 0;
    /// wl_compositor.create_region request
    pub const CREATE_REGION: u16 = 1;
}

// ── wl_surface opcodes ────────────────────────────────────────

impl WlSurface {
    /// wl_surface.enter event
    pub const ENTER: u16 = 0;
    /// wl_surface.leave event
    pub const LEAVE: u16 = 1;
    /// wl_surface.preferred_buffer_scale event
    pub const PREFERRED_BUFFER_SCALE: u16 = 2;
    /// wl_surface.preferred_buffer_transform event
    pub const PREFERRED_BUFFER_TRANSFORM: u16 = 3;

    // Requests
    /// wl_surface.destroy request
    pub const DESTROY: u16 = 0;
    /// wl_surface.attach request
    pub const ATTACH: u16 = 1;
    /// wl_surface.damage request
    pub const DAMAGE: u16 = 2;
    /// wl_surface.frame request (returns wl_callback)
    pub const FRAME: u16 = 3;
    /// wl_surface.set_opaque_region request
    pub const SET_OPAQUE_REGION: u16 = 4;
    /// wl_surface.set_input_region request
    pub const SET_INPUT_REGION: u16 = 5;
    /// wl_surface.commit request
    pub const COMMIT: u16 = 6;
    /// wl_surface.set_buffer_transform request
    pub const SET_BUFFER_TRANSFORM: u16 = 7;
    /// wl_surface.set_buffer_scale request
    pub const SET_BUFFER_SCALE: u16 = 8;
    /// wl_surface.damage_buffer request
    pub const DAMAGE_BUFFER: u16 = 9;
    /// wl_surface.offset request
    pub const OFFSET: u16 = 10;
}

// ── wl_region opcodes ─────────────────────────────────────────

impl WlRegion {
    // Requests
    /// wl_region.destroy request
    pub const DESTROY: u16 = 0;
    /// wl_region.add request
    pub const ADD: u16 = 1;
    /// wl_region.subtract request
    pub const SUBTRACT: u16 = 2;
}

// ── wl_seat opcodes ───────────────────────────────────────────

impl WlSeat {
    /// wl_seat.capabilities event
    pub const CAPABILITIES: u16 = 0;
    /// wl_seat.name event
    pub const NAME: u16 = 1;

    // Requests
    /// wl_seat.get_pointer request
    pub const GET_POINTER: u16 = 0;
    /// wl_seat.get_keyboard request
    pub const GET_KEYBOARD: u16 = 1;
    /// wl_seat.get_touch request
    pub const GET_TOUCH: u16 = 2;
    /// wl_seat.release request
    pub const RELEASE: u16 = 3;
}

// ── wl_pointer opcodes ────────────────────────────────────────

impl WlPointer {
    // Events
    /// wl_pointer.enter event
    pub const ENTER: u16 = 0;
    /// wl_pointer.leave event
    pub const LEAVE: u16 = 1;
    /// wl_pointer.motion event
    pub const MOTION: u16 = 2;
    /// wl_pointer.button event
    pub const BUTTON: u16 = 3;
    /// wl_pointer.axis event
    pub const AXIS: u16 = 4;
    /// wl_pointer.frame event
    pub const FRAME: u16 = 5;
    /// wl_pointer.axis_source event
    pub const AXIS_SOURCE: u16 = 6;
    /// wl_pointer.axis_stop event
    pub const AXIS_STOP: u16 = 7;
    /// wl_pointer.axis_discrete event
    pub const AXIS_DISCRETE: u16 = 8;
    /// wl_pointer.axis_value120 event
    pub const AXIS_VALUE120: u16 = 9;
    /// wl_pointer.axis_relative_direction event
    pub const AXIS_RELATIVE_DIRECTION: u16 = 10;

    // Requests
    /// wl_pointer.set_cursor request
    pub const SET_CURSOR: u16 = 0;
    /// wl_pointer.release request
    pub const RELEASE: u16 = 1;

    // Button state
    pub const BUTTON_STATE_RELEASED: u32 = 0;
    pub const BUTTON_STATE_PRESSED: u32 = 1;

    // Axis
    pub const AXIS_VERTICAL_SCROLL: u32 = 0;
    pub const AXIS_HORIZONTAL_SCROLL: u32 = 1;
}

// ── wl_keyboard opcodes ───────────────────────────────────────

impl WlKeyboard {
    // Events
    /// wl_keyboard.keymap event
    pub const KEYMAP: u16 = 0;
    /// wl_keyboard.enter event
    pub const ENTER: u16 = 1;
    /// wl_keyboard.leave event
    pub const LEAVE: u16 = 2;
    /// wl_keyboard.key event
    pub const KEY: u16 = 3;
    /// wl_keyboard.modifiers event
    pub const MODIFIERS: u16 = 4;
    /// wl_keyboard.repeat_info event
    pub const REPEAT_INFO: u16 = 5;

    // Requests
    /// wl_keyboard.release request
    pub const RELEASE: u16 = 0;
}

// ── wl_shm opcodes ────────────────────────────────────────────

impl WlShm {
    /// wl_shm.format event
    pub const FORMAT: u16 = 0;

    // Requests
    /// wl_shm.create_pool request
    pub const CREATE_POOL: u16 = 0;

    // Format constants
    pub const FORMAT_ARGB8888: u32 = 0;
    pub const FORMAT_XRGB8888: u32 = 1;
}

// ── wl_data_device_manager opcodes ──────────────────────────

impl WlDataDeviceManager {
    // Requests
    /// wl_data_device_manager.create_data_source request
    pub const CREATE_DATA_SOURCE: u16 = 0;
    /// wl_data_device_manager.get_data_device request
    pub const GET_DATA_DEVICE: u16 = 1;
}

// ── wl_data_device opcodes ───────────────────────────────────

impl WlDataDevice {
    // Events
    /// wl_data_device.data_offer event
    pub const DATA_OFFER: u16 = 0;
    /// wl_data_device.enter event (drag-and-drop)
    pub const ENTER: u16 = 1;
    /// wl_data_device.leave event (drag-and-drop)
    pub const LEAVE: u16 = 2;
    /// wl_data_device.motion event (drag-and-drop)
    pub const MOTION: u16 = 3;
    /// wl_data_device.drop event (drag-and-drop)
    pub const DROP: u16 = 4;
    /// wl_data_device.selection event (clipboard)
    pub const SELECTION: u16 = 5;

    // Requests
    /// wl_data_device.start_drag request
    pub const START_DRAG: u16 = 0;
    /// wl_data_device.set_selection request (clipboard)
    pub const SET_SELECTION: u16 = 1;
    /// wl_data_device.release request
    pub const RELEASE: u16 = 2;
}

// ── wl_data_source opcodes ───────────────────────────────────

impl WlDataSource {
    // Events
    /// wl_data_source.send event (compositor asks for data)
    pub const SEND: u16 = 0;
    /// wl_data_source.cancelled event
    pub const CANCELLED: u16 = 1;
    /// wl_data_source.dnd_drop_performed event
    pub const DND_DROP_PERFORMED: u16 = 2;
    /// wl_data_source.dnd_finished event
    pub const DND_FINISHED: u16 = 3;
    /// wl_data_source.action event
    pub const ACTION: u16 = 4;

    // Requests
    /// wl_data_source.offer request (advertise MIME type)
    pub const OFFER: u16 = 0;
    /// wl_data_source.destroy request
    pub const DESTROY: u16 = 1;
}

// ── wl_data_offer opcodes ────────────────────────────────────

impl WlDataOffer {
    // Events
    /// wl_data_offer.offer event (advertise available MIME type)
    pub const OFFER: u16 = 0;
    /// wl_data_offer.source_actions event
    pub const SOURCE_ACTIONS: u16 = 1;
    /// wl_data_offer.action event
    pub const ACTION: u16 = 2;

    // Requests
    /// wl_data_offer.accept request
    pub const ACCEPT: u16 = 0;
    /// wl_data_offer.receive request (accept MIME type, receive fd)
    pub const RECEIVE: u16 = 1;
    /// wl_data_offer.destroy request
    pub const DESTROY: u16 = 2;
    /// wl_data_offer.finish request (version 3+)
    pub const FINISH: u16 = 3;
    /// wl_data_offer.set_actions request (version 3+)
    pub const SET_ACTIONS: u16 = 4;
}

// ── wl_buffer opcodes ─────────────────────────────────────────

impl WlBuffer {
    /// wl_buffer.release event
    pub const RELEASE: u16 = 0;
}

// ── xdg_wm_base opcodes ───────────────────────────────────────

impl XdgWmBase {
    /// xdg_wm_base.ping event
    pub const PING: u16 = 0;

    // Requests
    /// xdg_wm_base.destroy request
    pub const DESTROY: u16 = 0;
    /// xdg_wm_base.create_positioner request
    pub const CREATE_POSITIONER: u16 = 1;
    /// xdg_wm_base.get_xdg_surface request
    pub const GET_XDG_SURFACE: u16 = 2;
    /// xdg_wm_base.pong request
    pub const PONG: u16 = 3;
}

// ── xdg_popup opcodes ─────────────────────────────────────────

impl XdgPopup {
    // Events
    /// xdg_popup.configure event
    pub const CONFIGURE: u16 = 0;
    /// xdg_popup.popup_done event
    pub const POPUP_DONE: u16 = 1;
    /// xdg_popup.repositioned event
    pub const REPOSITIONED: u16 = 2;

    // Requests
    /// xdg_popup.destroy request
    pub const DESTROY: u16 = 0;
    /// xdg_popup.grab request
    pub const GRAB: u16 = 1;
    /// xdg_popup.reposition request
    pub const REPOSITION: u16 = 2;
}

// ── xdg_positioner opcodes ─────────────────────────────────────

impl XdgPositioner {
    // Requests
    /// xdg_positioner.destroy request
    pub const DESTROY: u16 = 0;
    /// xdg_positioner.set_size request
    pub const SET_SIZE: u16 = 1;
    /// xdg_positioner.set_anchor_rect request
    pub const SET_ANCHOR_RECT: u16 = 2;
    /// xdg_positioner.set_anchor request
    pub const SET_ANCHOR: u16 = 3;
    /// xdg_positioner.set_gravity request
    pub const SET_GRAVITY: u16 = 4;
    /// xdg_positioner.set_constraint_adjustment request
    pub const SET_CONSTRAINT_ADJUSTMENT: u16 = 5;
    /// xdg_positioner.set_offset request
    pub const SET_OFFSET: u16 = 6;
    /// xdg_positioner.set_reactive request
    pub const SET_REACTIVE: u16 = 7;
    /// xdg_positioner.set_parent_size request
    pub const SET_PARENT_SIZE: u16 = 8;
    /// xdg_positioner.set_parent_configure request
    pub const SET_PARENT_CONFIGURE: u16 = 9;
}

// ── xdg_surface opcodes ───────────────────────────────────────

impl XdgSurface {
    /// xdg_surface.configure event
    pub const CONFIGURE: u16 = 0;

    // Requests
    /// xdg_surface.destroy request
    pub const DESTROY: u16 = 0;
    /// xdg_surface.get_toplevel request
    pub const GET_TOPLEVEL: u16 = 1;
    /// xdg_surface.get_popup request
    pub const GET_POPUP: u16 = 2;
    /// xdg_surface.set_window_geometry request
    pub const SET_WINDOW_GEOMETRY: u16 = 3;
    /// xdg_surface.ack_configure request
    pub const ACK_CONFIGURE: u16 = 4;
}

// ── xdg_toplevel opcodes ──────────────────────────────────────

impl XdgToplevel {
    // Events
    /// xdg_toplevel.configure event
    pub const CONFIGURE: u16 = 0;
    /// xdg_toplevel.close event
    pub const CLOSE: u16 = 1;
    /// xdg_toplevel.configure_bounds event
    pub const CONFIGURE_BOUNDS: u16 = 2;
    /// xdg_toplevel.wm_capabilities event
    pub const WM_CAPABILITIES: u16 = 3;

    // Requests
    /// xdg_toplevel.destroy request
    pub const DESTROY: u16 = 0;
    /// xdg_toplevel.set_parent request
    pub const SET_PARENT: u16 = 1;
    /// xdg_toplevel.set_title request
    pub const SET_TITLE: u16 = 2;
    /// xdg_toplevel.set_app_id request
    pub const SET_APP_ID: u16 = 3;
    /// xdg_toplevel.show_window_menu request
    pub const SHOW_WINDOW_MENU: u16 = 4;
    /// xdg_toplevel.move request
    pub const MOVE: u16 = 5;
    /// xdg_toplevel.resize request
    pub const RESIZE: u16 = 6;
    /// xdg_toplevel.set_min_size request
    pub const SET_MIN_SIZE: u16 = 7;
    /// xdg_toplevel.set_max_size request
    pub const SET_MAX_SIZE: u16 = 8;
    /// xdg_toplevel.set_maximized request
    pub const SET_MAXIMIZED: u16 = 9;
    /// xdg_toplevel.unset_maximized request
    pub const UNSET_MAXIMIZED: u16 = 10;
    /// xdg_toplevel.set_fullscreen request
    pub const SET_FULLSCREEN: u16 = 11;
    /// xdg_toplevel.unset_fullscreen request
    pub const UNSET_FULLSCREEN: u16 = 12;
    /// xdg_toplevel.set_minimized request
    pub const SET_MINIMIZED: u16 = 13;
}

// ════════════════════════════════════════════════════════════════
// Object Management
// ════════════════════════════════════════════════════════════════

/// A generic Wayland protocol object on the server side.
///
/// Each object has an ID, a type tag, and optional implementation-
/// specific data stored as a type-erased box.
pub struct WlObject {
    /// The object's unique ID.
    pub id: u32,
    /// The interface name (e.g., "wl_surface").
    /// Stored as `String` because names from the wire protocol are runtime values.
    pub interface: alloc::string::String,
    /// The version of this interface.
    pub version: u32,
    /// Implementation-specific data.
    pub data: Option<alloc::boxed::Box<dyn core::any::Any>>,
}

impl fmt::Debug for WlObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WlObject")
            .field("id", &self.id)
            .field("interface", &self.interface)
            .field("version", &self.version)
            .finish()
    }
}

impl WlObject {
    pub fn new(id: u32, interface: &str, version: u32) -> Self {
        Self {
            id,
            interface: alloc::string::String::from(interface),
            version,
            data: None,
        }
    }

    pub fn with_data<T: 'static>(id: u32, interface: &str, version: u32, data: T) -> Self {
        Self {
            id,
            interface: alloc::string::String::from(interface),
            version,
            data: Some(alloc::boxed::Box::new(data)),
        }
    }
}

/// Registry of all Wayland protocol objects for a server connection.
///
/// Maps object IDs to their protocol objects. Each connection has
/// its own registry (since object IDs are per-connection).
pub struct ObjectRegistry {
    objects: BTreeMap<u32, WlObject>,
    next_id: u32,
}

impl ObjectRegistry {
    /// Create a new object registry with the built-in server objects.
    ///
    /// Pre-populates:
    /// - wl_display (id=1)
    /// - wl_registry (id=2)
    pub fn new() -> Self {
        let mut objects = BTreeMap::new();

        // wl_display (singleton, always exists)
        objects.insert(WL_DISPLAY_ID, WlObject::new(WL_DISPLAY_ID, "wl_display", 1));

        // wl_registry will be created per-connection by the server
        // but we prepare the slot. Actually, registry is connection-local.

        Self {
            objects,
            next_id: FIRST_DYNAMIC_ID,
        }
    }

    /// Register a new object, assigning it the next available ID.
    /// Returns the assigned ID.
    pub fn register(&mut self, interface: &str, version: u32) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.objects.insert(id, WlObject::new(id, interface, version));
        id
    }

    /// Register an object with a specific ID (for new_id from client).
    pub fn register_with_id(&mut self, id: u32, interface: &str, version: u32) {
        if id >= self.next_id {
            self.next_id = id + 1;
        }
        self.objects.insert(id, WlObject::new(id, interface, version));
    }

    /// Register an object with data, auto-assigning the next available ID.
    pub fn register_with_data<T: 'static>(
        &mut self,
        interface: &str,
        version: u32,
        data: T,
    ) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.objects.insert(id, WlObject::with_data(id, interface, version, data));
        id
    }

    /// Register an object with data at a specific ID (for client-assigned new_id).
    pub fn register_with_id_and_data<T: 'static>(
        &mut self,
        id: u32,
        interface: &str,
        version: u32,
        data: T,
    ) {
        if id >= self.next_id {
            self.next_id = id + 1;
        }
        self.objects.insert(id, WlObject::with_data(id, interface, version, data));
    }

    /// Look up an object by ID.
    pub fn get(&self, id: u32) -> Option<&WlObject> {
        self.objects.get(&id)
    }

    /// Get mutable reference to an object.
    pub fn get_mut(&mut self, id: u32) -> Option<&mut WlObject> {
        self.objects.get_mut(&id)
    }

    /// Remove an object by ID.
    pub fn remove(&mut self, id: u32) -> Option<WlObject> {
        self.objects.remove(&id)
    }

    /// Get the data of an object by ID, downcast to the expected type.
    pub fn get_data<T: 'static>(&self, id: u32) -> Option<&T> {
        self.objects
            .get(&id)
            .and_then(|o| o.data.as_ref())
            .and_then(|d| d.downcast_ref::<T>())
    }

    /// Get mutable data.
    pub fn get_data_mut<T: 'static>(&mut self, id: u32) -> Option<&mut T> {
        self.objects
            .get_mut(&id)
            .and_then(|o| o.data.as_mut())
            .and_then(|d| d.downcast_mut::<T>())
    }

    /// Iterate over all objects.
    pub fn iter(&self) -> impl Iterator<Item = &WlObject> {
        self.objects.values()
    }

    /// Number of objects in the registry.
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// Get all objects of a specific interface type.
    pub fn get_by_interface(&self, interface: &str) -> Vec<&WlObject> {
        self.objects
            .values()
            .filter(|o| o.interface == interface)
            .collect()
    }
}

// ════════════════════════════════════════════════════════════════
// Interface Names
// ════════════════════════════════════════════════════════════════

/// Interface name constants.
pub mod iface {
    pub const WL_DISPLAY: &str = "wl_display";
    pub const WL_REGISTRY: &str = "wl_registry";
    pub const WL_CALLBACK: &str = "wl_callback";
    pub const WL_COMPOSITOR: &str = "wl_compositor";
    pub const WL_SURFACE: &str = "wl_surface";
    pub const WL_REGION: &str = "wl_region";
    pub const WL_SEAT: &str = "wl_seat";
    pub const WL_POINTER: &str = "wl_pointer";
    pub const WL_KEYBOARD: &str = "wl_keyboard";
    pub const WL_TOUCH: &str = "wl_touch";
    pub const WL_SHM: &str = "wl_shm";
    pub const WL_SHM_POOL: &str = "wl_shm_pool";
    pub const WL_BUFFER: &str = "wl_buffer";
    pub const WL_DATA_DEVICE_MANAGER: &str = "wl_data_device_manager";
    pub const WL_DATA_DEVICE: &str = "wl_data_device";
    pub const WL_DATA_SOURCE: &str = "wl_data_source";
    pub const WL_DATA_OFFER: &str = "wl_data_offer";
    pub const XDG_WM_BASE: &str = "xdg_wm_base";
    pub const XDG_SURFACE: &str = "xdg_surface";
    pub const XDG_TOPLEVEL: &str = "xdg_toplevel";
    pub const XDG_POPUP: &str = "xdg_popup";
    pub const XDG_POSITIONER: &str = "xdg_positioner";
}

// ════════════════════════════════════════════════════════════════
// Struct tags (used as phantom types for downcasting)
// ════════════════════════════════════════════════════════════════

pub struct WlDisplay;
pub struct WlRegistry;
pub struct WlCallback;
pub struct WlCompositor;
pub struct WlSurface;
pub struct WlRegion;
pub struct WlSeat;
pub struct WlPointer;
pub struct WlKeyboard;
pub struct WlTouch;
pub struct WlShm;
pub struct WlShmPool;
pub struct WlBuffer;
pub struct WlDataDeviceManager;
pub struct WlDataDevice;
pub struct WlDataSource;
pub struct WlDataOffer;
pub struct XdgWmBase;
pub struct XdgSurface;
pub struct XdgToplevel;
pub struct XdgPopup;
pub struct XdgPositioner;

// ════════════════════════════════════════════════════════════════
// Global Advertisement
// ════════════════════════════════════════════════════════════════

/// A global advertised to clients via wl_registry.global events.
#[derive(Debug, Clone)]
pub struct Global {
    /// Interface name (e.g., "wl_compositor").
    pub interface: &'static str,
    /// Interface version.
    pub version: u32,
    /// A unique name for this global (incremented per registration).
    pub name: u32,
}

/// Manages the list of globals advertised to clients.
pub struct GlobalRegistry {
    globals: Vec<Global>,
    next_name: u32,
}

impl GlobalRegistry {
    pub fn new() -> Self {
        Self {
            globals: Vec::new(),
            next_name: 1,
        }
    }

    /// Register a global (advertised to all clients via wl_registry).
    pub fn add(&mut self, interface: &'static str, version: u32) -> u32 {
        let name = self.next_name;
        self.next_name += 1;
        self.globals.push(Global { interface, version, name });
        name
    }

    /// Remove a global by name.
    pub fn remove(&mut self, name: u32) {
        self.globals.retain(|g| g.name != name);
    }

    /// Get all globals.
    pub fn all(&self) -> &[Global] {
        &self.globals
    }

    /// Find a global by interface name.
    pub fn find(&self, interface: &str) -> Option<&Global> {
        self.globals.iter().find(|g| g.interface == interface)
    }
}

// ════════════════════════════════════════════════════════════════
// Tests
// ════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_registry_creation() {
        let reg = ObjectRegistry::new();
        assert!(reg.get(WL_DISPLAY_ID).is_some());
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn register_and_lookup() {
        let mut reg = ObjectRegistry::new();
        let id = reg.register(iface::WL_SURFACE, 1);
        assert!(id >= FIRST_DYNAMIC_ID);
        assert!(reg.get(id).is_some());
        assert_eq!(reg.get(id).unwrap().interface, iface::WL_SURFACE);
    }

    #[test]
    fn register_with_data() {
        let mut reg = ObjectRegistry::new();
        let id = reg.register_with_data(iface::WL_SURFACE, 4, 42u32);
        assert_eq!(*reg.get_data::<u32>(id).unwrap(), 42);
    }

    #[test]
    fn remove_object() {
        let mut reg = ObjectRegistry::new();
        let id = reg.register(iface::WL_SURFACE, 1);
        assert!(reg.remove(id).is_some());
        assert!(reg.get(id).is_none());
    }

    #[test]
    fn global_registry() {
        let mut globals = GlobalRegistry::new();
        let name = globals.add("wl_compositor", 4);
        assert_eq!(name, 1);
        assert_eq!(globals.all().len(), 1);
        assert!(globals.find("wl_compositor").is_some());
        assert!(globals.find("wl_shm").is_none());
    }

    #[test]
    fn unique_ids() {
        let mut reg = ObjectRegistry::new();
        let id1 = reg.register(iface::WL_SURFACE, 1);
        let id2 = reg.register(iface::WL_SURFACE, 1);
        assert_ne!(id1, id2);
    }

    #[test]
    fn get_by_interface() {
        let mut reg = ObjectRegistry::new();
        reg.register(iface::WL_SURFACE, 1);
        reg.register(iface::WL_SURFACE, 1);
        reg.register(iface::WL_SEAT, 1);
        let surfaces = reg.get_by_interface(iface::WL_SURFACE);
        assert_eq!(surfaces.len(), 2);
    }
}
