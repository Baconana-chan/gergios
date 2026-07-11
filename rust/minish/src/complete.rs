//! # Tab Completion
//!
//! Completes command names (builtins and PATH executables),
//! file paths, and environment variable names.
//!
//! ## Features
//!
//! - First word: complete command names (builtins + PATH)
//! - Subsequent words: complete file paths
//! - Words starting with `$`: complete env var names

/// List of built-in command names for completion.
const BUILTINS: &[&str] = &[
    "cd", "pwd", "echo", "ls", "cat", "rm", "mv", "cp", "mkdir",
    "ps", "kill", "help", "export", "source", "exit", "quit",
    "true", "false",
];

/// Get completions for a partial word.
///
/// `is_first_word`: whether this is the first word (command name).
/// `partial`: the partial word to complete.
///
/// Returns a vector of matching completions.
pub fn complete(partial: &str, is_first_word: bool) -> Vec<String> {
    if partial.is_empty() {
        // Return all builtins
        if is_first_word {
            return BUILTINS.iter().map(|s| s.to_string()).collect();
        }
        // For empty path completion, return nothing (too many results)
        return Vec::new();
    }

    // Environment variable completion ($VAR)
    if partial.starts_with('$') {
        return complete_env_var(&partial[1..]);
    }

    // File path completion
    if !is_first_word || partial.contains('/') {
        return complete_path(partial);
    }

    // Command name completion
    if is_first_word {
        let mut results: Vec<String> = BUILTINS
            .iter()
            .filter(|cmd| cmd.starts_with(partial))
            .map(|s| s.to_string())
            .collect();

        // Also search PATH for external commands
        if let Ok(paths) = std::env::var("PATH") {
            for dir in std::env::split_paths(&paths) {
                if let Ok(entries) = std::fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        if let Some(name) = entry.file_name().to_str() {
                            if name.starts_with(partial) && !results.contains(&name.to_string()) {
                                results.push(name.to_string());
                            }
                        }
                    }
                }
            }
        }

        results.sort();
        results.dedup();
        results.truncate(50); // limit results
        return results;
    }

    Vec::new()
}

/// Complete file paths.
fn complete_path(partial: &str) -> Vec<String> {
    let (dir, prefix) = if let Some(pos) = partial.rfind(|c: char| c == '/' || c == '\\') {
        let d = &partial[..=pos];
        let p = &partial[pos + 1..];
        (d.to_string(), p.to_string())
    } else {
        (String::new(), partial.to_string())
    };

    let search_dir = if dir.is_empty() {
        "."
    } else {
        dir.trim_end_matches('/')
    };

    if search_dir.is_empty() {
        return Vec::new();
    }

    let mut results: Vec<String> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(search_dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.starts_with(&prefix) {
                    let mut full = dir.clone();
                    full.push_str(name);
                    if entry.file_type().map_or(false, |t| t.is_dir()) {
                        full.push('/');
                    }
                    results.push(full);
                }
            }
        }
    }

    results.sort();
    results
}

/// Complete environment variable names.
fn complete_env_var(partial: &str) -> Vec<String> {
    let mut results: Vec<String> = Vec::new();

    for (key, _value) in std::env::vars() {
        if key.starts_with(partial) {
            results.push(format!("${}", key));
        }
    }

    results.sort();
    results.truncate(20);
    results
}

/// Find the longest common prefix of a list of strings.
pub fn longest_common_prefix(strings: &[String]) -> String {
    if strings.is_empty() {
        return String::new();
    }
    if strings.len() == 1 {
        return strings[0].clone();
    }

    let first = &strings[0];
    let mut prefix_len = first.len();

    for s in &strings[1..] {
        prefix_len = prefix_len.min(s.len());
        for (i, (a, b)) in first[..prefix_len].chars().zip(s.chars()).enumerate() {
            if a != b {
                prefix_len = i;
                break;
            }
        }
    }

    first[..prefix_len].to_string()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complete_builtin() {
        let completions = complete("ec", true);
        assert!(completions.contains(&"echo".to_string()));
    }

    #[test]
    fn test_complete_empty() {
        let completions = complete("", true);
        assert!(!completions.is_empty());
        assert!(completions.contains(&"cd".to_string()));
        assert!(completions.contains(&"ls".to_string()));
    }

    #[test]
    fn test_complete_no_match() {
        let completions = complete("zzznonexistent", true);
        assert!(completions.is_empty());
    }

    #[test]
    fn test_complete_not_first_word() {
        // Second word should try file completion
        let completions = complete("", false);
        // For empty string, second word returns nothing (too many)
        assert!(completions.is_empty());
    }

    #[test]
    fn test_longest_common_prefix() {
        assert_eq!(
            longest_common_prefix(&["echo".to_string(), "ecl".to_string()]),
            "ec"
        );
    }

    #[test]
    fn test_longest_common_prefix_empty() {
        assert_eq!(longest_common_prefix(&[] as &[String]), "");
    }

    #[test]
    fn test_longest_common_prefix_single() {
        assert_eq!(longest_common_prefix(&["hello".to_string()]), "hello");
    }

    #[test]
    fn test_complete_env_var() {
        // Set a test variable
        std::env::set_var("MINISH_TEST_VAR", "test");
        let completions = complete_env_var("MINISH_TEST");
        assert!(completions.iter().any(|s| s.contains("MINISH_TEST_VAR")));
    }

    #[test]
    fn test_complete_path_doesnt_panic() {
        // Just verify the function doesn't panic with various inputs
        complete_path("/");
        complete_path("/nonexistent");
        complete_path("");
    }
}
