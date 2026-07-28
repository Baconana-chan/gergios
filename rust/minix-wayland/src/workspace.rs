//! # Workspace Manager — Virtual Desktops
//!
//! Implements Phase 4.4 of the GUI architecture: multiple workspaces
//! (virtual desktops). Each workspace has its own set of windows.
//! Only one workspace is visible at a time; switching workspaces
//! hides/show windows accordingly.
//!
//! ## Architecture
//!
//! ```text
//! WorkspaceManager
//!   ├── workspaces: Vec<WorkspaceInfo>
//!   │     └── each has: name, window_ids (xdg_toplevel_ids)
//!   └── current: usize (index of active workspace)
//! ```
//!
//! ## Integration
//!
//! - On window creation (GET_TOPLEVEL): `workspace.add_window(id)`
//! - On window destroy: `workspace.remove_window(id)`
//! - On workspace switch: `workspace.switch_to(index)` → caller updates visibility
//! - recalculate_layout: only includes windows from current workspace

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

/// Default number of workspaces.
pub const DEFAULT_WORKSPACE_COUNT: usize = 4;

/// Default workspace names.
pub const DEFAULT_WORKSPACE_NAMES: &[&str] = &["1", "2", "3", "4"];

/// Information about a single workspace.
#[derive(Clone, Debug)]
pub struct WorkspaceInfo {
    /// Display name (e.g., "1", "2", "Web", "Terminal").
    pub name: String,
    /// xdg_toplevel_ids of windows assigned to this workspace.
    pub window_ids: Vec<u32>,
}

impl WorkspaceInfo {
    fn new(name: String) -> Self {
        Self {
            name,
            window_ids: Vec::new(),
        }
    }
}

/// Result of a workspace switch operation.
///
/// Contains the window IDs that need to be hidden (from the old workspace)
/// and shown (on the new workspace), so the caller can update Shell visibility.
#[derive(Debug)]
pub struct SwitchResult {
    /// Window IDs to hide (set visible=false).
    pub to_hide: Vec<u32>,
    /// Window IDs to show (set visible=true).
    pub to_show: Vec<u32>,
    /// The new workspace index.
    pub new_index: usize,
    /// The name of the new workspace.
    pub new_name: String,
}

/// Manages multiple workspaces (virtual desktops).
///
/// Each workspace has a name and a set of window IDs. Windows are
/// assigned to a workspace on creation. Switching workspaces returns
/// which windows to hide/show.
pub struct WorkspaceManager {
    workspaces: Vec<WorkspaceInfo>,
    current: usize,
}

impl fmt::Debug for WorkspaceManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorkspaceManager")
            .field("workspace_count", &self.workspaces.len())
            .field("current", &self.current)
            .field("current_name", &self.workspaces[self.current].name)
            .field("window_counts", &self.window_counts())
            .finish()
    }
}

impl WorkspaceManager {
    /// Create a new workspace manager with the given number of workspaces.
    ///
    /// Default names are "1", "2", ..., up to 4. Workspaces beyond 4
    /// are named "N", "N+1", etc.
    pub fn new(count: usize) -> Self {
        let mut workspaces = Vec::with_capacity(count);
        for i in 0..count {
            let name = if i < DEFAULT_WORKSPACE_NAMES.len() {
                String::from(DEFAULT_WORKSPACE_NAMES[i])
            } else {
                // Format i+1 as a string (no fmt::Write in no_std)
                let n = i + 1;
                let mut s = String::new();
                let mut d = n;
                let mut digits = Vec::new();
                while d > 0 {
                    digits.push((d % 10) as u8 + b'0');
                    d /= 10;
                }
                if digits.is_empty() {
                    s.push('0');
                } else {
                    for &digit in digits.iter().rev() {
                        s.push(digit as char);
                    }
                }
                s
            };
            workspaces.push(WorkspaceInfo::new(name));
        }

        Self {
            workspaces,
            current: 0,
        }
    }

    /// Create a workspace manager with 4 default workspaces.
    pub fn default_workspaces() -> Self {
        Self::new(DEFAULT_WORKSPACE_COUNT)
    }

    // ── Workspace queries ─────────────────────────────────────────

    /// Get the current workspace index.
    pub fn current_index(&self) -> usize {
        self.current
    }

    /// Get the name of the current workspace.
    pub fn current_name(&self) -> &str {
        &self.workspaces[self.current].name
    }

    /// Get the total number of workspaces.
    pub fn workspace_count(&self) -> usize {
        self.workspaces.len()
    }

    /// Get info about a specific workspace.
    pub fn get_workspace(&self, index: usize) -> Option<&WorkspaceInfo> {
        self.workspaces.get(index)
    }

    /// Get info about all workspaces.
    pub fn all_workspaces(&self) -> &[WorkspaceInfo] {
        &self.workspaces
    }

    /// Get the window IDs on the current workspace.
    pub fn current_window_ids(&self) -> &[u32] {
        &self.workspaces[self.current].window_ids
    }

