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
mod jobs;
mod parser;
mod prompt;
mod shellopts;

use std::io::{self, BufRead};

/// Main entry point — runs the REPL loop.
fn main() {
    // Set up signal handling for job control
    setup_signal_handlers();

    let mut hist = history::History::new(500);
    let mut jobs_mgr = jobs::JobManager::new();
    let mut shell_opts = shellopts::ShellOptions::new();
    let mut last_exit_code: i32 = 0;

    // Non-interactive mode: execute script from stdin
    if !is_interactive() {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(text) => {
                    let trimmed = text.trim().to_string();
                    if !trimmed.is_empty() && !trimmed.starts_with('#') {
                        last_exit_code = execute_line(&trimmed, &mut hist, &mut jobs_mgr, &mut shell_opts, last_exit_code);
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
        // Reap completed background jobs and print notifications
        reap_background_jobs(&mut jobs_mgr);

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

        last_exit_code = execute_line(&trimmed, &mut hist, &mut jobs_mgr, &mut shell_opts, last_exit_code);
    }
}

/// Execute a parsed command line.
///
/// Delegates everything to `exec::run_pipeline()`, which handles:
/// - Single commands (builtins first, then external) with job control
/// - Conditional chains (`&&`, `||`)
/// - Pipe chains (`cmd1 | cmd2`) with real OS pipes
/// - Background jobs (`cmd &`) via JobManager
/// - Shell options (`-e`, `pipefail`) for script behavior
fn execute_line(
    line: &str,
    _hist: &mut history::History,
    jobs: &mut jobs::JobManager,
    opts: &mut shellopts::ShellOptions,
    last_ec: i32,
) -> i32 {
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

    exec::run_pipeline(&pipeline, last_ec, jobs, opts)
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

/// Set up signal handlers for job control.
///
/// The shell ignores SIGINT and SIGTSTP so they don't kill the shell
/// itself. Children reset these to default via pre_exec.
fn setup_signal_handlers() {
    #[cfg(unix)]
    {
        unsafe {
            // Ignore SIGINT so Ctrl+C doesn't kill the shell
            libc::signal(libc::SIGINT, libc::SIG_IGN);
            // Ignore SIGTSTP so Ctrl+Z doesn't stop the shell
            libc::signal(libc::SIGTSTP, libc::SIG_IGN);
            // Ignore SIGQUIT so Ctrl+\ doesn't kill the shell
            libc::signal(libc::SIGQUIT, libc::SIG_IGN);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = ();
    }
}

/// Check for completed/stopped background jobs and print notifications.
///
/// Uses non-blocking waitpid (WNOHANG) to check all tracked pgrps
/// without blocking the shell loop.
fn reap_background_jobs(jobs: &mut jobs::JobManager) {
    #[cfg(unix)]
    {
        // Collect pgrps to check before modifying jobs
        let pgrps: Vec<i32> = jobs.list().iter()
            .filter(|j| matches!(j.state, jobs::JobState::Running | jobs::JobState::Stopped))
            .map(|j| j.pgrp)
            .collect();

        let mut changed = false;
        for pgrp in pgrps {
            // Loop until no more children in this pgrp (handles multi-process jobs)
            loop {
                let mut status: i32 = 0;
                let ret = unsafe {
                    libc::waitpid(-pgrp, &mut status, libc::WNOHANG | libc::WUNTRACED)
                };

                if ret == 0 || ret == -1 {
                    break; // no more children to reap
                }

                if unsafe { libc::WIFEXITED(status) } {
                    let code = unsafe { libc::WEXITSTATUS(status) };
                    jobs.update_state(pgrp, jobs::JobState::Done(code as i32));
                    changed = true;
                } else if unsafe { libc::WIFSIGNALED(status) } {
                    let sig = unsafe { libc::WTERMSIG(status) };
                    jobs.update_state(pgrp, jobs::JobState::Killed(sig));
                    changed = true;
                } else if unsafe { libc::WIFSTOPPED(status) } {
                    jobs.update_state(pgrp, jobs::JobState::Stopped);
                    changed = true;
                }
            }
        }

        if changed {
            jobs.print_notifications();
            jobs.reap();
        }
    }
    #[cfg(not(unix))]
    {
        let _ = jobs;
    }
}
