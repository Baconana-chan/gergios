//! # bt-tool Command Implementations
//!
//! Each command function sends a BT_RQ_* IPC message to the daemon
//! and prints the formatted result.

use minix_rs::{self, Endpoint, Message};

/// DS label for the Bluetooth daemon.
const BT_DAEMON_LABEL: &str = "bluetoothd";

/// Resolve the bluetoothd endpoint via DS (data store).
fn find_daemon() -> Option<Endpoint> {
    // On MINIX, we look up the daemon via DS.
    // On host, return None (stub).
    #[cfg(target_os = "minix")]
    {
        // Use ds_retrieve_label_endpt via FFI
        let mut ep: Endpoint = 0;
        let r = unsafe { ds_retrieve_label_endpt(BT_DAEMON_LABEL.as_ptr(), &mut ep) };
        if r == 0 {
            return Some(ep);
        }
        eprintln!("bt-tool: bluetoothd not found (DS error {})", r);
        None
    }

    #[cfg(not(target_os = "minix"))]
    {
        eprintln!("bt-tool: bluetoothd not available on host");
        None
    }
}

/// External FFI for DS lookup.
#[cfg(target_os = "minix")]
extern "C" {
    fn ds_retrieve_label_endpt(label: *const u8, ep: *mut Endpoint) -> i32;
}

/// BT daemon message base (0x1D00 from minix/com.h).
const BT_RQ_BASE: i32 = 0x1D00;

/// Send a command to the daemon and receive a reply.
fn bt_sendrec(msg: &mut Message) -> i32 {
    if let Some(ep) = find_daemon() {
        let nr = msg.m_type;
        let r = minix_rs::sendrec(ep, nr, msg);
        if r != 0 {
            eprintln!("bt-tool: IPC error: {}", r);
            return 1;
        }
        if msg.m_type != 0 {
            eprintln!("bt-tool: daemon error: {}", -msg.m_type);
            return 1;
        }
        0
    } else {
        1
    }
}

/// Print usage information.
pub fn usage(progname: &str) {
    eprintln!(
        "Usage:
  {} scan                Start device discovery
  {} scan-stop           Stop device discovery
  {} devices             List discovered devices
  {} connect <bdaddr>    Connect to a device (XX:XX:XX:XX:XX:XX)
  {} disconnect <handle>  Disconnect by HCI handle
  {} connections         List active connections
  {} name <name>         Set local device name
  {} discoverable <on|off>  Set discoverable mode
  {} connectable <on|off>   Set connectable mode
  {} status              Show daemon status
  {} pair <bdaddr>       Pair with a device
  {} unpair <bdaddr>     Unpair a device
  {} -h                  Show this help",
        progname, progname, progname, progname, progname,
        progname, progname, progname, progname, progname,
        progname, progname, progname,
    );
}

// ── Commands ────────────────────────────────────────────────────────────

pub fn cmd_scan() -> i32 {
    let mut msg = Message::new();
    msg.set_type(BT_RQ_BASE + 0);
    bt_sendrec(&mut msg)
}

pub fn cmd_scan_stop() -> i32 {
    let mut msg = Message::new();
    msg.set_type(BT_RQ_BASE + 1);
    bt_sendrec(&mut msg)
}

pub fn cmd_devices() -> i32 {
    let mut msg = Message::new();
    msg.set_type(BT_RQ_BASE + 2);
    msg.write_i32(8, 32); // max devices
    let r = bt_sendrec(&mut msg);
    if r == 0 {
        let count = msg.read_i32(8); // m4_l1 = device count
        println!("Known devices: {}", count);
        // TODO: iterate via grant when daemon supports it
    }
    r
}

pub fn cmd_connect(bdaddr_str: &str) -> i32 {
    let bdaddr = parse_bdaddr(bdaddr_str);
    if bdaddr.is_none() {
        return 1;
    }
    let (low, high) = bdaddr.unwrap();

    let mut msg = Message::new();
    msg.set_type(BT_RQ_BASE + 3);
    msg.write_i32(8, low as i32);   // BD_ADDR low 32 bits
    msg.write_i32(16, high as i32); // BD_ADDR high 16 bits
    bt_sendrec(&mut msg)
}

