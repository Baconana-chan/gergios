//! # Network Manager
//!
//! Network interface management, statistics, routing table, and ARP cache.
//!
//! ## Architecture
//!
//! On MINIX, network information is retrieved via:
//! - `getifaddrs()` — list interfaces with addresses (BSD standard)
//! - `ioctl(s, SIOCGIFFLAGS, &ifr)` — interface flags (up/down/running)
//! - `ioctl(s, SIOCGIFDATA, &ifdr)` — interface statistics (packets, bytes, errors)
//! - `ioctl(s, SIOCGIFCONF, &ifc)` — interface configuration list
//! - `ioctl(s, SIOCGIFMTU, &ifr)` — interface MTU
//! - `/proc/net/route` — routing table (or sysctl)
//! - `/proc/net/arp` — ARP cache
//! - `sysctl net.inet.*` — network stack statistics

use std::fmt;

// ============================================================================
// Data types
// ============================================================================

/// Interface flags (matching BSD `IFF_*`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InterfaceFlag {
    Up = 0x1,
    Broadcast = 0x2,
    Debug = 0x4,
    Loopback = 0x8,
    PointToPoint = 0x10,
    Running = 0x40,
    NoArp = 0x80,
    Promisc = 0x100,
    AllMulti = 0x200,
    OActive = 0x400,
    Simplex = 0x800,
    Link0 = 0x1000,
    Link1 = 0x2000,
    Link2 = 0x4000,
    MultiBroad = 0x8000,
}

impl InterfaceFlag {
    /// Human-readable short name.
    fn short_name(&self) -> &'static str {
        match self {
            Self::Up => "UP",
            Self::Broadcast => "BROADCAST",
            Self::Debug => "DEBUG",
            Self::Loopback => "LOOPBACK",
            Self::PointToPoint => "POINTOPOINT",
            Self::Running => "RUNNING",
            Self::NoArp => "NOARP",
            Self::Promisc => "PROMISC",
            Self::AllMulti => "ALLMULTI",
            Self::OActive => "OACTIVE",
            Self::Simplex => "SIMPLEX",
            Self::Link0 => "LINK0",
            Self::Link1 => "LINK1",
            Self::Link2 => "LINK2",
            Self::MultiBroad => "MULTIBROAD",
        }
    }
}

/// Interface type / hardware type.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InterfaceType {
    Ethernet,
    Loopback,
    Wifi,
    Bridge,
    Tap,
    Vlan,
    Ppp,
    Other,
}

impl fmt::Display for InterfaceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ethernet => write!(f, "eth"),
            Self::Loopback => write!(f, "loop"),
            Self::Wifi => write!(f, "wlan"),
            Self::Bridge => write!(f, "bridge"),
            Self::Tap => write!(f, "tap"),
            Self::Vlan => write!(f, "vlan"),
            Self::Ppp => write!(f, "ppp"),
            Self::Other => write!(f, "other"),
        }
    }
}

/// A network interface entry.
#[derive(Clone, Debug)]
pub struct InterfaceInfo {
    /// Interface name (e.g., "eth0", "lo0", "wlan0").
    pub name: String,
    /// Interface type.
    pub if_type: InterfaceType,
    /// MAC address as colon-separated hex (e.g., "00:11:22:33:44:55").
    pub mac: String,
    /// IPv4 address.
    pub ipv4: String,
    /// IPv6 address.
    pub ipv6: String,
    /// Network mask (e.g., "255.255.255.0").
    pub netmask: String,
    /// Broadcast address.
    pub broadcast: String,
    /// MTU (bytes).
    pub mtu: u32,
    /// Speed in Mbps (0 = unknown).
    pub speed_mbps: u32,
    /// Flags bitmask.
    pub flags: u32,
    /// Whether the interface is administratively up.
    pub is_up: bool,
    /// Whether the interface has carrier / running.
    pub is_running: bool,
}

