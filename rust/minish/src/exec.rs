//! # External Command Executor
//!
//! Executes external programs via `std::process::Command`.
//! Handles pipelines, I/O redirection, and background jobs.
//!
//! ## Architecture
//!
//! ```text
//! run_pipeline()          — dispatcher (foreground / background)
//!   ├── exec_single()     — one command (builtin → external)
//!   ├── exec_sequential() — && / || conditionals
//!   └── exec_pipe_chain() — cmd1 | cmd2 | cmd3 (real OS pipes)
//!
//! Job integration (Unix only):
//!   run_background_job()  — spawn single cmd as background job
//!   run_pipe_chain_bg()   — spawn pipe chain as background job
//! ```

use crate::jobs::JobManager;
use crate::parser::{Command, Conditional, Pipeline};
use crate::shellopts::ShellOptions;
use std::io::{self, Write};
use std::process::{Child, ChildStdout, Command as ProcCommand, Stdio};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

/// Exit code used when a command could not be found/executed.
const CMD_NOT_FOUND: i32 = 127;

// ============================================================================
// Public API
// ============================================================================

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
            .append(true).create(true).open(file)
        {
            Ok(f) => f,
            Err(e) => { eprintln!("minish: {}: {}", file, e); return 1; }
        }));
    }

    // stderr redirect
    if let Some(ref file) = cmd.stderr_file {
        proc.stderr(Stdio::from(match std::fs::File::create(file) {
            Ok(f) => f,
            Err(e) => { eprintln!("minish: {}: {}", file, e); return 1; }
        }));
    }

    match proc.output() {
        Ok(output) => {
            io::stdout().write_all(&output.stdout).ok();
            io::stderr().write_all(&output.stderr).ok();
            output.status.code().unwrap_or(1)
        }
        Err(e) => {
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
/// Dispatches to the appropriate execution strategy:
/// - 1 command  → `exec_single()` (builtin or external)
/// - 2+ + conditional → `exec_sequential()` (no pipes)
/// - 2+ + no conditional → `exec_pipe_chain()` (real OS pipes)
///
/// Handles background jobs by checking the pipeline's background flag
/// and registering with the JobManager.
///
/// Shell options (`-e`, `pipefail`) are applied:
/// - `-e`: non-zero exit causes early return (skips remaining commands)
/// - `pipefail`: pipeline returns rightmost non-zero exit code
pub fn run_pipeline(pipeline: &Pipeline, _last_exit_code: i32, jobs: &mut JobManager, opts: &mut ShellOptions) -> i32 {
    let cmds = &pipeline.commands;

    if cmds.is_empty() {
        return 0;
    }

    // Check if any command has the background flag
    let is_bg = cmds.iter().any(|c| c.background);

    // Single command — no pipe needed
    if cmds.len() == 1 {
        let cmd = &cmds[0];
        if cmd.args.is_empty() {
            return 0;
        }

        // Try builtins first
        let command = &cmd.args[0];
        let cmd_args = &cmd.args[1..];
        if let Some(code) = crate::builtins::try_builtin(command, cmd_args, jobs, opts) {
            if opts.exit_on_error && code != 0 {
                return code; // -e triggered
            }
            return code;
        }

        // External command
        let exit_code = if is_bg {
            run_background_job(cmd, jobs)
        } else {
            run_external(cmd, _last_exit_code)
        };

        if opts.exit_on_error && exit_code != 0 {
            return exit_code; // -e triggered
        }
        return exit_code;
    }
    // Conditionals (&&, ||) — sequential execution with short-circuit
    else if let Some(cond) = pipeline.conditional {
        // exec_sequential handles -e internally (only for last command in chain)
        return exec_sequential(cmds, cond, _last_exit_code, jobs, opts);
    }
    // Pure pipe chain — real OS pipes
    else if is_bg {
        return run_pipe_chain_bg(cmds, jobs);
    } else {
        let exit_code = if opts.pipefail {
            exec_pipe_chain_pipefail(cmds)
        } else {
            exec_pipe_chain(cmds)
        };
        if opts.exit_on_error && exit_code != 0 {
            return exit_code; // -e triggered
        }
        return exit_code;
    }
}

// ============================================================================
// Internal helpers
// ============================================================================

/// Execute a single command (try builtin first, then external).
fn exec_single(cmd: &Command, last_ec: i32, jobs: &mut JobManager, opts: &mut ShellOptions) -> i32 {
    if cmd.args.is_empty() {
        return 0;
    }
    let command = &cmd.args[0];
    let cmd_args = &cmd.args[1..];
    if let Some(code) = crate::builtins::try_builtin(command, cmd_args, jobs, opts) {
        return code;
    }
    run_external(cmd, last_ec)
}

/// Sequential execution with conditional short-circuit (&&, ||).
///
/// With `set -e`: only the LAST command in the chain triggers exit-on-error.
/// Earlier commands in &&/|| are exempt per POSIX rules.
fn exec_sequential(cmds: &[Command], cond: Conditional, _last_ec: i32, jobs: &mut JobManager, opts: &mut ShellOptions) -> i32 {
    let mut exit_code = 0;
    for (i, cmd) in cmds.iter().enumerate() {
        if cmd.args.is_empty() { continue; }
        if i > 0 {
            match cond {
                Conditional::And => {
                    if exit_code != 0 {
                        opts.suppress_set_e = true; // short-circuit from non-last cmd
                        break;
                    }
                }
                Conditional::Or  => {
                    if exit_code == 0 {
                        opts.suppress_set_e = true; // short-circuit from non-last cmd
                        break;
                    }
                }
            }
        }
        exit_code = exec_single(cmd, exit_code, jobs, opts);

        // set -e: only trigger for the LAST command in the AND-OR list
        // Non-last commands are exempt per POSIX semantics
        if opts.exit_on_error && exit_code != 0 && i == cmds.len() - 1 {
            break;
        }
    }
    exit_code
}

// ============================================================================
// Background job spawning (Unix only — uses process groups)
// ============================================================================

#[cfg(unix)]
fn setup_child_pgrp(is_leader: bool) -> Result<(), std::io::Error> {
    unsafe {
        libc::signal(libc::SIGINT, libc::SIG_DFL);
        libc::signal(libc::SIGTSTP, libc::SIG_DFL);
        libc::signal(libc::SIGQUIT, libc::SIG_DFL);
        if is_leader {
            libc::setpgid(0, 0);
        } else {
            // pgrp is captured from the closure scope — handled per-call
        }
    }
    Ok(())
}

/// Spawn a single external command as a background job (Unix version).
#[cfg(unix)]
fn run_background_job(cmd: &Command, jobs: &mut JobManager) -> i32 {
    if cmd.args.is_empty() {
        return 0;
    }
    let program = &cmd.args[0];
    let args = &cmd.args[1..];

    let mut proc = ProcCommand::new(program);
    proc.args(args);

    unsafe {
        proc.pre_exec(|| {
            libc::signal(libc::SIGINT, libc::SIG_DFL);
            libc::signal(libc::SIGTSTP, libc::SIG_DFL);
            libc::signal(libc::SIGQUIT, libc::SIG_DFL);
            libc::setpgid(0, 0);
            Ok(())
        });
    }

    match proc.spawn() {
        Ok(child) => {
            let pid = child.id();
            let pgrp = pid as i32;
            // Safety: set pgrp from parent side too
            unsafe { libc::setpgid(pid as i32, pgrp); }
            let _ = child; // detach
            let command_str = format!("{} {}", program, args.join(" "));
            jobs.add(pgrp, pid, &command_str);
            if let Some(job) = jobs.list().last() {
                println!("[{}] {}", job.id, pid);
            }
            0
        }
        Err(e) => {
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

/// Spawn a single external command as a background job (non-Unix fallback).
#[cfg(not(unix))]
fn run_background_job(cmd: &Command, jobs: &mut JobManager) -> i32 {
    if cmd.args.is_empty() {
        return 0;
    }
    let program = &cmd.args[0];
    let args = &cmd.args[1..];

    match ProcCommand::new(program).args(args).spawn() {
        Ok(child) => {
            let pid = child.id();
            let _ = child;
            let command_str = format!("{} {}", program, args.join(" "));
            jobs.add(pid as i32, pid, &command_str);
            if let Some(job) = jobs.list().last() {
                println!("[{}] {}", job.id, pid);
            }
            0
        }
        Err(e) => {
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

/// Run a pipe chain as a background job (Unix version with process groups).
#[cfg(unix)]
fn run_pipe_chain_bg(cmds: &[Command], jobs: &mut JobManager) -> i32 {
    let n = cmds.len();
    let mut children: Vec<Child> = Vec::with_capacity(n);
    let mut prev_stdout: Option<ChildStdout> = None;
    let mut pgrp: i32 = 0;

    for (i, cmd) in cmds.iter().enumerate() {
        if cmd.args.is_empty() { continue; }
        let program = &cmd.args[0];
        let args = &cmd.args[1..];
        let mut proc = ProcCommand::new(program);
        proc.args(args);

        // stdin
        if i == 0 {
            if let Some(ref file) = cmd.stdin_file {
                match std::fs::File::open(file) {
                    Ok(f) => { proc.stdin(f); }
                    Err(e) => { eprintln!("minish: {}: {}", file, e); kill_children(&mut children); return 1; }
                }
            } else {
                proc.stdin(Stdio::null());
            }
        } else if let Some(prev_out) = prev_stdout.take() {
            proc.stdin(prev_out);
        }

        // stdout
        let is_last = i == n - 1;
        if is_last {
            if let Some(ref file) = cmd.stdout_file {
                match std::fs::File::create(file) {
                    Ok(f) => { proc.stdout(f); }
                    Err(e) => { eprintln!("minish: {}: {}", file, e); kill_children(&mut children); return 1; }
                }
            } else if let Some(ref file) = cmd.stdout_append {
                match std::fs::OpenOptions::new().append(true).create(true).open(file) {
                    Ok(f) => { proc.stdout(f); }
                    Err(e) => { eprintln!("minish: {}: {}", file, e); kill_children(&mut children); return 1; }
                }
            } else {
                proc.stdout(Stdio::null());
            }
        } else {
            proc.stdout(Stdio::piped());
        }

        if let Some(ref file) = cmd.stderr_file {
            match std::fs::File::create(file) {
                Ok(f) => { proc.stderr(f); }
                Err(e) => { eprintln!("minish: {}: {}", file, e); kill_children(&mut children); return 1; }
            }
        }

        // pre_exec: reset signals and set pgrp
        let is_leader = i == 0;
        let leader_pgrp = if is_leader { 0 } else { pgrp };
        unsafe {
            proc.pre_exec(move || {
                libc::signal(libc::SIGINT, libc::SIG_DFL);
                libc::signal(libc::SIGTSTP, libc::SIG_DFL);
                libc::signal(libc::SIGQUIT, libc::SIG_DFL);
                if is_leader {
                    libc::setpgid(0, 0);
                } else {
                    libc::setpgid(0, leader_pgrp);
                }
                Ok(())
            });
        }

        match proc.spawn() {
            Ok(mut child) => {
                if is_leader {
                    pgrp = child.id() as i32;
                }
                // Also set from parent
                unsafe { libc::setpgid(child.id() as i32, pgrp); }
                if !is_last { prev_stdout = child.stdout.take(); }
                children.push(child);
            }
            Err(e) => {
                let msg = if e.kind() == io::ErrorKind::NotFound {
                    format!("minish: {}: command not found", program)
                } else {
                    format!("minish: {}: {}", program, e)
                };
                eprintln!("{}", msg);
                kill_children(&mut children);
                return CMD_NOT_FOUND;
            }
        }
    }

    if pgrp > 0 && !children.is_empty() {
        let first_pid = children[0].id();
        let cmd_str = cmds.iter()
            .map(|c| c.args.join(" "))
            .collect::<Vec<_>>()
            .join(" | ");
        jobs.add(pgrp, first_pid, &cmd_str);
        if let Some(job) = jobs.list().last() {
            println!("[{}] {}", job.id, pgrp);
        }
    }
    0
}

/// Run a pipe chain as a background job (non-Unix fallback — no pgrp).
#[cfg(not(unix))]
fn run_pipe_chain_bg(cmds: &[Command], jobs: &mut JobManager) -> i32 {
    let n = cmds.len();
    let mut children: Vec<Child> = Vec::with_capacity(n);
    let mut prev_stdout: Option<ChildStdout> = None;

    for (i, cmd) in cmds.iter().enumerate() {
        if cmd.args.is_empty() { continue; }
        let program = &cmd.args[0];
        let args = &cmd.args[1..];
        let mut proc = ProcCommand::new(program);
        proc.args(args);

        if i == 0 {
            if let Some(ref file) = cmd.stdin_file {
                match std::fs::File::open(file) {
                    Ok(f) => { proc.stdin(f); }
                    Err(e) => { eprintln!("minish: {}: {}", file, e); kill_children(&mut children); return 1; }
                }
            } else { proc.stdin(Stdio::null()); }
        } else if let Some(prev_out) = prev_stdout.take() { proc.stdin(prev_out); }

        let is_last = i == n - 1;
        if is_last {
            if let Some(ref file) = cmd.stdout_file {
                match std::fs::File::create(file) {
                    Ok(f) => { proc.stdout(f); }
                    Err(e) => { eprintln!("minish: {}: {}", file, e); kill_children(&mut children); return 1; }
                }
            } else { proc.stdout(Stdio::null()); }
        } else { proc.stdout(Stdio::piped()); }

        if let Some(ref file) = cmd.stderr_file {
            match std::fs::File::create(file) {
                Ok(f) => { proc.stderr(f); }
                Err(e) => { eprintln!("minish: {}: {}", file, e); kill_children(&mut children); return 1; }
            }
        }

        match proc.spawn() {
            Ok(mut child) => {
                if !is_last { prev_stdout = child.stdout.take(); }
                children.push(child);
            }
            Err(e) => {
                let msg = if e.kind() == io::ErrorKind::NotFound {
                    format!("minish: {}: command not found", program)
                } else {
                    format!("minish: {}: {}", program, e)
                };
                eprintln!("{}", msg);
                kill_children(&mut children);
                return CMD_NOT_FOUND;
            }
        }
    }

    if !children.is_empty() {
        let first_pid = children[0].id();
        let cmd_str = cmds.iter()
            .map(|c| c.args.join(" "))
            .collect::<Vec<_>>()
            .join(" | ");
        jobs.add(first_pid as i32, first_pid, &cmd_str);
        if let Some(job) = jobs.list().last() {
            println!("[{}] {}", job.id, first_pid);
        }
    }
    0
}

// ============================================================================
// Foreground pipe chain (cross-platform)
// ============================================================================

/// Execute a pipe chain: spawn all commands as external processes,
/// connect stdout→stdin via OS pipes, wait for all children,
/// return exit code of the last command.
///
/// On Unix, uses process group management and WUNTRACED for Ctrl+Z.
fn exec_pipe_chain(cmds: &[Command]) -> i32 {
    let n = cmds.len();
    let mut children: Vec<Child> = Vec::with_capacity(n);
    let mut prev_stdout: Option<ChildStdout> = None;

    for (i, cmd) in cmds.iter().enumerate() {
        if cmd.args.is_empty() { continue; }

        let program = &cmd.args[0];
        let args = &cmd.args[1..];
        let mut proc = ProcCommand::new(program);
        proc.args(args);

        // stdin
        if i == 0 {
            if let Some(ref file) = cmd.stdin_file {
                match std::fs::File::open(file) {
                    Ok(f) => { proc.stdin(f); }
                    Err(e) => { eprintln!("minish: {}: {}", file, e); kill_children(&mut children); return 1; }
                }
            } else { proc.stdin(Stdio::inherit()); }
        } else if let Some(prev_out) = prev_stdout.take() { proc.stdin(prev_out); }

        // stdout
        let is_last = i == n - 1;
        if is_last {
            if let Some(ref file) = cmd.stdout_file {
                match std::fs::File::create(file) {
                    Ok(f) => { proc.stdout(f); }
                    Err(e) => { eprintln!("minish: {}: {}", file, e); kill_children(&mut children); return 1; }
                }
            } else if let Some(ref file) = cmd.stdout_append {
                match std::fs::OpenOptions::new().append(true).create(true).open(file) {
                    Ok(f) => { proc.stdout(f); }
                    Err(e) => { eprintln!("minish: {}: {}", file, e); kill_children(&mut children); return 1; }
                }
            } else { proc.stdout(Stdio::inherit()); }
        } else { proc.stdout(Stdio::piped()); }

        // stderr
        if let Some(ref file) = cmd.stderr_file {
            match std::fs::File::create(file) {
                Ok(f) => { proc.stderr(f); }
                Err(e) => { eprintln!("minish: {}: {}", file, e); kill_children(&mut children); return 1; }
            }
        }

        // On Unix: set up process group and reset signals via pre_exec
        #[cfg(unix)]
        {
            let is_leader = i == 0;
            let current_pgrp = if is_leader { 0 } else { pgrp };
            // pgrp is declared before this block — need to reference it
            // Use a local variable for the closure
            let leader_pgrp_val = if i == 0 { 0 } else { 
                // pgrp might not be set yet for the first child
                0 
            };
            // Actually, for the first child we set pgrp=0 and let setpgid(0,0) handle it
            // For subsequent children, pgrp has been set
            let use_pgrp = if i == 0 { 0 } else { 
                // We need pgrp from outside — use a trick
                0 
            };
        }

        #[cfg(unix)]
        let mut pgrp: i32 = 0;

        #[cfg(unix)]
        {
            let is_leader = i == 0;
            let leader_pgrp = if is_leader { 0 } else { pgrp };
            unsafe {
                proc.pre_exec(move || {
                    libc::signal(libc::SIGINT, libc::SIG_DFL);
                    libc::signal(libc::SIGTSTP, libc::SIG_DFL);
                    libc::signal(libc::SIGQUIT, libc::SIG_DFL);
                    if is_leader {
                        libc::setpgid(0, 0);
                    } else {
                        libc::setpgid(0, leader_pgrp);
                    }
                    Ok(())
                });
            }
        }

        match proc.spawn() {
            Ok(mut child) => {
                #[cfg(unix)]
                {
                    if i == 0 {
                        pgrp = child.id() as i32;
                    }
                    unsafe { libc::setpgid(child.id() as i32, pgrp); }
                }
                if !is_last { prev_stdout = child.stdout.take(); }
                children.push(child);
            }
            Err(e) => {
                let msg = if e.kind() == io::ErrorKind::NotFound {
                    format!("minish: {}: command not found", program)
                } else {
                    format!("minish: {}: {}", program, e)
                };
                eprintln!("{}", msg);
                #[cfg(unix)] { kill_children(&mut children); }
                #[cfg(not(unix))] { for c in &mut children { let _ = c.kill(); let _ = c.wait(); } }
                return CMD_NOT_FOUND;
            }
        }
    }

    // Wait for all children
    #[cfg(unix)]
    {
        wait_children_unix(&mut children)
    }
    #[cfg(not(unix))]
    {
        wait_children_fallback(&mut children)
    }
}

/// Like `exec_pipe_chain` but with `pipefail`: returns the rightmost
/// non-zero exit code from the pipeline (or 0 if all commands succeed).
fn exec_pipe_chain_pipefail(cmds: &[Command]) -> i32 {
    let n = cmds.len();
    let mut children: Vec<Child> = Vec::with_capacity(n);
    let mut prev_stdout: Option<ChildStdout> = None;

    // Spawn (same as exec_pipe_chain)
    for (i, cmd) in cmds.iter().enumerate() {
        if cmd.args.is_empty() { continue; }
        let program = &cmd.args[0];
        let args = &cmd.args[1..];
        let mut proc = ProcCommand::new(program);
        proc.args(args);

        if i == 0 {
            if let Some(ref file) = cmd.stdin_file {
                match std::fs::File::open(file) {
                    Ok(f) => { proc.stdin(f); }
                    Err(e) => { eprintln!("minish: {}: {}", file, e); kill_children(&mut children); return 1; }
                }
            } else { proc.stdin(Stdio::inherit()); }
        } else if let Some(prev_out) = prev_stdout.take() { proc.stdin(prev_out); }

        let is_last = i == n - 1;
        if is_last {
            if let Some(ref file) = cmd.stdout_file {
                match std::fs::File::create(file) {
                    Ok(f) => { proc.stdout(f); }
                    Err(e) => { eprintln!("minish: {}: {}", file, e); kill_children(&mut children); return 1; }
                }
            } else if let Some(ref file) = cmd.stdout_append {
                match std::fs::OpenOptions::new().append(true).create(true).open(file) {
                    Ok(f) => { proc.stdout(f); }
                    Err(e) => { eprintln!("minish: {}: {}", file, e); kill_children(&mut children); return 1; }
                }
            } else { proc.stdout(Stdio::inherit()); }
        } else { proc.stdout(Stdio::piped()); }

        if let Some(ref file) = cmd.stderr_file {
            match std::fs::File::create(file) {
                Ok(f) => { proc.stderr(f); }
                Err(e) => { eprintln!("minish: {}: {}", file, e); kill_children(&mut children); return 1; }
            }
        }

        #[cfg(unix)]
        let mut pgrp: i32 = 0;
        #[cfg(unix)]
        {
            let is_leader = i == 0;
            let leader_pgrp = if is_leader { 0 } else { pgrp };
            unsafe {
                proc.pre_exec(move || {
                    libc::signal(libc::SIGINT, libc::SIG_DFL);
                    libc::signal(libc::SIGTSTP, libc::SIG_DFL);
                    libc::signal(libc::SIGQUIT, libc::SIG_DFL);
                    if is_leader { libc::setpgid(0, 0); }
                    else { libc::setpgid(0, leader_pgrp); }
                    Ok(())
                });
            }
        }

        match proc.spawn() {
            Ok(mut child) => {
                #[cfg(unix)] {
                    if i == 0 { pgrp = child.id() as i32; }
                    unsafe { libc::setpgid(child.id() as i32, pgrp); }
                }
                if !is_last { prev_stdout = child.stdout.take(); }
                children.push(child);
            }
            Err(e) => {
                let msg = if e.kind() == io::ErrorKind::NotFound {
                    format!("minish: {}: command not found", program)
                } else { format!("minish: {}: {}", program, e) };
                eprintln!("{}", msg);
                kill_children(&mut children);
                return CMD_NOT_FOUND;
            }
        }
    }

    // Wait with pipefail: track rightmost non-zero exit code
    wait_children_pipefail(&mut children)
}

/// Wait for children tracking the rightmost non-zero exit code (pipefail).
#[cfg(unix)]
fn wait_children_pipefail(children: &mut Vec<Child>) -> i32 {
    let mut last_exit = 0;
    let mut rightmost_nonzero = 0;
    for child in children.iter() {
        let pid = child.id() as i32;
        loop {
            let mut status: i32 = 0;
            let ret = unsafe { libc::waitpid(pid, &mut status, libc::WUNTRACED) };
            if ret == -1 { break; }
            if unsafe { libc::WIFEXITED(status) } {
                let code = unsafe { libc::WEXITSTATUS(status) } as i32;
                last_exit = code;
                if code != 0 { rightmost_nonzero = code; }
                break;
            } else if unsafe { libc::WIFSIGNALED(status) } {
                let sig = unsafe { libc::WTERMSIG(status) };
                let code = 128 + sig as i32;
                last_exit = code;
                if code != 0 { rightmost_nonzero = code; }
                break;
            } else if unsafe { libc::WIFSTOPPED(status) } {
                println!("\n[?]+  Stopped");
                last_exit = 148;
                if 148 != 0 { rightmost_nonzero = 148; }
                break;
            }
        }
    }
    // pipefail: return rightmost non-zero, or 0 if all succeeded
    if rightmost_nonzero != 0 { rightmost_nonzero } else { last_exit }
}

/// Wait for children tracking rightmost non-zero (non-Unix pipefail).
#[cfg(not(unix))]
fn wait_children_pipefail(children: &mut Vec<Child>) -> i32 {
    let mut last_exit = 0;
    let mut rightmost_nonzero = 0;
    for child in children.iter_mut() {
        match child.wait() {
            Ok(status) => {
                let code = status.code().unwrap_or(1);
                last_exit = code;
                if code != 0 { rightmost_nonzero = code; }
            }
            Err(_) => {
                last_exit = 1;
                rightmost_nonzero = 1;
            }
        }
    }
    if rightmost_nonzero != 0 { rightmost_nonzero } else { last_exit }
}

/// Wait for children with WUNTRACED (Unix — supports Ctrl+Z).
#[cfg(unix)]
fn wait_children_unix(children: &mut Vec<Child>) -> i32 {
    let mut exit_code = 0;
    for child in children.iter() {
        let pid = child.id() as i32;
        loop {
            let mut status: i32 = 0;
            let ret = unsafe { libc::waitpid(pid, &mut status, libc::WUNTRACED) };
            if ret == -1 { break; }

            if unsafe { libc::WIFEXITED(status) } {
                exit_code = unsafe { libc::WEXITSTATUS(status) } as i32;
                break;
            } else if unsafe { libc::WIFSIGNALED(status) } {
                let _sig = unsafe { libc::WTERMSIG(status) };
                exit_code = 128 + _sig as i32;
                break;
            } else if unsafe { libc::WIFSTOPPED(status) } {
                println!("\n[?]+  Stopped");
                exit_code = 148; // 128 + SIGTSTP (20)
                break;
            }
        }
    }
    exit_code
}

/// Wait for children via Child::wait() (cross-platform fallback).
#[cfg(not(unix))]
fn wait_children_fallback(children: &mut Vec<Child>) -> i32 {
    let mut exit_code = 0;
    for child in children.iter_mut() {
        match child.wait() {
            Ok(status) => { exit_code = status.code().unwrap_or(1); }
            Err(_) => { exit_code = 1; }
        }
    }
    exit_code
}

/// Kill all spawned children (used on error to avoid orphaned processes).
fn kill_children(children: &mut Vec<Child>) {
    for child in children.iter_mut() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// Run a background job (legacy, no job control).
#[allow(dead_code)]
pub fn run_background(cmd: &Command) -> i32 {
    let program = &cmd.args[0];
    let args = &cmd.args[1..];
    match ProcCommand::new(program).args(args).spawn() {
        Ok(child) => { println!("[{}] {}", child.id(), program); 0 }
        Err(e) => { eprintln!("minish: {}: {}", program, e); 1 }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::JobManager;

    fn opts() -> ShellOptions { ShellOptions::new() }

    #[test]
    fn test_run_empty_command() {
        let cmd = Command {
            args: vec![], stdin_file: None, stdout_file: None,
            stdout_append: None, stderr_file: None, background: false,
        };
        assert_eq!(run_external(&cmd, 0), 0);
    }

    #[test]
    fn test_run_nonexistent_command() {
        let cmd = Command {
            args: vec!["nonexistent_cmd_xyz_999".to_string()],
            stdin_file: None, stdout_file: None, stdout_append: None, stderr_file: None,
            background: false,
        };
        assert_eq!(run_external(&cmd, 0), CMD_NOT_FOUND);
    }

    #[test]
    fn test_run_pipeline_empty() {
        let pipeline = Pipeline { commands: vec![], conditional: None };
        let mut jobs = JobManager::new();
        let mut o = opts();
        assert_eq!(run_pipeline(&pipeline, 0, &mut jobs, &mut o), 0);
    }

    #[test]
    fn test_run_pipeline_single_builtin() {
        let cmd = Command {
            args: vec!["echo".to_string(), "hello".to_string()],
            stdin_file: None, stdout_file: None, stdout_append: None, stderr_file: None,
            background: false,
        };
        let pipeline = Pipeline { commands: vec![cmd], conditional: None };
        let mut jobs = JobManager::new();
        let mut o = opts();
        assert_eq!(run_pipeline(&pipeline, 0, &mut jobs, &mut o), 0);
    }

    #[test]
    fn test_run_pipeline_background() {
        let cmd = Command {
            args: vec!["sleep".to_string(), "1".to_string()],
            stdin_file: None, stdout_file: None, stdout_append: None, stderr_file: None,
            background: true,
        };
        let pipeline = Pipeline { commands: vec![cmd], conditional: None };
        let mut jobs = JobManager::new();
        let mut o = opts();
        let code = run_pipeline(&pipeline, 0, &mut jobs, &mut o);
        assert_eq!(code, 0);
        assert!(!jobs.list().is_empty());
        assert!(jobs.list()[0].command.contains("sleep"));
    }

    #[test]
    fn test_pipe_chain_two_commands() {
        let cmd1 = Command {
            args: vec!["echo".to_string(), "hello".to_string()],
            stdin_file: None, stdout_file: None, stdout_append: None, stderr_file: None,
            background: false,
        };
        let cmd2 = Command {
            args: vec!["cat".to_string()],
            stdin_file: None, stdout_file: None, stdout_append: None, stderr_file: None,
            background: false,
        };
        assert_eq!(exec_pipe_chain(&[cmd1, cmd2]), 0);
    }

    #[test]
    fn test_pipe_chain_three_commands() {
        let cmd1 = Command {
            args: vec!["echo".to_string(), "hello".to_string()],
            stdin_file: None, stdout_file: None, stdout_append: None, stderr_file: None,
            background: false,
        };
        let cmd2 = Command {
            args: vec!["tr".to_string(), "a-z".to_string(), "A-Z".to_string()],
            stdin_file: None, stdout_file: None, stdout_append: None, stderr_file: None,
            background: false,
        };
        let cmd3 = Command {
            args: vec!["cat".to_string()],
            stdin_file: None, stdout_file: None, stdout_append: None, stderr_file: None,
            background: false,
        };
        assert_eq!(exec_pipe_chain(&[cmd1, cmd2, cmd3]), 0);
    }

    #[test]
    fn test_pipe_chain_first_cmd_not_found() {
        let cmd1 = Command {
            args: vec!["_nonexistent_xyz_pipe_test_".to_string()],
            stdin_file: None, stdout_file: None, stdout_append: None, stderr_file: None,
            background: false,
        };
        let cmd2 = Command {
            args: vec!["cat".to_string()],
            stdin_file: None, stdout_file: None, stdout_append: None, stderr_file: None,
            background: false,
        };
        assert_eq!(exec_pipe_chain(&[cmd1, cmd2]), CMD_NOT_FOUND);
    }

    #[test]
    fn test_pipe_chain_empty_args_command_skipped() {
        let cmd1 = Command {
            args: vec!["echo".to_string(), "skip".to_string()],
            stdin_file: None, stdout_file: None, stdout_append: None, stderr_file: None,
            background: false,
        };
        let cmd2 = Command { args: vec![], stdin_file: None, stdout_file: None,
            stdout_append: None, stderr_file: None, background: false };
        let cmd3 = Command {
            args: vec!["cat".to_string()],
            stdin_file: None, stdout_file: None, stdout_append: None, stderr_file: None,
            background: false,
        };
        assert_eq!(exec_pipe_chain(&[cmd1, cmd2, cmd3]), 0);
    }

    #[test]
    fn test_sequential_and_both_succeed() {
        let mut jobs = JobManager::new();
        let mut o = opts();
        let cmd_true = Command { args: vec!["true".to_string()],
            stdin_file: None, stdout_file: None, stdout_append: None, stderr_file: None, background: false };
        let cmd_true2 = Command { args: vec!["true".to_string()],
            stdin_file: None, stdout_file: None, stdout_append: None, stderr_file: None, background: false };
        assert_eq!(exec_sequential(&[cmd_true, cmd_true2], Conditional::And, 0, &mut jobs, &mut o), 0);
    }

    #[test]
    fn test_sequential_and_short_circuit() {
        let mut jobs = JobManager::new();
        let mut o = opts();
        let cmd_false = Command { args: vec!["false".to_string()],
            stdin_file: None, stdout_file: None, stdout_append: None, stderr_file: None, background: false };
        let cmd_true = Command { args: vec!["true".to_string()],
            stdin_file: None, stdout_file: None, stdout_append: None, stderr_file: None, background: false };
        assert_eq!(exec_sequential(&[cmd_false, cmd_true], Conditional::And, 0, &mut jobs, &mut o), 1);
    }

    #[test]
    fn test_sequential_or_short_circuit() {
        let mut jobs = JobManager::new();
        let mut o = opts();
        let cmd_true = Command { args: vec!["true".to_string()],
            stdin_file: None, stdout_file: None, stdout_append: None, stderr_file: None, background: false };
        let cmd_false = Command { args: vec!["false".to_string()],
            stdin_file: None, stdout_file: None, stdout_append: None, stderr_file: None, background: false };
        assert_eq!(exec_sequential(&[cmd_true, cmd_false], Conditional::Or, 0, &mut jobs, &mut o), 0);
    }

    #[test]
    fn test_run_background_job_integration() {
        let mut jobs = JobManager::new();
        let cmd = Command {
            args: vec!["sleep".to_string(), "1".to_string()],
            stdin_file: None, stdout_file: None, stdout_append: None, stderr_file: None,
            background: true,
        };
        let code = run_background_job(&cmd, &mut jobs);
        assert_eq!(code, 0);
        assert_eq!(jobs.list().len(), 1);
    }

    #[test]
    fn test_run_pipeline_no_background() {
        let mut jobs = JobManager::new();
        let mut o = opts();
        let cmd = Command {
            args: vec!["echo".to_string(), "fg".to_string()],
            stdin_file: None, stdout_file: None, stdout_append: None, stderr_file: None,
            background: false,
        };
        let pipeline = Pipeline { commands: vec![cmd], conditional: None };
        assert_eq!(run_pipeline(&pipeline, 0, &mut jobs, &mut o), 0);
        assert!(jobs.list().is_empty());
    }

    #[test]
    fn test_run_background_job_empty_args() {
        let mut jobs = JobManager::new();
        let cmd = Command { args: vec![], stdin_file: None, stdout_file: None,
            stdout_append: None, stderr_file: None, background: true };
        assert_eq!(run_background_job(&cmd, &mut jobs), 0);
    }
}