pub fn cmd_disconnect(handle_str: &str) -> i32 {
    let handle: u16 = match u16::from_str_radix(handle_str, 16) {
        Ok(h) => h,
        Err(_) => {
            eprintln!("bt-tool: invalid handle '{}'", handle_str);
            return 1;
        }
    };

    let mut msg = Message::new();
    msg.set_type(BT_RQ_BASE + 4);
    msg.write_i32(16, (handle as i32) << 16);
    msg.write_i32(24, 0x13); // reason = Remote User Terminated
    bt_sendrec(&mut msg)
}

pub fn cmd_connections() -> i32 {
    let mut msg = Message::new();
    msg.set_type(BT_RQ_BASE + 5);
    msg.write_i32(8, 16); // max connections
    let r = bt_sendrec(&mut msg);
    if r == 0 {
        let count = msg.read_i32(8);
        println!("Active connections: {}", count);
    }
    r
}

pub fn cmd_set_name(name: &str) -> i32 {
    let mut msg = Message::new();
    msg.set_type(BT_RQ_BASE + 6);
    // Copy name into payload starting at offset 32
    let name_bytes = name.as_bytes();
    let len = name_bytes.len().min(47);
    msg.payload[32..32 + len].copy_from_slice(&name_bytes[..len]);
    msg.payload[32 + len] = 0; // null-terminate
    bt_sendrec(&mut msg)
}

pub fn cmd_set_discoverable(val: &str) -> i32 {
    let enable = matches!(val, "on" | "1" | "true");
    let mut msg = Message::new();
    msg.set_type(BT_RQ_BASE + 7);
    msg.write_i32(8, enable as i32);
    bt_sendrec(&mut msg)
}

pub fn cmd_set_connectable(val: &str) -> i32 {
    let enable = matches!(val, "on" | "1" | "true");
    let mut msg = Message::new();
    msg.set_type(BT_RQ_BASE + 8);
    msg.write_i32(8, enable as i32);
    bt_sendrec(&mut msg)
}

pub fn cmd_status() -> i32 {
    let mut msg = Message::new();
    msg.set_type(BT_RQ_BASE + 9);
    let r = bt_sendrec(&mut msg);
    if r == 0 {
        let running = msg.read_i32(8);
        let num_devices = msg.read_i32(16);
        let num_connections = msg.read_i32(24);
        println!("Bluetooth daemon status:");
        println!("  Running:     {}", running);
        println!("  Devices:     {}", num_devices);
        println!("  Connections: {}", num_connections);
    }
    r
}

pub fn cmd_pair(bdaddr_str: &str) -> i32 {
    let bdaddr = parse_bdaddr(bdaddr_str);
    if bdaddr.is_none() {
        return 1;
    }
    let (low, high) = bdaddr.unwrap();

    let mut msg = Message::new();
    msg.set_type(BT_RQ_BASE + 10);
    msg.write_i32(8, low as i32);
    msg.write_i32(16, high as i32);
    bt_sendrec(&mut msg)
}

pub fn cmd_unpair(bdaddr_str: &str) -> i32 {
    let bdaddr = parse_bdaddr(bdaddr_str);
    if bdaddr.is_none() {
        return 1;
    }
    let (low, high) = bdaddr.unwrap();

    let mut msg = Message::new();
    msg.set_type(BT_RQ_BASE + 11);
    msg.write_i32(8, low as i32);
    msg.write_i32(16, high as i32);
    bt_sendrec(&mut msg)
}

/// Parse a BD_ADDR string ("XX:XX:XX:XX:XX:XX") into low/high parts.
fn parse_bdaddr(s: &str) -> Option<(u32, u16)> {
    let mut bytes = [0u8; 6];
    let mut i = 0;

    for octet in s.split(':') {
        if i >= 6 {
            eprintln!("bt-tool: invalid BD_ADDR '{}'", s);
            return None;
        }
        bytes[i] = u8::from_str_radix(octet, 16).ok()?;
        i += 1;
    }
    if i != 6 {
        eprintln!("bt-tool: invalid BD_ADDR '{}'", s);
        return None;
    }

    let low = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let high = u16::from_le_bytes([bytes[4], bytes[5]]);

    Some((low, high))
}