impl InterfaceInfo {
    /// Format interface flags as a space-separated string.
    pub fn flags_string(&self) -> String {
        let all_flags: [(InterfaceFlag, bool); 9] = [
            (InterfaceFlag::Up, self.is_up),
            (InterfaceFlag::Running, self.is_running),
            (InterfaceFlag::Broadcast, (self.flags & InterfaceFlag::Broadcast as u32) != 0),
            (InterfaceFlag::Loopback, self.if_type == InterfaceType::Loopback),
            (InterfaceFlag::PointToPoint, (self.flags & InterfaceFlag::PointToPoint as u32) != 0),
            (InterfaceFlag::Promisc, (self.flags & InterfaceFlag::Promisc as u32) != 0),
            (InterfaceFlag::NoArp, (self.flags & InterfaceFlag::NoArp as u32) != 0),
            (InterfaceFlag::AllMulti, (self.flags & InterfaceFlag::AllMulti as u32) != 0),
            (InterfaceFlag::Simplex, (self.flags & InterfaceFlag::Simplex as u32) != 0),
        ];

        let mut parts: Vec<String> = Vec::new();
        for (flag, enabled) in &all_flags {
            if *enabled {
                parts.push(flag.short_name().to_string());
            }
        }

        if self.speed_mbps > 0 {
            parts.push(format!("{}Mb", self.speed_mbps));
        }

        parts.join(" ")
    }
}

/// Interface statistics counters.
#[derive(Clone, Debug)]
pub struct InterfaceStats {
    /// Interface name.
    pub if_name: String,
    /// Received packets.
    pub rx_packets: u64,
    /// Received bytes.
    pub rx_bytes: u64,
    /// Receive errors.
    pub rx_errors: u64,
    /// Received packets dropped.
    pub rx_dropped: u64,
    /// Transmitted packets.
    pub tx_packets: u64,
    /// Transmitted bytes.
    pub tx_bytes: u64,
    /// Transmit errors.
    pub tx_errors: u64,
    /// Transmitted packets dropped.
    pub tx_dropped: u64,
    /// Collisions detected.
    pub collisions: u64,
}

/// A routing table entry.
#[derive(Clone, Debug)]
pub struct RouteEntry {
    /// Destination network/address.
    pub destination: String,
    /// Gateway address.
    pub gateway: String,
    /// Network mask.
    pub netmask: String,
    /// Interface for this route.
    pub if_name: String,
    /// Route flags (RTF_* style).
    pub flags: u32,
    /// Metric / priority.
    pub metric: u32,
    /// Reference count.
    pub ref_count: u32,
    /// Use count.
    pub use_count: u64,
}

impl RouteEntry {
    /// Short flag description.
    pub fn flags_string(&self) -> String {
        let mut parts = Vec::new();
        if (self.flags & 0x01) != 0 { parts.push("U"); }    // RTF_UP
        if (self.flags & 0x02) != 0 { parts.push("G"); }    // RTF_GATEWAY
        if (self.flags & 0x08) != 0 { parts.push("H"); }    // RTF_HOST
        if (self.flags & 0x40) != 0 { parts.push("D"); }   // RTF_DYNAMIC
        if (self.flags & 0x80) != 0 { parts.push("M"); }   // RTF_MODIFIED
        parts.join("")
    }
}

/// An ARP cache entry.
#[derive(Clone, Debug)]
pub struct ArpEntry {
    /// IP address.
    pub ip: String,
    /// MAC address.
    pub mac: String,
    /// Interface name.
    pub if_name: String,
    /// Entry type (static, dynamic, incomplete).
    pub entry_type: ArpEntryType,
    /// Expiry / remaining lifetime (seconds).
    pub expire_secs: u32,
}

/// ARP entry type.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArpEntryType {
    Static,
    Dynamic,
    Incomplete,
    Unknown,
}

impl fmt::Display for ArpEntryType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Static => write!(f, "static"),
            Self::Dynamic => write!(f, "dynamic"),
            Self::Incomplete => write!(f, "incomplete"),
            Self::Unknown => write!(f, "?"),
        }
    }
}

