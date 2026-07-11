//! # CLI — Command Parser and Help System

/// Print the main usage message.
pub fn print_usage(prog: &str) {
    println!("GergiOS Admin Shell v{}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("Usage: {} <command> [subcommand] [args...]", prog);
    println!("       {}                        # Interactive TUI shell", prog);
    println!("       {} --tui                   # Interactive TUI shell", prog);
    println!();
    println!("Commands:");
    println!("  services        Service management (list, status, start, stop, restart)");
    println!("  system          System monitoring (info, cpu, memory, disk, uptime)");
    println!("  network         Network management (interfaces, stats, route, arp)");
    println!("  security        Security management (mac, caps, audit)");
    println!("  dashboard       Real-time TUI monitoring dashboard");
    println!("  help            Show this help message");
    println!("  version         Show version");
    println!();
    println!("Interactive Shell: ");
    println!("  {}                  Start TUI shell (no args)", prog);
    println!("  {} --tui              Start TUI shell", prog);
    println!("  {} shell              Run shell mode", prog);
    println!("  {} --dashboard        Start TUI dashboard", prog);
    println!("  {} dashboard          Start TUI dashboard", prog);
    println!();
    println!("Examples:");
    println!("  {} services list", prog);
    println!("  {} services status vfs", prog);
    println!("  {} services restart bluetoothd", prog);
    println!("  {} system info", prog);
    println!("  {} system disk", prog);
    println!("  {} network interfaces", prog);
    println!("  {} network stats", prog);
    println!("  {} network route", prog);
    println!("  {} network arp", prog);
    println!("  {} security mac status", prog);
    println!("  {} security caps list 100", prog);
    println!("  {} security audit events 10", prog);
    println!("  {} dashboard           Real-time monitoring dashboard", prog);
    println!();
    println!("For detailed help on a command: {} <command> help", prog);
}