    /// Get window counts for all workspaces.
    pub fn window_counts(&self) -> Vec<usize> {
        self.workspaces.iter().map(|w| w.window_ids.len()).collect()
    }

    /// Find which workspace contains a window. Returns None if not found.
    pub fn find_window_workspace(&self, xdg_toplevel_id: u32) -> Option<usize> {
        self.workspaces.iter().position(|ws| ws.window_ids.contains(&xdg_toplevel_id))
    }

    /// Check if a window is on the current workspace.
    pub fn is_on_current(&self, xdg_toplevel_id: u32) -> bool {
        self.workspaces[self.current].window_ids.contains(&xdg_toplevel_id)
    }

    // ── Window management ─────────────────────────────────────────

    /// Add a window to the current workspace.
    ///
    /// If the window already exists in another workspace, it is moved
    /// to the current workspace. Returns true if the window was added
    /// (or moved) successfully.
    pub fn add_window(&mut self, xdg_toplevel_id: u32) -> bool {
        // Remove from any existing workspace first
        let existing = self.find_window_workspace(xdg_toplevel_id);
        if let Some(idx) = existing {
            if idx == self.current {
                return false; // already in current workspace
            }
            self.workspaces[idx].window_ids.retain(|&id| id != xdg_toplevel_id);
        }
        self.workspaces[self.current].window_ids.push(xdg_toplevel_id);
        true
    }

    /// Remove a window from all workspaces.
    ///
    /// Returns the workspace index it was removed from, or None.
    pub fn remove_window(&mut self, xdg_toplevel_id: u32) -> Option<usize> {
        for (idx, ws) in self.workspaces.iter_mut().enumerate() {
            if let Some(pos) = ws.window_ids.iter().position(|&id| id == xdg_toplevel_id) {
                ws.window_ids.remove(pos);
                return Some(idx);
            }
        }
        None
    }

    /// Move a window to a specific workspace.
    ///
    /// Returns true if the window was moved.
    pub fn move_window(&mut self, xdg_toplevel_id: u32, target_workspace: usize) -> bool {
        if target_workspace >= self.workspaces.len() {
            return false;
        }
        // Remove from current workspace
        self.remove_window(xdg_toplevel_id);
        // Add to target workspace
        self.workspaces[target_workspace].window_ids.push(xdg_toplevel_id);
        true
    }

    /// Rename a workspace.
    pub fn rename_workspace(&mut self, index: usize, name: String) -> bool {
        if let Some(ws) = self.workspaces.get_mut(index) {
            ws.name = name;
            true
        } else {
            false
        }
    }

    // ── Workspace switching ───────────────────────────────────────

    /// Switch to a different workspace.
    ///
    /// Returns a `SwitchResult` with the window IDs to hide and show.
    /// The caller is responsible for actually updating window visibility
    /// in the Shell and compositor.
    ///
    /// Returns None if the index is out of bounds or is the current workspace.
    pub fn switch_to(&mut self, index: usize) -> Option<SwitchResult> {
        if index >= self.workspaces.len() || index == self.current {
            return None;
        }

        let old_idx = self.current;
        let old_ids = self.workspaces[old_idx].window_ids.clone();
        let new_ids = self.workspaces[index].window_ids.clone();

        self.current = index;

        Some(SwitchResult {
            to_hide: old_ids,
            to_show: new_ids,
            new_index: index,
            new_name: self.workspaces[index].name.clone(),
        })
    }

    /// Switch to the previous workspace (wrapping).
    pub fn switch_prev(&mut self) -> Option<SwitchResult> {
        let prev = if self.current == 0 {
            self.workspaces.len() - 1
        } else {
            self.current - 1
        };
        self.switch_to(prev)
    }

    /// Switch to the next workspace (wrapping).
    pub fn switch_next(&mut self) -> Option<SwitchResult> {
        let next = if self.current + 1 >= self.workspaces.len() {
            0
        } else {
            self.current + 1
        };
        self.switch_to(next)
    }
}

impl Default for WorkspaceManager {
    fn default() -> Self {
        Self::default_workspaces()
    }
}