/// Known network interfaces in GergiOS.
pub(crate) const KNOWN_INTERFACES: &[(&str, InterfaceType)] = &[
    ("lo0", InterfaceType::Loopback),
    ("eth0", InterfaceType::Ethernet),
    ("eth1", InterfaceType::Ethernet),
    ("wlan0", InterfaceType::Wifi),
    ("bridge0", InterfaceType::Bridge),
];

// ============================================================================
// Public API
// ============================================================================

/// List all network interfaces.
pub fn show_interfaces() -> Result<(), String> {
    println!("{:<8} {:<6} {:<18} {:<16} {:<6} {:<5} {}",
        "IFACE", "TYPE", "MAC", "IP", "MTU", "SPEED", "FLAGS");
    println!("{}", "-".repeat(90));

    for &(name, _if_type) in KNOWN_INTERFACES {
        let _ = _if_type;
        let info = query_interface(name);
        let status_icon = if info.is_up {
            if info.is_running { "▲" } else { "△" }
        } else {
            "▼"
        };

        let speed_str = if info.speed_mbps > 0 {
            format!("{}M", info.speed_mbps)
        } else {
            "-".to_string()
        };

        println!("{:<2} {:<6} {:<6} {:<18} {:<16} {:>6} {:>5} {}",
            status_icon,
            info.name,
            format!("{}", info.if_type),
            info.mac,
            info.ipv4,
            info.mtu,
            speed_str,
            info.flags_string(),
        );
    }

    Ok(())
}

/// Show detailed information for a single interface.
pub fn show_iface(name: &str) -> Result<(), String> {
    let info = query_interface(name);

    println!("Interface:  {}", info.name);
    println!("Type:       {} ({})", info.if_type, match info.if_type {
        InterfaceType::Ethernet => "Ethernet",
        InterfaceType::Loopback => "Loopback",
        InterfaceType::Wifi => "Wireless",
        InterfaceType::Bridge => "Bridge",
        InterfaceType::Tap => "TAP",
        InterfaceType::Vlan => "VLAN",
        InterfaceType::Ppp => "PPP",
        InterfaceType::Other => "Other",
    });
    println!("MAC:        {}", info.mac);
    println!("IPv4:       {}", info.ipv4);
    println!("IPv6:       {}", if info.ipv6.is_empty() { "-" } else { &info.ipv6 });
    println!("Netmask:    {}", info.netmask);
    println!("Broadcast:  {}", info.broadcast);
    println!("MTU:        {}", info.mtu);
    println!("Speed:      {} Mbps", if info.speed_mbps > 0 {
        format!("{}", info.speed_mbps)
    } else {
        "unknown".to_string()
    });
    println!("Status:     {} / {}",
        if info.is_up { "UP" } else { "DOWN" },
        if info.is_running { "RUNNING" } else { "NOT RUNNING" },
    );
    println!("Flags:      {}", info.flags_string());

    // Show statistics
    let stats = query_stats(name);
    println!();
    println!("Statistics:");
    println!("  RX: {} packets, {} bytes, {} errors, {} dropped",
        stats.rx_packets, format_bytes(stats.rx_bytes),
        stats.rx_errors, stats.rx_dropped);
    println!("  TX: {} packets, {} bytes, {} errors, {} dropped",
        stats.tx_packets, format_bytes(stats.tx_bytes),
        stats.tx_errors, stats.tx_dropped);
    if stats.collisions > 0 {
        println!("  Collisions: {}", stats.collisions);
    }

    Ok(())
}

/// Show interface statistics for all interfaces.
pub fn show_stats() -> Result<(), String> {
    println!("{:<8} {:>10} {:>12} {:>8} {:>8} {:>10} {:>12} {:>8} {:>8}",
        "IFACE", "RX-PKTS", "RX-BYTES", "RX-ERR", "RX-DROP",
        "TX-PKTS", "TX-BYTES", "TX-ERR", "TX-DROP");
    println!("{}", "-".repeat(95));

    for &(name, _) in KNOWN_INTERFACES {
        let s = query_stats(name);

        println!("{:<8} {:>10} {:>12} {:>8} {:>8} {:>10} {:>12} {:>8} {:>8}",
            s.if_name,
            s.rx_packets,
            format_bytes(s.rx_bytes),
            s.rx_errors,
            s.rx_dropped,
            s.tx_packets,
            format_bytes(s.tx_bytes),
            s.tx_errors,
            s.tx_dropped,
        );
    }

    Ok(())
}

