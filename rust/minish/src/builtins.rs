//! # Built-in Commands
//!
//! In-process shell builtins that run without forking.
//! Each function returns `Some(exit_code)` if it handled the command,
//! or `None` if the command should be treated as external.

use std::io::Write;
use std::path::Path;

/// Try to execute a built-in command.
/// Returns `Some(exit_code)` if the command was a builtin,
/// `None` if it should be passed to an external executor.
pub fn try_builtin(command: &str, args: &[String]) -> Option<i32> {
    match command {
        "cd" => Some(cd(args)),
        "pwd" => Some(pwd(args)),
        "echo" => Some(echo(args)),
        "ls" => Some(ls(args)),
        "cat" => Some(cat(args)),
        "rm" => Some(rm(args)),
        "mv" => Some(mv(args)),
        "cp" => Some(cp(args)),
        "mkdir" => Some(mkdir(args)),
        "ps" => Some(ps(args)),
        "kill" => Some(kill(args)),
        "help" => Some(help(args)),
        "export" => Some(export(args)),
        "source" => Some(source(args)),
        "true" | ":" => Some(0),
        "false" => Some(1),
        _ => None,
    }
}

/// Change directory.
fn cd(args: &[String]) -> i32 {
    let target = if args.is_empty() {
        // Default to HOME or /
        std::env::var("HOME").unwrap_or_else(|_| "/".to_string())
    } else {
        args[0].clone()
    };

    match std::env::set_current_dir(&target) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("cd: {}: {}", target, e);
            1
        }
    }
}

/// Print working directory.
fn pwd(_args: &[String]) -> i32 {
    match std::env::current_dir() {
        Ok(path) => {
            println!("{}", path.display());
            0
        }
        Err(e) => {
            eprintln!("pwd: {}", e);
            1
        }
    }
}

/// Echo arguments to stdout.
fn echo(args: &[String]) -> i32 {
    let mut no_newline = false;
    let mut start = 0;

    if !args.is_empty() && args[0] == "-n" {
        no_newline = true;
        start = 1;
    }

    let output = args[start..].join(" ");
    if no_newline {
        print!("{}", output);
        std::io::stdout().flush().ok();
    } else {
        println!("{}", output);
    }
    0
}

/// List directory contents.
fn ls(args: &[String]) -> i32 {
    let path = if args.is_empty() || args[0].starts_with('-') {
        "."
    } else {
        &args[0]
    };

    match std::fs::read_dir(path) {
        Ok(entries) => {
            let mut files: Vec<_> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect();
            files.sort();

            for f in &files {
                let path = Path::new(&path).join(f);
                let is_dir = path.is_dir();
                if is_dir {
                    println!("{}/", f);
                } else {
                    println!("{}", f);
                }
            }
            0
        }
        Err(e) => {
            eprintln!("ls: {}: {}", path, e);
            1
        }
    }
}

/// Concatenate and print files.
fn cat(args: &[String]) -> i32 {
    if args.is_empty() {
        // Read from stdin
        let stdin = std::io::stdin();
        for line in stdin.lines() {
            match line {
                Ok(l) => println!("{}", l),
                Err(_) => break,
            }
        }
        return 0;
    }

    for path in args {
        match std::fs::read_to_string(path) {
            Ok(content) => print!("{}", content),
            Err(e) => {
                eprintln!("cat: {}: {}", path, e);
                return 1;
            }
        }
    }
    0
}

/// Remove files or directories.
fn rm(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("rm: missing operand");
        return 1;
    }

    let mut recursive = false;
    let mut force = false;
    let mut paths: Vec<&str> = Vec::new();

    for arg in args {
        match arg.as_str() {
            "-r" | "-rf" | "-fr" => recursive = true,
            "-f" => force = true,
            _ => paths.push(arg),
        }
    }

    let mut exit_code = 0;
    for path in paths {
        let p = Path::new(path);
        if !p.exists() {
            if !force {
                eprintln!("rm: {}: No such file or directory", path);
                exit_code = 1;
            }
            continue;
        }

        if p.is_dir() {
            if recursive {
                if let Err(e) = std::fs::remove_dir_all(path) {
                    eprintln!("rm: {}: {}", path, e);
                    exit_code = 1;
                }
            } else {
                eprintln!("rm: {}: Is a directory", path);
                exit_code = 1;
            }
        } else {
            if let Err(e) = std::fs::remove_file(path) {
                eprintln!("rm: {}: {}", path, e);
                exit_code = 1;
            }
        }
    }

    exit_code
}

/// Move/rename files.
fn mv(args: &[String]) -> i32 {
    if args.len() < 2 {
        eprintln!("mv: missing operand");
        return 1;
    }

    let src = &args[0];
    let dst = &args[1];

    match std::fs::rename(src, dst) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("mv: {} -> {}: {}", src, dst, e);
            1
        }
    }
}

