//! # External Command Executor
//!
//! Executes external programs via `std::process::Command`.
//! Handles pipelines, I/O redirection, and background jobs.
//!
//! ## Architecture
//!
//! ```text
//! run_external(cmd)    — single command with optional redirects
//! run_pipeline(cmds)    — chain of piped commands
//! ```
//!
//! On MINIX, external commands are found via `$PATH` and executed
//! with `fork()`/`execve()` (through `std::process::Command`).

use crate::parser::{Command, Pipeline};
use std::io::{self, Write};
use std::process::{Command as ProcCommand, Stdio};

/// Exit code used when a command could not be found/executed.
const CMD_NOT_FOUND: i32 = 127;

/// Run a single external command with optional redirects.
///
/// Returns the exit code.
pub fn run_external(cmd: &Command, _last_exit_code: i32) -> i32 {
    if cmd.args.is_empty() {
        return 0;
    }

    let program = &cmd.args[0];
    let args = &cmd.args[1..];

    let mut proc = ProcCommand::new(program);
    proc.args(args);

    // stdin redirect
    if let Some(ref file) = cmd.stdin_file {
        proc.stdin(Stdio::from(match std::fs::File::open(file) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("minish: {}: {}", file, e);
                return 1;
            }
        }));
    }

    // stdout redirect
    if let Some(ref file) = cmd.stdout_file {
        proc.stdout(Stdio::from(match std::fs::File::create(file) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("minish: {}: {}", file, e);
                return 1;
            }
        }));
    } else if let Some(ref file) = cmd.stdout_append {
        proc.stdout(Stdio::from(match std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(file)
        {
            Ok(f) => f,
            Err(e) => {
                eprintln!("minish: {}: {}", file, e);
                return 1;
            }
        }));
    }

    // stderr redirect
    if let Some(ref file) = cmd.stderr_file {
        proc.stderr(Stdio::from(match std::fs::File::create(file) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("minish: {}: {}", file, e);
                return 1;
            }
        }));
    }

    // Execute
    match proc.output() {
        Ok(output) => {
            // Print stdout/stderr
            io::stdout().write_all(&output.stdout).ok();
            io::stderr().write_all(&output.stderr).ok();

            output.status.code().unwrap_or(1)
        }
        Err(e) => {
            // Check if command was not found
            if e.kind() == io::ErrorKind::NotFound {
                eprintln!("minish: {}: command not found", program);
                CMD_NOT_FOUND
            } else {
                eprintln!("minish: {}: {}", program, e);
                1
            }
        }
    }
}

/// Run a pipeline of commands.
///
/// For now, simple sequential execution with conditional operators.
/// Full pipe implementation will use OS pipes.
pub fn run_pipeline(pipeline: &Pipeline, _last_exit_code: i32) -> i32 {
    if pipeline.commands.is_empty() {
        return 0;
    }

    // Simple sequential execution with conditional operators
    let mut exit_code = 0;

    for cmd in &pipeline.commands {
        if cmd.args.is_empty() {
            continue;
        }

        // Check conditional
        if let Some(cond) = pipeline.conditional {
            match cond {
                crate::parser::Conditional::And => {
                    if exit_code != 0 {
                        break; // short-circuit on failure
                    }
                }
                crate::parser::Conditional::Or => {
                    if exit_code == 0 {
                        break; // short-circuit on success
                    }
                }
            }
        }

        let command = &cmd.args[0];
        let cmd_args = &cmd.args[1..];

        // Try builtins first
        exit_code = if let Some(code) = crate::builtins::try_builtin(command, cmd_args) {
            code
        } else {
            run_external(cmd, exit_code)
        };
    }

    exit_code
}

/// Run a background job.
///
/// Spawns the command in a new process and returns immediately.
/// The job's PID is printed for later management.
#[allow(dead_code)]
pub fn run_background(cmd: &Command) -> i32 {
    let program = &cmd.args[0];
    let args = &cmd.args[1..];

    match ProcCommand::new(program).args(args).spawn() {
        Ok(child) => {
            println!("[{}] {}", child.id(), program);
            0
        }
        Err(e) => {
            eprintln!("minish: {}: {}", program, e);
            1
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_empty_command() {
        let cmd = Command {
            args: vec![],
            stdin_file: None,
            stdout_file: None,
            stdout_append: None,
            stderr_file: None,
            background: false,
        };
        assert_eq!(run_external(&cmd, 0), 0);
    }

    #[test]
    fn test_run_nonexistent_command() {
        let cmd = Command {
            args: vec!["nonexistent_cmd_xyz_999".to_string()],
            stdin_file: None,
            stdout_file: None,
            stdout_append: None,
            stderr_file: None,
            background: false,
        };
        let code = run_external(&cmd, 0);
        assert_eq!(code, CMD_NOT_FOUND);
    }

    #[test]
    fn test_run_pipeline_empty() {
        let pipeline = Pipeline {
            commands: vec![],
            conditional: None,
        };
        assert_eq!(run_pipeline(&pipeline, 0), 0);
    }

    #[test]
    fn test_run_pipeline_single() {
        let cmd = Command {
            args: vec!["echo".to_string(), "hello".to_string()],
            stdin_file: None,
            stdout_file: None,
            stdout_append: None,
            stderr_file: None,
            background: false,
        };
        let pipeline = Pipeline {
            commands: vec![cmd],
            conditional: None,
        };
        // echo is a builtin, should succeed
        assert_eq!(run_pipeline(&pipeline, 0), 0);
    }
}