/// Show the routing table.
pub fn show_route() -> Result<(), String> {
    println!("{:<18} {:<18} {:<18} {:<8} {:<6} {}",
        "DESTINATION", "GATEWAY", "NETMASK", "IFACE", "FLAGS", "METRIC");
    println!("{}", "-".repeat(85));

    let routes = get_routing_table();
    for r in &routes {
        println!("{:<18} {:<18} {:<18} {:<8} {:<6} {}",
            r.destination,
            r.gateway,
            r.netmask,
            r.if_name,
            r.flags_string(),
            r.metric,
        );
    }

    println!();
    println!("Flags: U=UP G=GATEWAY H=HOST D=DYNAMIC M=MODIFIED");

    Ok(())
}

/// Show the ARP cache.
pub fn show_arp() -> Result<(), String> {
    println!("{:<18} {:<18} {:<8} {:<8} {}",
        "IP", "MAC", "IFACE", "TYPE", "EXPIRE");
    println!("{}", "-".repeat(70));

    let arp_entries = get_arp_cache();
    for e in &arp_entries {
        let expire_str = if e.expire_secs > 0 {
            format!("{}s", e.expire_secs)
        } else {
            "perm".to_string()
        };

        println!("{:<18} {:<18} {:<8} {:<8} {}",
            e.ip,
            e.mac,
            e.if_name,
            format!("{}", e.entry_type),
            expire_str,
        );
    }

    Ok(())
}

// ============================================================================
// Data collectors (stubs — will use real SIOCGIF* ioctl on MINIX)
// ============================================================================

/// Query interface information.
pub(crate) fn query_interface(name: &str) -> InterfaceInfo {
    // Stub implementation — on MINIX this would:
    // 1. socket(AF_INET, SOCK_DGRAM, 0) for ioctl
    // 2. memset + strcpy(ifr.ifr_name, name)
    // 3. ioctl(s, SIOCGIFFLAGS, &ifr) — get flags
    // 4. ioctl(s, SIOCGIFADDR, &ifr) — get IPv4
    // 5. ioctl(s, SIOCGIFNETMASK, &ifr) — get netmask
    // 6. ioctl(s, SIOCGIFBRDADDR, &ifr) — get broadcast
    // 7. ioctl(s, SIOCGIFMTU, &ifr) — get MTU
    // 8. getifaddrs(&ifap) — get all addrs (including IPv6, MAC)
    // 9. freeifaddrs(ifap)

    match name {
        "lo0" => InterfaceInfo {
            name: name.to_string(),
            if_type: InterfaceType::Loopback,
            mac: "00:00:00:00:00:00".to_string(),
            ipv4: "127.0.0.1".to_string(),
            ipv6: "::1".to_string(),
            netmask: "255.0.0.0".to_string(),
            broadcast: "127.255.255.255".to_string(),
            mtu: 16384,
            speed_mbps: 0,
            flags: 0,
            is_up: true,
            is_running: true,
        },
        "eth0" => InterfaceInfo {
            name: name.to_string(),
            if_type: InterfaceType::Ethernet,
            mac: "00:1A:2B:3C:4D:5E".to_string(),
            ipv4: "10.0.2.15".to_string(),
            ipv6: "fe80::21a:2bff:fe3c:4d5e".to_string(),
            netmask: "255.255.255.0".to_string(),
            broadcast: "10.0.2.255".to_string(),
            mtu: 1500,
            speed_mbps: 1000,
            flags: 0,
            is_up: true,
            is_running: true,
        },
        "eth1" => InterfaceInfo {
            name: name.to_string(),
            if_type: InterfaceType::Ethernet,
            mac: "00:DE:AD:BE:EF:01".to_string(),
            ipv4: "192.168.1.100".to_string(),
            ipv6: "fe80::2de:adff:febe:ef01".to_string(),
            netmask: "255.255.255.0".to_string(),
            broadcast: "192.168.1.255".to_string(),
            mtu: 1500,
            speed_mbps: 100,
            flags: 0,
            is_up: true,
            is_running: true,
        },
        "wlan0" => InterfaceInfo {
            name: name.to_string(),
            if_type: InterfaceType::Wifi,
            mac: "AA:BB:CC:DD:EE:FF".to_string(),
            ipv4: "192.168.1.42".to_string(),
            ipv6: String::new(),
            netmask: "255.255.255.0".to_string(),
            broadcast: "192.168.1.255".to_string(),
            mtu: 1500,
            speed_mbps: 300,
            flags: 0,
            is_up: true,
            is_running: true,
        },
        "bridge0" => InterfaceInfo {
            name: name.to_string(),
            if_type: InterfaceType::Bridge,
            mac: "02:00:00:00:00:01".to_string(),
            ipv4: "10.0.3.1".to_string(),
            ipv6: String::new(),
            netmask: "255.255.255.0".to_string(),
            broadcast: "10.0.3.255".to_string(),
            mtu: 1500,
            speed_mbps: 0,
            flags: 0,
            is_up: true,
            is_running: true,
        },
        _ => InterfaceInfo {
            name: name.to_string(),
            if_type: InterfaceType::Other,
            mac: "??:??:??:??:??:??".to_string(),
            ipv4: String::new(),
            ipv6: String::new(),
            netmask: String::new(),
            broadcast: String::new(),
            mtu: 1500,
            speed_mbps: 0,
            flags: 0,
            is_up: false,
            is_running: false,
        },
    }
}

