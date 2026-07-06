/// Memory-safe .modinfo section parser for Linux kernel modules.
///
/// The .modinfo section contains key=value pairs as NUL-terminated
/// strings in a flat array.  Each entry has the format "key=value\0".
///
/// Known keys:
///   - name:      Module name (e.g. "e1000e")
///   - license:   License string (e.g. "GPL", "Dual BSD/GPL")
///   - depends:   Comma-separated dependency list
///   - vermagic:  Kernel version magic string
///   - alias:     Device alias (e.g. "pci:v00008086d0000150C")
///   - parm:      Parameter description (e.g. "debug:int")
///   - description: Human-readable description

use crate::error::ElfError;

/// Maximum number of modinfo entries we track.
pub const MAX_MODINFO_ENTRIES: usize = 32;

/// A single parsed .modinfo key=value entry.
#[derive(Clone, Debug)]
pub struct ModinfoEntry {
    pub key: String,
    pub value: String,
}

/// Parsed .modinfo section.
#[derive(Clone, Debug)]
pub struct ModInfo {
    pub entries: Vec<ModinfoEntry>,
}

impl ModInfo {
    /// Create an empty ModInfo.
    pub fn new() -> Self {
        ModInfo { entries: Vec::with_capacity(MAX_MODINFO_ENTRIES) }
    }

    /// Get a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.iter()
            .find(|e| e.key == key)
            .map(|e| e.value.as_str())
    }

    /// Check if a module's license is GPL-compatible.
    pub fn is_gpl_compatible(&self) -> bool {
        match self.get("license") {
            Some(lic) => {
                lic == "GPL"
                    || lic == "GPL v2"
                    || lic == "GPL v3"
                    || lic == "GPL and additional rights"
                    || lic.starts_with("Dual")
                    || lic.contains("GPL")
            }
            None => false,
        }
    }
}

/// Parse .modinfo section data.
///
/// The data is an array of NUL-terminated "key=value" strings.
/// This is the Linux kernel's modinfo format (NOT the MODINFO_HDR_SZ
/// format used by some internal kernel representations — that format
/// has uint32_t name_len + uint32_t value_len prefixes).
///
/// Actually, Linux .ko files use the NUL-separated format:
///   "key=value\0key2=value2\0"
pub fn parse_modinfo(data: &[u8]) -> Result<ModInfo, ElfError> {
    let mut mi = ModInfo::new();
    let mut pos = 0;

    while pos < data.len() {
        // Find the NUL terminator
        let end = data[pos..].iter().position(|&b| b == 0)
            .map(|n| pos + n)
            .unwrap_or(data.len());

        if end <= pos {
            // Empty entry or end of data
            pos = end + 1;
            continue;
        }

        let entry = &data[pos..end];
        if let Some(eq_pos) = entry.iter().position(|&b| b == b'=') {
            let key = String::from_utf8_lossy(&entry[..eq_pos]).to_string();
            let value = String::from_utf8_lossy(&entry[eq_pos + 1..]).to_string();

            if mi.entries.len() < MAX_MODINFO_ENTRIES {
                mi.entries.push(ModinfoEntry { key, value });
            }
        }

        pos = end + 1; // skip NUL
    }

    Ok(mi)
}

/// Parse only the module name from .modinfo (fast path for depmod).
pub fn parse_modinfo_name(data: &[u8]) -> Option<String> {
    let mut pos = 0;
    while pos < data.len() {
        let end = data[pos..].iter().position(|&b| b == 0)
            .map(|n| pos + n)
            .unwrap_or(data.len());
        if end <= pos {
            pos = end + 1;
            continue;
        }
        let entry = &data[pos..end];
        if let Some(eq_pos) = entry.iter().position(|&b| b == b'=') {
            if entry[..eq_pos] == *b"name" {
                return Some(String::from_utf8_lossy(&entry[eq_pos + 1..]).to_string());
            }
        }
        pos = end + 1;
    }
    None
}

/// Parse the dependency list from the "depends" modinfo key.
pub fn parse_depends(dep_str: &str) -> Vec<String> {
    dep_str.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Parse the alias list — extract PCI aliases.
pub fn parse_aliases(modinfo: &ModInfo) -> Vec<String> {
    let mut aliases = Vec::new();
    if let Some(val) = modinfo.get("alias") {
        // "alias" can appear multiple times — we only get the first
        // with the current simple parser. For multiple aliases, the
        // key is repeated in the raw modinfo data.
        if val.starts_with("pci:") {
            aliases.push(val.to_string());
        }
    }
    aliases
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modinfo_basic() {
        let data = b"name=e1000e\0license=GPL\0vermagic=6.1.0\0";
        let mi = parse_modinfo(data).expect("should parse");
        assert_eq!(mi.get("name"), Some("e1000e"));
        assert_eq!(mi.get("license"), Some("GPL"));
        assert_eq!(mi.get("vermagic"), Some("6.1.0"));
        assert!(mi.is_gpl_compatible());
    }

    #[test]
    fn test_modinfo_non_gpl() {
        let data = b"name=closed_mod\0license=Proprietary\0";
        let mi = parse_modinfo(data).expect("should parse");
        assert!(!mi.is_gpl_compatible());
    }

    #[test]
    fn test_modinfo_depends() {
        let data = b"name=ata_piix\0depends=libata,scsi_mod\0";
        let mi = parse_modinfo(data).expect("should parse");
        let deps = parse_depends(mi.get("depends").unwrap_or(""));
        assert_eq!(deps, vec!["libata", "scsi_mod"]);
    }

    #[test]
    fn test_parse_name_fast() {
        let data = b"vermagic=6.1.0\0name=test_mod\0license=GPL\0";
        assert_eq!(parse_modinfo_name(data), Some("test_mod".to_string()));
    }
}
