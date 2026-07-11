//! # Command History
//!
//! In-memory command history with:
//! - Add commands (dedup adjacent)
//! - Navigate up/down
//! - Search (prefix match)
//! - Capacity limit

/// In-memory command history.
///
/// Newest entries are at the end of the vector.
pub struct History {
    /// Stored command lines (oldest first).
    entries: Vec<String>,
    /// Maximum number of entries.
    capacity: usize,
    /// Current navigation position (None = not browsing).
    pos: Option<usize>,
    /// Line saved when entering history navigation.
    saved_line: String,
}

impl History {
    /// Create a new history with the given capacity.
    pub fn new(capacity: usize) -> Self {
        History {
            entries: Vec::with_capacity(capacity.min(1)),
            capacity,
            pos: None,
            saved_line: String::new(),
        }
    }

    /// Add a command to the history.
    /// Duplicates with the previous entry are skipped.
    pub fn add(&mut self, line: &str) {
        let trimmed = line.trim().to_string();
        if trimmed.is_empty() {
            return;
        }

        // Skip if same as last entry
        if self.entries.last().map_or(false, |last| last == &trimmed) {
            return;
        }

        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push(trimmed);
        self.pos = None;
    }

    /// Get the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if history is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all history.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.pos = None;
    }

    /// Start navigating history (save current buffer).
    /// The first call to `older()` will return the most recent entry.
    pub fn start_nav(&mut self, current_line: &str) {
        if !self.entries.is_empty() && self.pos.is_none() {
            self.saved_line = current_line.to_string();
            // Don't set pos here — let older() determine the starting position
            // on first call. This avoids skipping the newest entry.
        }
    }

    /// Move to older entry. Returns the command, or None if at oldest.
    pub fn older(&mut self) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }

        match self.pos {
            None => {
                self.pos = Some(self.entries.len() - 1);
            }
            Some(p) => {
                if p > 0 {
                    self.pos = Some(p - 1);
                } else {
                    return None; // already at oldest
                }
            }
        }

        self.pos.and_then(|p| self.entries.get(p).map(|s| s.as_str()))
    }

    /// Move to newer entry. Returns the command, or None if back to saved.
    pub fn newer(&mut self) -> Option<&str> {
        match self.pos {
            None => None,
            Some(p) => {
                if p + 1 < self.entries.len() {
                    self.pos = Some(p + 1);
                    self.pos.and_then(|p| self.entries.get(p).map(|s| s.as_str()))
                } else {
                    // Back to saved line
                    self.pos = None;
                    if self.saved_line.is_empty() {
                        None
                    } else {
                        Some(self.saved_line.as_str())
                    }
                }
            }
        }
    }

    /// Stop navigating and return to normal mode.
    pub fn stop_nav(&mut self) {
        self.pos = None;
        self.saved_line.clear();
    }

    /// Search for the most recent command starting with `prefix`.
    pub fn search(&self, prefix: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|entry| entry.starts_with(prefix))
            .map(|s| s.as_str())
    }

    /// Get all entries (oldest first).
    pub fn all(&self) -> &[String] {
        &self.entries
    }

    /// Check if currently navigating history.
    pub fn is_navigating(&self) -> bool {
        self.pos.is_some()
    }

    /// Get the saved line (the line that was active when navigation started).
    pub fn saved_line(&self) -> &str {
        &self.saved_line
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_history_empty() {
        let h = History::new(100);
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn test_add_one() {
        let mut h = History::new(100);
        h.add("ls -la");
        assert_eq!(h.len(), 1);
        assert_eq!(h.entries[0], "ls -la");
    }

    #[test]
    fn test_add_multiple() {
        let mut h = History::new(100);
        h.add("ls");
        h.add("pwd");
        h.add("cd /tmp");
        assert_eq!(h.len(), 3);
    }

    #[test]
    fn test_add_duplicate_consecutive() {
        let mut h = History::new(100);
        h.add("ls");
        h.add("ls");
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn test_add_empty() {
        let mut h = History::new(100);
        h.add("");
        h.add("  ");
        assert!(h.is_empty());
    }

    #[test]
    fn test_capacity_limit() {
        let mut h = History::new(3);
        h.add("a");
        h.add("b");
        h.add("c");
        h.add("d");
        assert_eq!(h.len(), 3);
        assert_eq!(h.entries[0], "b");
    }

    #[test]
    fn test_navigation() {
        let mut h = History::new(100);
        h.add("ls");
        h.add("pwd");
        h.add("cd /tmp");

        // Navigate back (oldest first, newest at end)
        h.start_nav("current");
        assert_eq!(h.older(), Some("cd /tmp"));
        assert_eq!(h.older(), Some("pwd"));
        assert_eq!(h.older(), Some("ls"));
        assert_eq!(h.older(), None); // at oldest

        // Navigate forward
        assert_eq!(h.newer(), Some("pwd"));
        assert_eq!(h.newer(), Some("cd /tmp"));
        // Back to saved line (non-empty saved_line returns Some(saved_line))
        assert_eq!(h.newer(), Some("current"));
    }

    #[test]
    fn test_search() {
        let mut h = History::new(100);
        h.add("ls -la");
        h.add("pwd");
        h.add("ls /tmp");

        assert_eq!(h.search("ls"), Some("ls /tmp")); // most recent match
        assert_eq!(h.search("pw"), Some("pwd"));
        assert_eq!(h.search("xyz"), None);
    }

    #[test]
    fn test_clear() {
        let mut h = History::new(100);
        h.add("ls");
        h.clear();
        assert!(h.is_empty());
    }

    #[test]
    fn test_stop_nav() {
        let mut h = History::new(100);
        h.add("ls");
        h.start_nav("current");
        h.stop_nav();
        assert!(h.pos.is_none());
        assert!(h.saved_line.is_empty());
    }

    #[test]
    fn test_all_returns_entries() {
        let mut h = History::new(100);
        h.add("a");
        h.add("b");
        assert_eq!(h.all().len(), 2);
    }
}