/// Copy files.
fn cp(args: &[String]) -> i32 {
    if args.len() < 2 {
        eprintln!("cp: missing operand");
        return 1;
    }

    let src = &args[0];
    let dst = &args[1];
    let src_path = Path::new(src);

    if !src_path.exists() {
        eprintln!("cp: {}: No such file or directory", src);
        return 1;
    }

    if src_path.is_dir() {
        // Simple recursive copy
        match cp_recursive(src_path, Path::new(dst)) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("cp: {} -> {}: {}", src, dst, e);
                1
            }
        }
    } else {
        match std::fs::copy(src, dst) {
            Ok(_) => 0,
            Err(e) => {
                eprintln!("cp: {} -> {}: {}", src, dst, e);
                1
            }
        }
    }
}

/// Recursive directory copy.
fn cp_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if src.is_dir() {
        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let entry_type = entry.file_type()?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());

            if entry_type.is_dir() {
                cp_recursive(&src_path, &dst_path)?;
            } else {
                std::fs::copy(&src_path, &dst_path)?;
            }
        }
        Ok(())
    } else {
        std::fs::copy(src, dst)?;
        Ok(())
    }
}

/// Create directories.
fn mkdir(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("mkdir: missing operand");
        return 1;
    }

    let mut exit_code = 0;
    for path in args {
        if let Err(e) = std::fs::create_dir_all(path) {
            eprintln!("mkdir: {}: {}", path, e);
            exit_code = 1;
        }
    }
    exit_code
}

/// List processes (reads /proc on Linux/MINIX, or shows stub).
fn ps(_args: &[String]) -> i32 {
    // On MINIX, this would read /proc/*/psinfo
    // For now, show a stub process list
    println!("PID   COMMAND");
    println!("{}", "─".repeat(40));
    println!("1     init");
    println!("100   vfs");
    println!("101   pm");
    println!("102   rs");
    println!("103   vm");
    println!("104   ds");
    println!("105   sched");
    println!("200   minish   (current shell)");
    println!("{}    minish   (this process)", std::process::id());
    0
}

/// Send a signal to a process.
fn kill(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("kill: usage: kill [-signal] <pid>");
        return 1;
    }

    let mut signal = libc::SIGTERM;
    let mut pid_start = 0;

    if args[0].starts_with('-') {
        // Parse signal number
        let sig_str = &args[0][1..];
        signal = match sig_str.parse::<i32>() {
            Ok(n) => n,
            Err(_) => {
                eprintln!("kill: invalid signal: {}", args[0]);
                return 1;
            }
        };
        pid_start = 1;
    }

    if pid_start >= args.len() {
        eprintln!("kill: missing pid");
        return 1;
    }

    let mut exit_code = 0;
    for arg in &args[pid_start..] {
        let pid: i32 = match arg.parse() {
            Ok(n) => n,
            Err(_) => {
                eprintln!("kill: invalid pid: {}", arg);
                exit_code = 1;
                continue;
            }
        };

        #[cfg(unix)]
        {
            let result = unsafe { libc::kill(pid, signal) };
            if result != 0 {
                let err = std::io::Error::last_os_error();
                eprintln!("kill: {}: {}", pid, err);
                exit_code = 1;
            }
        }
        #[cfg(not(unix))]
        {
            let _ = (pid, signal);
            eprintln!("kill: not supported on this platform");
            exit_code = 1;
        }
    }

    exit_code
}

/// Show help message.
fn help(_args: &[String]) -> i32 {
    println!("minish — Minimal GergiOS Shell");
    println!();
    println!("Built-in commands:");
    println!("  cd [dir]       Change directory");
    println!("  pwd            Print working directory");
    println!("  echo [-n] ...   Print text");
    println!("  ls [path]      List directory");
    println!("  cat [files]    Print file contents");
    println!("  rm [-r] files  Remove files");
    println!("  mv src dst     Move/rename");
    println!("  cp src dst     Copy");
    println!("  mkdir dirs     Create directories");
    println!("  ps             List processes");
    println!("  kill [-sig] pid Send signal");
    println!("  export VAR=val Set environment variable");
    println!("  source file    Execute script");
    println!("  help           Show this help");
    println!("  exit           Exit shell");
    println!();
    println!("Syntax:");
    println!("  cmd | cmd      Pipeline");
    println!("  cmd > file     Redirect stdout");
    println!("  cmd >> file    Append stdout");
    println!("  cmd < file     Redirect stdin");
    println!("  cmd &          Background");
    println!("  cmd && cmd     Conditional AND");
    println!("  cmd || cmd     Conditional OR");
    0
}

