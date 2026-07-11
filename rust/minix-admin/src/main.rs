//! # minix-admin — GergiOS Admin Shell
//!
//! Unified administration tool for GergiOS.
//!
//! ## Usage
//!
//! ```text
//! minix-admin services list              — list all services
//! minix-admin services status <name>     — service status
//! minix-admin services start <name>      — start a service
//! minix-admin services stop <name>       — stop a service
//! minix-admin services restart <name>    — restart a service
//! minix-admin system info               — system information
//! minix-admin system cpu                — CPU usage
//! minix-admin system memory             — memory usage
//! minix-admin system disk               — disk usage
//! minix-admin system uptime             — system uptime
//! minix-admin help                      — this help
//! ```

mod cli;
mod dashboard;
mod network;
mod security;
mod services;
mod shell;
mod system;

use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    let prog = if args.is_empty() { "minix-admin" } else { &args[0] };

    if args.len() < 2 {
        // No args — enter interactive shell
        match shell::Shell::run() {
            Ok(()) => process::exit(0),
            Err(e) => {
                eprintln!("{}: TUI error: {}", prog, e);
                process::exit(1);
            }
        }
    }

    // Check for TUI/interactive flags
    match args[1].as_str() {
        "--tui" | "-t" | "--interactive" | "-i" | "shell" | "--dashboard" | "-d" | "dashboard" | "dash" => {
            // Check for dashboard mode
            if args[1] == "--dashboard" || args[1] == "-d" || args[1] == "dashboard" || args[1] == "dash" {
                match dashboard::Dashboard::run() {
                    Ok(()) => process::exit(0),
                    Err(e) => {
                        eprintln!("{}: Dashboard error: {}", prog, e);
                        process::exit(1);
                    }
                }
            }
            match shell::Shell::run() {
                Ok(()) => process::exit(0),
                Err(e) => {
                    eprintln!("{}: TUI error: {}", prog, e);
                    process::exit(1);
                }
            }
        }
        _ => {}
    }

    let command = &args[1];
    let subargs = &args[2..];

    let result = match command.as_str() {
        "services" => handle_services(subargs),
        "system" => handle_system(subargs),
        "network" | "net" => handle_network(subargs),
        "security" | "sec" => handle_security(subargs),
        "help" | "-h" | "--help" => {
            cli::print_usage(prog);
            Ok(())
        }
        "version" | "-V" | "--version" => {
            println!("minix-admin v{}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        _ => {
            eprintln!("{}: unknown command '{}'. Try '{} help'.", prog, command, prog);
            process::exit(1);
        }
    };

    if let Err(e) = result {
        eprintln!("{}: error: {}", prog, e);
        process::exit(1);
    }
}

fn handle_services(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return Err(format!("Usage: services <list|status|start|stop|restart> [name]"));
    }

    match args[0].as_str() {
        "list" => services::list_services(),
        "status" => {
            if args.len() < 2 {
                // Show status of all services
                services::list_services()
            } else {
                services::service_status(&args[1])
            }
        }
        "start" => {
            if args.len() < 2 {
                return Err("Usage: services start <name>".to_string());
            }
            services::start_service(&args[1])
        }
        "stop" => {
            if args.len() < 2 {
                return Err("Usage: services stop <name>".to_string());
            }
            services::stop_service(&args[1])
        }
        "restart" => {
            if args.len() < 2 {
                return Err("Usage: services restart <name>".to_string());
            }
            services::restart_service(&args[1])
        }
        _ => Err(format!("Unknown service command '{}'. Try 'services list'.", args[0])),
    }
}

fn handle_security(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        // Default: show all security info
        security::mac_status()?;
        println!();
        security::audit_status()?;
        Ok(())
    } else {
        match args[0].as_str() {
            "mac" => handle_mac(&args[1..]),
            "caps" | "capabilities" => handle_caps(&args[1..]),
            "audit" => handle_audit(&args[1..]),
            _ => Err(format!("Unknown security command '{}'. Try 'security mac status'.", args[0])),
        }
    }
}

fn handle_mac(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return security::mac_status();
    }
    match args[0].as_str() {
        "status" | "show" => security::mac_status(),
        "enable" | "on" => security::mac_enable(),
        "disable" | "off" => security::mac_disable(),
        "rules" => security::mac_show_rules(),
        _ => Err(format!("Unknown mac command '{}'. Try 'mac status'.", args[0])),
    }
}

fn handle_caps(args: &[String]) -> Result<(), String> {
    if args.is_empty() || args[0] == "help" {
        return Err("Usage: caps list <pid> | caps set <pid> <capability>".to_string());
    }
    match args[0].as_str() {
        "list" | "show" => {
            if args.len() < 2 {
                return Err("Usage: caps list <pid>".to_string());
            }
            let pid: i32 = args[1].parse()
                .map_err(|_| format!("Invalid pid '{}'", args[1]))?;
            security::caps_list(pid)
        }
        "set" => {
            if args.len() < 3 {
                return Err("Usage: caps set <pid> <capability>".to_string());
            }
            let pid: i32 = args[1].parse()
                .map_err(|_| format!("Invalid pid '{}'", args[1]))?;
            security::caps_set(pid, &args[2])
        }
        _ => Err(format!("Unknown caps command '{}'. Try 'caps list <pid>'.", args[0])),
    }
}

fn handle_audit(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return security::audit_status();
    }
    match args[0].as_str() {
        "status" | "show" => security::audit_status(),
        "enable" | "on" => security::audit_enable(),
        "disable" | "off" => security::audit_disable(),
        "events" | "logs" => {
            let limit = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(20);
            security::audit_events(limit)
        }
        "stats" | "statistics" => security::audit_stats(),
        _ => Err(format!("Unknown audit command '{}'. Try 'audit status'.", args[0])),
    }
}

fn handle_network(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        // Default: show all interfaces
        network::show_interfaces()
    } else {
        match args[0].as_str() {
            "interfaces" | "list" | "ls" => network::show_interfaces(),
            "status" | "iface" | "show" => {
                if args.len() < 2 {
                    network::show_interfaces()
                } else {
                    network::show_iface(&args[1])
                }
            }
            "stats" | "statistics" => network::show_stats(),
            "route" | "routes" | "routing" => network::show_route(),
            "arp" => network::show_arp(),
            _ => Err(format!("Unknown network command '{}'. Try 'network interfaces'.", args[0])),
        }
    }
}

fn handle_system(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        // Default: show all system info
        system::show_system_info()
    } else {
        match args[0].as_str() {
            "info" => system::show_system_info(),
            "cpu" => system::show_cpu(),
            "memory" | "mem" => system::show_memory(),
            "disk" => system::show_disk(),
            "uptime" => system::show_uptime(),
            _ => Err(format!("Unknown system command '{}'. Try 'system info'.", args[0])),
        }
    }
}
