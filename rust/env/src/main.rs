//! Rust port of the MINIX/NetBSD `env` utility.
//!
//! Usage:
//!   env [-i] [name=value ...] [utility [args ...]]
//!
//! Prints the current environment or runs a utility with a modified environment.

use std::env;
use std::io::{self, Write};
use std::process::Command;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut ignore_env = false;

    // Parse -i flag
    if !argv.is_empty() && argv[0] == "-i" {
        ignore_env = true;
        argv = &argv[1..];
    }

    if argv.is_empty() {
        // Print environment
        let stdout = io::stdout();
        let mut out = stdout.lock();
        for (key, val) in env::vars() {
            let _ = writeln!(out, "{key}={val}");
        }
        return;
    }

    // Parse name=value assignments
    let mut assignments: Vec<(String, String)> = Vec::new();
    let mut cmd_start = 0;
    for (i, arg) in argv.iter().enumerate() {
        if let Some(eq) = arg.find('=') {
            if eq > 0 {
                let name = arg[..eq].to_string();
                let val = arg[eq + 1..].to_string();
                assignments.push((name, val));
                cmd_start = i + 1;
                continue;
            }
        }
        break;
    }

    if cmd_start >= argv.len() {
        eprintln!("env: utility argument required");
        std::process::exit(1);
    }

    // Build command
    let prog = &argv[cmd_start];
    let prog_args: Vec<&str> = argv[cmd_start + 1..].iter().map(|s| s.as_str()).collect();

    let mut cmd = Command::new(prog);
    cmd.args(&prog_args);

    if ignore_env {
        cmd.env_clear();
    }
    for (name, val) in &assignments {
        cmd.env(name, val);
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("env: {prog}: {e}");
            std::process::exit(1);
        }
    };

    let status = child.wait().unwrap_or_else(|_| std::process::exit(1));
    std::process::exit(status.code().unwrap_or(1));
}