/// Set environment variables.
fn export(args: &[String]) -> i32 {
    if args.is_empty() {
        // Print all environment variables
        for (key, value) in std::env::vars() {
            println!("{}={}", key, value);
        }
        return 0;
    }

    for arg in args {
        if let Some(eq_pos) = arg.find('=') {
            let key = &arg[..eq_pos];
            let value = &arg[eq_pos + 1..];
            std::env::set_var(key, value);
        } else {
            // Just print the variable value
            match std::env::var(arg) {
                Ok(val) => println!("{}={}", arg, val),
                Err(_) => {} // variable not set
            }
        }
    }
    0
}

/// Source (execute) a script file.
fn source(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("source: missing filename");
        return 1;
    }

    let path = &args[0];
    match std::fs::read_to_string(path) {
        Ok(contents) => {
            for line in contents.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                // Note: in a real shell, this would call the main executor
                // For now, print a message
                println!("source: executing: {}", trimmed);
            }
            0
        }
        Err(e) => {
            eprintln!("source: {}: {}", path, e);
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
    fn test_try_builtin_known() {
        assert!(try_builtin("cd", &[]).is_some());
        assert!(try_builtin("ls", &[]).is_some());
        assert!(try_builtin("echo", &["hello".to_string()]).is_some());
        assert!(try_builtin("help", &[]).is_some());
        assert!(try_builtin("true", &[]).is_some());
        assert!(try_builtin(":", &[]).is_some());
        assert!(try_builtin("false", &[]).is_some());
    }

    #[test]
    fn test_try_builtin_unknown() {
        assert!(try_builtin("nonexistent_cmd_12345", &[]).is_none());
    }

    #[test]
    fn test_echo_default() {
        assert_eq!(echo(&["hello".to_string(), "world".to_string()]), 0);
        assert_eq!(echo(&[]), 0);
    }

    #[test]
    fn test_echo_no_newline() {
        assert_eq!(echo(&["-n".to_string(), "hello".to_string()]), 0);
    }

    #[test]
    fn test_pwd_default() {
        assert_eq!(pwd(&[]), 0);
    }

    #[test]
    fn test_cd_no_args() {
        // cd without args should use HOME
        assert_eq!(cd(&[]), 0);
    }

    #[test]
    fn test_cd_nonexistent() {
        assert_eq!(cd(&["/nonexistent_path_xyz_123".to_string()]), 1);
    }

    #[test]
    fn test_cd_to_root() {
        assert_eq!(cd(&["/".to_string()]), 0);
    }

    #[test]
    fn test_mkdir_no_args() {
        assert_eq!(mkdir(&[]), 1);
    }

    #[test]
    fn test_rm_no_args() {
        assert_eq!(rm(&[]), 1);
    }

    #[test]
    fn test_mv_no_args() {
        assert_eq!(mv(&[]), 1);
    }

    #[test]
    fn test_mv_one_arg() {
        assert_eq!(mv(&["src".to_string()]), 1);
    }

    #[test]
    fn test_cp_no_args() {
        assert_eq!(cp(&[]), 1);
    }

    #[test]
    fn test_cp_nonexistent_src() {
        // Source doesn't exist
        let result = cp(&["/nonexistent_xyz".to_string(), "/tmp/dst".to_string()]);
        assert_eq!(result, 1);
    }

    #[test]
    fn test_help_doesnt_panic() {
        assert_eq!(help(&[]), 0);
    }

    #[test]
    fn test_export_empty() {
        assert_eq!(export(&[]), 0);
    }

    #[test]
    fn test_export_set() {
        assert_eq!(export(&["TEST_VARIABLE_SET=hello".to_string()]), 0);
    }

    #[test]
    fn test_source_no_args() {
        assert_eq!(source(&[]), 1);
    }

    #[test]
    fn test_source_nonexistent() {
        assert_eq!(source(&["/nonexistent_xyz_file".to_string()]), 1);
    }

    #[test]
    fn test_ps_doesnt_panic() {
        assert_eq!(ps(&[]), 0);
    }

    #[test]
    fn test_kill_no_args() {
        assert_eq!(kill(&[]), 1);
    }

    #[test]
    fn test_kill_invalid_pid() {
        assert_eq!(kill(&["-9".to_string(), "not_a_number".to_string()]), 1);
    }

    #[test]
    fn test_kill_invalid_signal() {
        assert_eq!(kill(&["-abc".to_string(), "1".to_string()]), 1);
    }

    #[test]
    fn test_ls_nonexistent() {
        assert_eq!(ls(&["/nonexistent_ls_test".to_string()]), 1);
    }

    #[test]
    fn test_ls_root() {
        assert_eq!(ls(&["/".to_string()]), 0);
    }

    #[test]
    fn test_ls_flag_only() {
        assert_eq!(ls(&["-l".to_string()]), 0);
    }

    #[test]
    fn test_ls_default() {
        assert_eq!(ls(&[]), 0);
    }

    #[test]
    fn test_cat_nonexistent() {
        assert_eq!(cat(&["/nonexistent_test_cat_file".to_string()]), 1);
    }
}
