//! # bt-tool — Bluetooth Control CLI for GergiOS
//!
//! Phase 8.6: Command-line tool for interacting with the Bluetooth daemon.
//!
//! Usage:
//!   bt-tool scan              Start device discovery
//!   bt-tool scan-stop         Stop device discovery
//!   bt-tool devices           List discovered devices
//!   bt-tool connect <addr>    Connect to a device (XX:XX:XX:XX:XX:XX)
//!   bt-tool disconnect <hdl>  Disconnect by HCI handle
//!   bt-tool connections       List active connections
//!   bt-tool name <name>       Set local device name
//!   bt-tool discoverable <on|off>  Set discoverable mode
//!   bt-tool connectable <on|off>   Set connectable mode
//!   bt-tool status            Show daemon status
//!   bt-tool pair <addr>       Pair with a device
//!   bt-tool unpair <addr>     Unpair a device
//!   bt-tool -h                Show this help
//!
//! Communication:
//!   - Finds bluetoothd via DS label lookup
//!   - Sends BT_RQ_* IPC messages
//!   - Prints formatted output

mod commands;

use std::env;
use std::process::exit;

fn main() {
    let args: Vec<String> = env::args().collect();
    let progname = &args[0];

    if args.len() < 2 {
        commands::usage(progname);
        exit(1);
    }

    let cmd = &args[1];

    let result = match cmd.as_str() {
        "scan" => commands::cmd_scan(),
        "scan-stop" => commands::cmd_scan_stop(),
        "devices" => commands::cmd_devices(),
        "connect" => {
            if args.len() < 3 {
                eprintln!("Usage: {} connect <bdaddr>", progname);
                exit(1);
            }
            commands::cmd_connect(&args[2])
        }
        "disconnect" => {
            if args.len() < 3 {
                eprintln!("Usage: {} disconnect <handle>", progname);
                exit(1);
            }
            commands::cmd_disconnect(&args[2])
        }
        "connections" => commands::cmd_connections(),
        "name" => {
            if args.len() < 3 {
                eprintln!("Usage: {} name <name>", progname);
                exit(1);
            }
            commands::cmd_set_name(&args[2])
        }
        "discoverable" => {
            if args.len() < 3 {
                eprintln!("Usage: {} discoverable <on|off>", progname);
                exit(1);
            }
            commands::cmd_set_discoverable(&args[2])
        }
        "connectable" => {
            if args.len() < 3 {
                eprintln!("Usage: {} connectable <on|off>", progname);
                exit(1);
            }
            commands::cmd_set_connectable(&args[2])
        }
        "status" => commands::cmd_status(),
        "pair" => {
            if args.len() < 3 {
                eprintln!("Usage: {} pair <bdaddr>", progname);
                exit(1);
            }
            commands::cmd_pair(&args[2])
        }
        "unpair" => {
            if args.len() < 3 {
                eprintln!("Usage: {} unpair <bdaddr>", progname);
                exit(1);
            }
            commands::cmd_unpair(&args[2])
        }
        "register-service" => {
            if args.len() < 6 {
                eprintln!("Usage: {} register-service <uuid_hex> <psm_hex> <channel> <name>", progname);
                exit(1);
            }
            commands::cmd_register_service(&args[2], &args[3], &args[4], &args[5])
        }
        "-h" | "--help" => {
            commands::usage(progname);
            0
        }
        _ => {
            eprintln!("{}: unknown command '{}'", progname, cmd);
            commands::usage(progname);
            1
        }
    };

    exit(result);
}
