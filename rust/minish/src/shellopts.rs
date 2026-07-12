//! # Shell Options — `set -e`, `set -o pipefail`
//!
//! Manages shell options that control script execution behavior:
//!
//! - **`set -e`** (`exit_on_error`): Exit immediately if a command
//!   exits with a non-zero status. Exceptions: commands in `&&`/`||`
//!   lists that are NOT the last command (POSIX behavior).
//!
//! - **`set -o pipefail`** (`pipefail`): Pipeline exit code is the
//!   rightmost non-zero exit code (instead of the last command's).
//!
//! ## `set` syntax
//!
//! ```text
//! set              — show current options
//! set -e           — enable exit on error
//! set +e           — disable exit on error
//! set -o pipefail  — enable pipefail
//! set +o pipefail  — disable pipefail
//! ```

/// Shell option flags.
#[derive(Debug, Clone, Copy)]
pub struct ShellOptions {
    /// `set -e`: exit on error (non-zero exit code).
    pub exit_on_error: bool,
    /// `set -o pipefail`: pipeline exit code = rightmost non-zero.
    pub pipefail: bool,
    /// Internal flag: set by `exec_sequential` when short-circuit from
    /// a non-last AND-OR command suppressed `-e`. Checked by `source()`
    /// to avoid incorrectly triggering `-e` on short-circuit exits.
    pub suppress_set_e: bool,
}

impl ShellOptions {
    /// Create new options with defaults (both disabled).
    pub fn new() -> Self {
        ShellOptions {
            exit_on_error: false,
            pipefail: false,
            suppress_set_e: false,
        }
    }

    /// Apply `set` command arguments to these options.
    ///
    /// Returns `0` on success, `1` on error.
    pub fn apply(args: &[String], opts: &mut ShellOptions) -> i32 {
        if args.is_empty() {
            // `set` with no args: show current options
            println!("exit_on_error (-e): {}", if opts.exit_on_error { "on" } else { "off" });
            println!("pipefail (-o pipefail): {}", if opts.pipefail { "on" } else { "off" });
            return 0;
        }

        let mut exit_code = 0;
        let mut i = 0;

        while i < args.len() {
            let arg = &args[i];

            match arg.as_str() {
                "-e" => {
                    opts.exit_on_error = true;
                }
                "+e" => {
                    opts.exit_on_error = false;
                }
                "-o" => {
                    i += 1;
                    if i >= args.len() {
                        eprintln!("set: -o: missing option name");
                        exit_code = 1;
                        break;
                    }
                    match args[i].as_str() {
                        "pipefail" => opts.pipefail = true,
                        other => {
                            eprintln!("set: -o: unknown option '{}'", other);
                            exit_code = 1;
                        }
                    }
                }
                "+o" => {
                    i += 1;
                    if i >= args.len() {
                        eprintln!("set: +o: missing option name");
                        exit_code = 1;
                        break;
                    }
                    match args[i].as_str() {
                        "pipefail" => opts.pipefail = false,
                        other => {
                            eprintln!("set: +o: unknown option '{}'", other);
                            exit_code = 1;
                        }
                    }
                }
                _ => {
                    eprintln!("set: unrecognized option '{}'", arg);
                    exit_code = 1;
                }
            }

            i += 1;
        }

        exit_code
    }
}

impl Default for ShellOptions {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_options() {
        let opts = ShellOptions::new();
        assert!(!opts.exit_on_error);
        assert!(!opts.pipefail);
    }

    #[test]
    fn test_set_e_on() {
        let mut opts = ShellOptions::new();
        ShellOptions::apply(&["-e".to_string()], &mut opts);
        assert!(opts.exit_on_error);
    }

    #[test]
    fn test_set_e_off() {
        let mut opts = ShellOptions::new();
        opts.exit_on_error = true;
        ShellOptions::apply(&["+e".to_string()], &mut opts);
        assert!(!opts.exit_on_error);
    }

    #[test]
    fn test_set_pipefail_on() {
        let mut opts = ShellOptions::new();
        ShellOptions::apply(&["-o".to_string(), "pipefail".to_string()], &mut opts);
        assert!(opts.pipefail);
    }

    #[test]
    fn test_set_pipefail_off() {
        let mut opts = ShellOptions::new();
        opts.pipefail = true;
        ShellOptions::apply(&["+o".to_string(), "pipefail".to_string()], &mut opts);
        assert!(!opts.pipefail);
    }

    #[test]
    fn test_set_multiple() {
        let mut opts = ShellOptions::new();
        ShellOptions::apply(&[
            "-e".to_string(),
            "-o".to_string(),
            "pipefail".to_string(),
        ], &mut opts);
        assert!(opts.exit_on_error);
        assert!(opts.pipefail);
    }

    #[test]
    fn test_set_empty_args() {
        let mut opts = ShellOptions::new();
        opts.exit_on_error = true;
        // No args: show options (no-op, just prints)
        let result = ShellOptions::apply(&[], &mut opts);
        assert_eq!(result, 0);
        assert!(opts.exit_on_error); // unchanged
    }

    #[test]
    fn test_set_unknown_option() {
        let mut opts = ShellOptions::new();
        let result = ShellOptions::apply(&["-x".to_string()], &mut opts);
        assert_eq!(result, 1);
    }

    #[test]
    fn test_set_unknown_o_option() {
        let mut opts = ShellOptions::new();
        let result = ShellOptions::apply(&["-o".to_string(), "unknown".to_string()], &mut opts);
        assert_eq!(result, 1);
    }

    #[test]
    fn test_set_o_missing_arg() {
        let mut opts = ShellOptions::new();
        let result = ShellOptions::apply(&["-o".to_string()], &mut opts);
        assert_eq!(result, 1);
    }
}