// ════════════════════════════════════════════════════════════════
// Tests
// ════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_workspaces() {
        let wm = WorkspaceManager::default_workspaces();
        assert_eq!(wm.workspace_count(), 4);
        assert_eq!(wm.current_index(), 0);
        assert_eq!(wm.current_name(), "1");
    }

    #[test]
    fn custom_count() {
        let wm = WorkspaceManager::new(2);
        assert_eq!(wm.workspace_count(), 2);
        assert_eq!(wm.current_name(), "1");

        let wm = WorkspaceManager::new(6);
        assert_eq!(wm.workspace_count(), 6);
        assert_eq!(wm.get_workspace(4).unwrap().name, "5");
        assert_eq!(wm.get_workspace(5).unwrap().name, "6");
    }

    #[test]
    fn add_window() {
        let mut wm = WorkspaceManager::default_workspaces();
        assert!(wm.add_window(10));
        assert!(wm.is_on_current(10));
        assert_eq!(wm.current_window_ids().len(), 1);

        // Add another
        assert!(wm.add_window(20));
        assert_eq!(wm.current_window_ids().len(), 2);

        // Duplicate
        assert!(!wm.add_window(10));
        assert_eq!(wm.current_window_ids().len(), 2);
    }

    #[test]
    fn remove_window() {
        let mut wm = WorkspaceManager::default_workspaces();
        wm.add_window(10);
        wm.add_window(20);
        assert_eq!(wm.current_window_ids().len(), 2);

        let ws = wm.remove_window(10);
        assert_eq!(ws, Some(0));
        assert!(!wm.is_on_current(10));
        assert_eq!(wm.current_window_ids().len(), 1);

        // Remove non-existent
        assert!(wm.remove_window(999).is_none());
    }

    #[test]
    fn move_window_between_workspaces() {
        let mut wm = WorkspaceManager::default_workspaces();
        wm.add_window(10);
        wm.add_window(20);
        assert_eq!(wm.current_window_ids().len(), 2);

        // Move window to workspace 2
        assert!(wm.move_window(10, 1));
        assert!(!wm.is_on_current(10));
        assert_eq!(wm.current_window_ids().len(), 1);
        assert_eq!(wm.get_workspace(1).unwrap().window_ids.len(), 1);

        // Move to invalid workspace
        assert!(!wm.move_window(20, 99));
    }

    #[test]
    fn switch_workspace() {
        let mut wm = WorkspaceManager::default_workspaces();
        wm.add_window(10);
        wm.add_window(20);

        // Add a window to workspace 2 by switching, adding, switching back
        wm.switch_to(1);
        wm.add_window(30);
        wm.switch_to(0);

        assert!(wm.is_on_current(10));
        assert!(wm.is_on_current(20));
        assert!(!wm.is_on_current(30));

        // Switch to workspace 2
        let result = wm.switch_to(1);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.to_hide, alloc::vec![10, 20]);
        assert_eq!(r.to_show, alloc::vec![30]);
        assert_eq!(r.new_index, 1);
        assert_eq!(r.new_name, "2");

        assert!(wm.is_on_current(30));
    }

    #[test]
    fn switch_to_same_workspace_returns_none() {
        let mut wm = WorkspaceManager::default_workspaces();
        assert!(wm.switch_to(0).is_none());
    }

    #[test]
    fn switch_to_invalid_returns_none() {
        let mut wm = WorkspaceManager::default_workspaces();
        assert!(wm.switch_to(99).is_none());
    }

    #[test]
    fn switch_prev_next_wrapping() {
        let mut wm = WorkspaceManager::new(3);
        assert_eq!(wm.current_index(), 0);

        // Next from 0 → 1
        let r = wm.switch_next();
        assert!(r.is_some());
        assert_eq!(r.unwrap().new_index, 1);

        // Next from 1 → 2
        let r = wm.switch_next();
        assert!(r.is_some());
        assert_eq!(r.unwrap().new_index, 2);

        // Next from 2 → 0 (wrap)
        let r = wm.switch_next();
        assert!(r.is_some());
        assert_eq!(r.unwrap().new_index, 0);

        // Prev from 0 → 2 (wrap)
        let r = wm.switch_prev();
        assert!(r.is_some());
        assert_eq!(r.unwrap().new_index, 2);
    }

    #[test]
    fn rename_workspace() {
        let mut wm = WorkspaceManager::default_workspaces();
        assert!(wm.rename_workspace(0, "Web".into()));
        assert_eq!(wm.current_name(), "Web");
        assert_eq!(wm.get_workspace(0).unwrap().name, "Web");

        // Invalid index
        assert!(!wm.rename_workspace(99, "Test".into()));
    }

    #[test]
    fn find_window_workspace() {
        let mut wm = WorkspaceManager::default_workspaces();
        wm.add_window(10);
        wm.switch_to(1);
        wm.add_window(20);

        assert_eq!(wm.find_window_workspace(10), Some(0));
        assert_eq!(wm.find_window_workspace(20), Some(1));
        assert_eq!(wm.find_window_workspace(999), None);
    }

    #[test]
    fn window_counts() {
        let mut wm = WorkspaceManager::new(3);
        wm.add_window(10);
        wm.add_window(20);
        wm.switch_to(1);
        wm.add_window(30);

        let counts = wm.window_counts();
        assert_eq!(counts, alloc::vec![2, 1, 0]);
    }

    #[test]
    fn remove_window_cleans_up_correctly() {
        let mut wm = WorkspaceManager::default_workspaces();
        wm.add_window(10);
        wm.switch_to(1);
        wm.add_window(10); // moved to workspace 1

        assert!(wm.is_on_current(10));
        assert_eq!(wm.remove_window(10), Some(1));
        assert!(!wm.is_on_current(10));
    }
}