/// Query interface statistics.
pub(crate) fn query_stats(name: &str) -> InterfaceStats {
    // Stub — on MINIX: ioctl(s, SIOCGIFDATA, &ifdr)
    // or read /sys/class/net/<name>/statistics/

    match name {
        "lo0" => InterfaceStats {
            if_name: name.to_string(),
            rx_packets: 42_513,
            rx_bytes: 3_217_890,
            rx_errors: 0,
            rx_dropped: 0,
            tx_packets: 42_513,
            tx_bytes: 3_217_890,
            tx_errors: 0,
            tx_dropped: 0,
            collisions: 0,
        },
        "eth0" => InterfaceStats {
            if_name: name.to_string(),
            rx_packets: 1_234_567,
            rx_bytes: 890_123_456,
            rx_errors: 2,
            rx_dropped: 5,
            tx_packets: 987_654,
            tx_bytes: 456_789_012,
            tx_errors: 1,
            tx_dropped: 3,
            collisions: 0,
        },
        "eth1" => InterfaceStats {
            if_name: name.to_string(),
            rx_packets: 456_789,
            rx_bytes: 321_098_765,
            rx_errors: 0,
            rx_dropped: 1,
            tx_packets: 345_678,
            tx_bytes: 234_567_890,
            tx_errors: 0,
            tx_dropped: 0,
            collisions: 2,
        },
        "wlan0" => InterfaceStats {
            if_name: name.to_string(),
            rx_packets: 1_024,
            rx_bytes: 524_288,
            rx_errors: 12,
            rx_dropped: 3,
            tx_packets: 512,
            tx_bytes: 131_072,
            tx_errors: 8,
            tx_dropped: 0,
            collisions: 0,
        },
        "bridge0" => InterfaceStats {
            if_name: name.to_string(),
            rx_packets: 10_000,
            rx_bytes: 5_000_000,
            rx_errors: 0,
            rx_dropped: 0,
            tx_packets: 10_000,
            tx_bytes: 5_000_000,
            tx_errors: 0,
            tx_dropped: 0,
            collisions: 0,
        },
        _ => InterfaceStats {
            if_name: name.to_string(),
            rx_packets: 0,
            rx_bytes: 0,
            rx_errors: 0,
            rx_dropped: 0,
            tx_packets: 0,
            tx_bytes: 0,
            tx_errors: 0,
            tx_dropped: 0,
            collisions: 0,
        },
    }
}

