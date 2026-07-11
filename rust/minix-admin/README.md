# minix-admin — GergiOS Admin Shell

**minix-admin** is the unified system administration tool for GergiOS (MINIX 3 compatible).
It provides a single entry point for managing services, monitoring system resources,
inspecting network state, and auditing security — all from one CLI or TUI interface.

## Features

| Feature | Command | Description |
|---------|---------|-------------|
| **Service Management** | `services list/status/start/stop/restart` | Manage system services via RS IPC |
| **System Monitor** | `system info/cpu/memory/disk/uptime` | CPU, memory, disk, uptime monitoring |
| **Network Manager** | `network interfaces/stats/route/arp` | Interfaces, statistics, routing, ARP |
| **Security Manager** | `security mac/caps/audit` | MAC enforcement, capabilities, audit |
| **TUI Dashboard** | `dashboard` | Real-time monitoring (auto-refresh 2s) |
| **Interactive Shell** | `shell` or no args | Line editing, history, tab-completion |

## Installation

```bash
# Build from source
cd rust && cargo build -p minix-admin --release

# Install binary
cp target/release/minix-admin /usr/local/sbin/

# Install man page
cp minix-admin.8 /usr/share/man/man8/
```

## Quick Start

```bash
# Interactive TUI shell (no arguments)
minix-admin

# Real-time monitoring dashboard
minix-admin dashboard

# List all services
minix-admin services list

# Check system resources
minix-admin system info
minix-admin system memory
minix-admin system disk

# Network diagnostics
minix-admin network interfaces
minix-admin network stats
minix-admin network route

# Security status
minix-admin security mac status
minix-admin security audit events 10
```

## Usage

```text
minix-admin <command> [subcommand] [args...]
minix-admin                        # Interactive TUI shell
minix-admin --tui                  # Interactive TUI shell
minix-admin --dashboard            # TUI dashboard
```

### Commands

| Command | Description |
|---------|-------------|
| `services` | Service management (list, status, start, stop, restart) |
| `system` | System monitoring (info, cpu, memory, disk, uptime) |
| `network` | Network management (interfaces, stats, route, arp) |
| `security` | Security management (mac, caps, audit) |
| `dashboard` | Real-time TUI monitoring dashboard |
| `help` | Show help message |
| `version` | Show version |

## Architecture

```text
┌──────────────────────────────────────────────────────────┐
│                     minix-admin                           │
│                                                           │
│  CLI/TUI Parser → Dispatcher → Module Handlers           │
│                                                           │
│  Services  System  Network  Security  Dashboard           │
│     │        │        │         │          │              │
│     ├─ RS IPC ├─ procfs├─ ioctl  ├─ macd IPC├─ ALL       │
│     │         ├─ sysctl├─ sysctl ├─ auditd  │              │
│     │         │        │         ├─ capctl  │              │
└──────────────────────────────────────────────────────────┘
```

## Development

```bash
# Build
cargo build -p minix-admin

# Run tests
cargo test -p minix-admin

# Check with clippy
cargo clippy -p minix-admin

# Build for release
cargo build -p minix-admin --release
```

### Adding a new command

1. Create a new module in `src/` (e.g., `src/foo.rs`)
2. Implement public functions returning `Result<(), String>`
3. Add `mod foo;` in `main.rs`
4. Add handler in the dispatch chain
5. Add tab-completion entries in `shell.rs`

## License

GergiOS — see the LICENSE file in the project root.

## See Also

- `minix-admin(8)` — full man page
- `minix-term(3)` — terminal library used by the TUI
