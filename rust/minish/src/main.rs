//! # minish — Minimal Rust Shell for GergiOS
//!
//! A lightweight, safe, POSIX-compatible shell with:
//! - Built-in commands (cd, ls, echo, pwd, cat, rm, mv, cp, mkdir, ps, kill, exit, help, export, source)
//! - Command pipelines (`cmd1 | cmd2`)
//! - I/O redirection (`>`, `>>`, `<`)
//! - Background jobs (`&`)
//! - Tab completion
//! - Command history
//! - Colorized prompt
//!
//! ## Architecture
//!
//! ```text
//! main() — REPL loop
//!   │
//!   ├── prompt::render()        — colorized prompt (user@host:cwd$)
//!   ├── read_line()             — line editing with history + completion
//!   ├── parser::parse_line()    — tokenize → Command/Pipeline
//!   ├── executor::execute()     — builtin or fork/exec
//!   └── (loop)
//! ```

mod builtins;
mod complete;
mod exec;
mod history;
mod input;
mod parser;
mod prompt;

use std::io::{self, BufRead};

/// Main entry point — runs the REPL loop.
fn main() {
    let mut hist = history::History::new(500);
    let mut last_exit_code: i32 = 0;

    // Non-interactive mode: execute script from stdin
    if !is_interactive() {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(text) => {
                    let trimmed = text.trim().to_string();
                    if !trimmed.is_empty() && !trimmed.starts_with('#') {
                        last_exit_code = execute_line(&trimmed, &mut hist, last_exit_code);
                    }
                }
                Err(_) => break,
            }
        }
        std::process::exit(last_exit_code);
    }

    // Interactive REPL loop

    // Print welcome
    println!("minish v{} — Minimal GergiOS Shell", env!("CARGO_PKG_VERSION"));
    println!("Type 'help' for available commands.\n");

    loop {
        // Render prompt
        let prompt_str = prompt::render(last_exit_code);

        // Read a line with raw-mode editing (history, tab-completion, cursor)
        let line = match input::read_line(&prompt_str, &mut hist, last_exit_code) {
            Some(l) => l,
            None => {
                // Ctrl+D (EOF)
                println!("exit");
                break;
            }
        };

        let trimmed = line.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }

        // Add to history
        hist.add(&trimmed);

        // Check for exit
        if trimmed == "exit" || trimmed == "quit" {
            println!("exit");
            break;
        }

        last_exit_code = execute_line(&trimmed, &mut hist, last_exit_code);
    }
}

/// Execute a parsed command line.
fn execute_line(line: &str, _hist: &mut history::History, last_ec: i32) -> i32 {
    // Parse the command line
    let pipeline = match parser::parse_line(line) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("minish: parse error: {}", e);
            return 1;
        }
    };

    if pipeline.commands.is_empty() {
        return 0;
    }

    // Single command execution
    if pipeline.commands.len() == 1 {
        let cmd = &pipeline.commands[0];
        if cmd.args.is_empty() {
            return 0;
        }

        let command = &cmd.args[0];
        let cmd_args = &cmd.args[1..];

        // Try builtins first
        if let Some(code) = builtins::try_builtin(command, cmd_args) {
            return code;
        }

        // External command
        exec::run_external(cmd, last_ec)
    } else {
        // Pipeline: multiple commands
        exec::run_pipeline(&pipeline, last_ec)
    }
}

/// Check if running interactively (has a TTY).
fn is_interactive() -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::isatty(libc::STDIN_FILENO) != 0 }
    }
    #[cfg(not(unix))]
    {
        true // assume interactive on non-Unix
    }
}