/// Get the routing table.
fn get_routing_table() -> Vec<RouteEntry> {
    // Stub — on MINIX: read /proc/net/route or sysctl net.inet.ip.routingtable
    vec![
        RouteEntry {
            destination: "default".to_string(),
            gateway: "10.0.2.2".to_string(),
            netmask: "0.0.0.0".to_string(),
            if_name: "eth0".to_string(),
            flags: 0x3, // UP + GATEWAY
            metric: 100,
            ref_count: 0,
            use_count: 42_000,
        },
        RouteEntry {
            destination: "10.0.2.0".to_string(),
            gateway: "0.0.0.0".to_string(),
            netmask: "255.255.255.0".to_string(),
            if_name: "eth0".to_string(),
            flags: 0x1, // UP
            metric: 0,
            ref_count: 0,
            use_count: 12_000,
        },
        RouteEntry {
            destination: "192.168.1.0".to_string(),
            gateway: "0.0.0.0".to_string(),
            netmask: "255.255.255.0".to_string(),
            if_name: "eth1".to_string(),
            flags: 0x1, // UP
            metric: 0,
            ref_count: 0,
            use_count: 8_500,
        },
        RouteEntry {
            destination: "192.168.1.0".to_string(),
            gateway: "0.0.0.0".to_string(),
            netmask: "255.255.255.0".to_string(),
            if_name: "wlan0".to_string(),
            flags: 0x1, // UP
            metric: 200,
            ref_count: 0,
            use_count: 120,
        },
        RouteEntry {
            destination: "127.0.0.1".to_string(),
            gateway: "127.0.0.1".to_string(),
            netmask: "255.255.255.255".to_string(),
            if_name: "lo0".to_string(),
            flags: 0x9, // UP + HOST
            metric: 0,
            ref_count: 1,
            use_count: 42_513,
        },
    ]
}

/// Get the ARP cache.
fn get_arp_cache() -> Vec<ArpEntry> {
    // Stub — on MINIX: read /proc/net/arp or sysctl net.inet.arp.cache
    vec![
        ArpEntry {
            ip: "10.0.2.2".to_string(),
            mac: "52:54:00:12:34:56".to_string(),
            if_name: "eth0".to_string(),
            entry_type: ArpEntryType::Dynamic,
            expire_secs: 843,
        },
        ArpEntry {
            ip: "10.0.2.3".to_string(),
            mac: "08:00:27:AB:CD:EF".to_string(),
            if_name: "eth0".to_string(),
            entry_type: ArpEntryType::Dynamic,
            expire_secs: 234,
        },
        ArpEntry {
            ip: "192.168.1.1".to_string(),
            mac: "00:11:22:33:44:55".to_string(),
            if_name: "eth1".to_string(),
            entry_type: ArpEntryType::Dynamic,
            expire_secs: 621,
        },
        ArpEntry {
            ip: "192.168.1.10".to_string(),
            mac: "AA:BB:CC:DD:EE:11".to_string(),
            if_name: "eth1".to_string(),
            entry_type: ArpEntryType::Static,
            expire_secs: 0,
        },
        ArpEntry {
            ip: "10.0.2.5".to_string(),
            mac: "(incomplete)".to_string(),
            if_name: "eth0".to_string(),
            entry_type: ArpEntryType::Incomplete,
            expire_secs: 45,
        },
    ]
}

// ============================================================================
// Display helpers
// ============================================================================

/// Format bytes as a human-readable string (KB, MB, GB).
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        if bytes % GB == 0 {
            format!("{}GB", bytes / GB)
        } else {
            format!("{:.1}GB", bytes as f64 / GB as f64)
        }
    } else if bytes >= MB {
        if bytes % MB == 0 {
            format!("{}MB", bytes / MB)
        } else {
            format!("{:.1}MB", bytes as f64 / MB as f64)
        }
    } else if bytes >= KB {
        format!("{}KB", bytes / KB)
    } else {
        format!("{}B", bytes)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interface_type_display() {
        assert_eq!(format!("{}", InterfaceType::Ethernet), "eth");
        assert_eq!(format!("{}", InterfaceType::Loopback), "loop");
        assert_eq!(format!("{}", InterfaceType::Wifi), "wlan");
    }

    #[test]
    fn test_interface_flag_short_names() {
        assert_eq!(InterfaceFlag::Up.short_name(), "UP");
        assert_eq!(InterfaceFlag::Running.short_name(), "RUNNING");
        assert_eq!(InterfaceFlag::Broadcast.short_name(), "BROADCAST");
    }

    #[test]
    fn test_known_interfaces_lo0() {
        let info = query_interface("lo0");
        assert_eq!(info.name, "lo0");
        assert_eq!(info.if_type, InterfaceType::Loopback);
        assert!(info.is_up);
        assert!(info.is_running);
        assert_eq!(info.ipv4, "127.0.0.1");
    }

    #[test]
    fn test_known_interfaces_eth0() {
        let info = query_interface("eth0");
        assert_eq!(info.name, "eth0");
        assert_eq!(info.if_type, InterfaceType::Ethernet);
        assert_eq!(info.mac, "00:1A:2B:3C:4D:5E");
        assert_eq!(info.mtu, 1500);
        assert_eq!(info.speed_mbps, 1000);
    }

    #[test]
    fn test_unknown_interface() {
        let info = query_interface("nonexistent99");
        assert_eq!(info.if_type, InterfaceType::Other);
        assert!(!info.is_up);
    }

    #[test]
    fn test_interface_flags_string() {
        let info = query_interface("lo0");
        let flags = info.flags_string();
        assert!(flags.contains("UP"));
        assert!(flags.contains("LOOPBACK"));
    }

    #[test]
    fn test_eth0_flags() {
        let info = query_interface("eth0");
        let flags = info.flags_string();
        assert!(flags.contains("UP"));
        assert!(flags.contains("1000Mb"));
    }

    #[test]
    fn test_route_entry_flags() {
        let routes = get_routing_table();
        assert!(!routes.is_empty());

        let default_route = &routes[0];
        assert_eq!(default_route.destination, "default");
        assert_eq!(default_route.flags_string(), "UG");
    }

    #[test]
    fn test_routing_table_has_loopback() {
        let routes = get_routing_table();
        let lo = routes.iter().find(|r| r.if_name == "lo0");
        assert!(lo.is_some());
        assert_eq!(lo.unwrap().flags_string(), "UH");
    }

    #[test]
    fn test_arp_cache() {
        let arp = get_arp_cache();
        assert!(!arp.is_empty());
        assert!(arp.iter().any(|e| e.ip == "10.0.2.2"));
    }

    #[test]
    fn test_arp_entry_types() {
        let arp = get_arp_cache();
        let static_entry = arp.iter().find(|e| e.entry_type == ArpEntryType::Static);
        assert!(static_entry.is_some());
        assert_eq!(static_entry.unwrap().expire_secs, 0);

        let incomplete = arp.iter().find(|e| e.entry_type == ArpEntryType::Incomplete);
        assert!(incomplete.is_some());
        assert!(incomplete.unwrap().mac.contains("incomplete"));
    }

    #[test]
    fn test_arp_entry_type_display() {
        assert_eq!(format!("{}", ArpEntryType::Static), "static");
        assert_eq!(format!("{}", ArpEntryType::Dynamic), "dynamic");
    }

    #[test]
    fn test_stats_lo0() {
        let stats = query_stats("lo0");
        assert_eq!(stats.if_name, "lo0");
        assert_eq!(stats.rx_packets, stats.tx_packets);
        assert_eq!(stats.rx_errors, 0);
    }

    #[test]
    fn test_stats_eth0_has_traffic() {
        let stats = query_stats("eth0");
        assert!(stats.rx_packets > 1_000_000);
        assert!(stats.tx_bytes > 100_000_000);
    }

    #[test]
    fn test_unknown_stats_zero() {
        let stats = query_stats("nonexistent");
        assert_eq!(stats.rx_packets, 0);
        assert_eq!(stats.tx_bytes, 0);
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0B");
        assert_eq!(format_bytes(512), "512B");
        assert_eq!(format_bytes(1024), "1KB");
        assert_eq!(format_bytes(2048), "2KB");
        assert_eq!(format_bytes(1_048_576), "1MB");
        assert_eq!(format_bytes(1_073_741_824), "1GB");
        assert_eq!(format_bytes(1_500_000), "1.4MB");
    }

    #[test]
    fn test_show_interfaces_dont_panic() {
        assert!(show_interfaces().is_ok());
    }

    #[test]
    fn test_show_iface_known() {
        assert!(show_iface("eth0").is_ok());
        assert!(show_iface("lo0").is_ok());
    }

    #[test]
    fn test_show_iface_unknown() {
        assert!(show_iface("nonexistent").is_ok());
    }

    #[test]
    fn test_show_stats_dont_panic() {
        assert!(show_stats().is_ok());
    }

    #[test]
    fn test_show_route_dont_panic() {
        assert!(show_route().is_ok());
    }

    #[test]
    fn test_show_arp_dont_panic() {
        assert!(show_arp().is_ok());
    }

    #[test]
    fn test_route_flags_parsing() {
        let mut r = RouteEntry {
            destination: "test".to_string(), gateway: "0.0.0.0".to_string(),
            netmask: "255.255.255.0".to_string(), if_name: "eth0".to_string(),
            flags: 0, metric: 0, ref_count: 0, use_count: 0,
        };
        assert_eq!(r.flags_string(), "");

        r.flags = 0x01;
        assert_eq!(r.flags_string(), "U");

        r.flags = 0x03;
        assert_eq!(r.flags_string(), "UG");

        r.flags = 0xCB; // all flags: U + G + H + D + M
        assert_eq!(r.flags_string(), "UGHDM");
    }

    #[test]
    fn test_format_bytes_zero() {
        assert_eq!(format_bytes(0), "0B");
    }

    #[test]
    fn test_format_bytes_max() {
        // Large values shouldn't panic
        let result = format_bytes(u64::MAX);
        assert!(result.contains("GB") || result.contains("TB"));
    }

    #[test]
    fn test_format_bytes_exact() {
        assert_eq!(format_bytes(1), "1B");
        assert_eq!(format_bytes(1023), "1023B");
        assert_eq!(format_bytes(1024), "1KB");
        assert_eq!(format_bytes(1024 * 1024), "1MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1GB");
    }

    #[test]
    fn test_arp_display_all_types() {
        let arp = get_arp_cache();
        assert!(arp.iter().any(|e| e.entry_type == ArpEntryType::Static));
        assert!(arp.iter().any(|e| e.entry_type == ArpEntryType::Dynamic));
        assert!(arp.iter().any(|e| e.entry_type == ArpEntryType::Incomplete));
    }

    #[test]
    fn test_interface_type_display_all() {
        assert_eq!(format!("{}", InterfaceType::Tap), "tap");
        assert_eq!(format!("{}", InterfaceType::Vlan), "vlan");
        assert_eq!(format!("{}", InterfaceType::Ppp), "ppp");
        assert_eq!(format!("{}", InterfaceType::Other), "other");
    }

    #[test]
    fn test_known_interfaces_list() {
        assert!(KNOWN_INTERFACES.iter().any(|(n, _)| *n == "lo0"));
        assert!(KNOWN_INTERFACES.iter().any(|(n, _)| *n == "eth0"));
        assert!(KNOWN_INTERFACES.iter().any(|(n, _)| *n == "wlan0"));
        assert_eq!(KNOWN_INTERFACES.len(), 5);
    }
}
