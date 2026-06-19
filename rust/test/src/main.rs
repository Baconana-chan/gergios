use std::env;
use std::fs;
use std::io;
use std::path::Path;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut tokens: Vec<String> = Vec::new();
    for arg in &args[1..] {
        tokens.push(arg.clone());
    }

    if tokens.is_empty() {
        process::exit(1);
    }

    // Remove trailing "]" if this is a [...] invocation
    if tokens.last().map(|s| s.as_str()) == Some("]") {
        tokens.pop();
    }

    if tokens.is_empty() {
        process::exit(1);
    }

    let expr: Vec<&str> = tokens.iter().map(|s| s.as_str()).collect();
    let result = eval_or(&expr, 0).0;

    if result {
        process::exit(0);
    } else {
        process::exit(1);
    }
}

/// Evaluate `-o` (OR) expressions.
fn eval_or(expr: &[&str], start: usize) -> (bool, usize) {
    let (left, mut pos) = eval_and(expr, start);
    if pos >= expr.len() {
        return (left, pos);
    }
    if expr[pos] == "-o" {
        let (right, next) = eval_or(expr, pos + 1);
        (left || right, next)
    } else {
        (left, pos)
    }
}

/// Evaluate `-a` (AND) expressions.
fn eval_and(expr: &[&str], start: usize) -> (bool, usize) {
    let (left, mut pos) = eval_primary(expr, start);
    if pos >= expr.len() {
        return (left, pos);
    }
    if expr[pos] == "-a" {
        let (right, next) = eval_and(expr, pos + 1);
        (left && right, next)
    } else {
        (left, pos)
    }
}

/// Recognized unary operators (take one operand after the operator).
fn is_unary_op(s: &str) -> bool {
    matches!(
        s,
        "-b" | "-c" | "-d" | "-e" | "-f" | "-g" | "-h" | "-L"
            | "-n" | "-p" | "-r" | "-S" | "-s" | "-u" | "-w" | "-x" | "-z"
    )
}

/// Recognized binary operators (take left, op, right).
fn is_binary_op(s: &str) -> bool {
    matches!(
        s,
        "=" | "!=" | "-eq" | "-ne" | "-gt" | "-ge" | "-lt" | "-le"
            | "-nt" | "-ot" | "-ef"
    )
}

/// Evaluate a primary expression: unary op, binary op, parenthesized, or string test.
fn eval_primary(expr: &[&str], start: usize) -> (bool, usize) {
    if start >= expr.len() {
        return (false, start);
    }

    // Handle !
    if expr[start] == "!" {
        let (inner, pos) = eval_primary(expr, start + 1);
        return (!inner, pos);
    }

    // Handle ( ... )
    if expr[start] == "(" {
        let (inner, pos) = eval_or(expr, start + 1);
        if pos < expr.len() && expr[pos] == ")" {
            return (inner, pos + 1);
        }
        return (false, pos);
    }

    // Unary operator: op is expr[start], operand is expr[start+1]
    if start + 1 < expr.len() && is_unary_op(expr[start]) {
        let val = eval_unary(expr[start], expr[start + 1]);
        return (val, start + 2);
    }

    // Binary operator: left is expr[start], op is expr[start+1], right is expr[start+2]
    if start + 2 < expr.len() && is_binary_op(expr[start + 1]) {
        let val = eval_binary(expr[start], expr[start + 1], expr[start + 2]);
        return (val, start + 3);
    }

    // Single operand: string test (non-empty = true)
    (!expr[start].is_empty(), start + 1)
}

/// Evaluate a unary operator.
fn eval_unary(op: &str, operand: &str) -> bool {
    match op {
        "-b" => is_block_special(operand),
        "-c" => is_char_special(operand),
        "-d" => is_directory(operand),
        "-e" => path_exists(operand),
        "-f" => is_regular_file(operand),
        "-g" => has_setgid(operand),
        "-h" | "-L" => is_symlink(operand),
        "-n" => !operand.is_empty(),
        "-p" => is_fifo(operand),
        "-r" => is_readable(operand),
        "-S" => is_socket(operand),
        "-s" => is_nonzero_size(operand),
        "-u" => has_setuid(operand),
        "-w" => is_writable(operand),
        "-x" => is_executable(operand),
        "-z" => operand.is_empty(),
        _ => false,
    }
}

/// Evaluate a binary operator.
fn eval_binary(left: &str, op: &str, right: &str) -> bool {
    match op {
        "=" => left == right,
        "!=" => left != right,
        "-eq" => {
            let l = left.parse::<i64>().unwrap_or(0);
            let r = right.parse::<i64>().unwrap_or(0);
            l == r
        }
        "-ne" => {
            let l = left.parse::<i64>().unwrap_or(0);
            let r = right.parse::<i64>().unwrap_or(0);
            l != r
        }
        "-gt" => {
            let l = left.parse::<i64>().unwrap_or(0);
            let r = right.parse::<i64>().unwrap_or(0);
            l > r
        }
        "-ge" => {
            let l = left.parse::<i64>().unwrap_or(0);
            let r = right.parse::<i64>().unwrap_or(0);
            l >= r
        }
        "-lt" => {
            let l = left.parse::<i64>().unwrap_or(0);
            let r = right.parse::<i64>().unwrap_or(0);
            l < r
        }
        "-le" => {
            let l = left.parse::<i64>().unwrap_or(0);
            let r = right.parse::<i64>().unwrap_or(0);
            l <= r
        }
        "-nt" => is_newer_than(left, right),
        "-ot" => is_older_than(left, right),
        "-ef" => same_file(left, right),
        _ => false,
    }
}

// --- File test implementations ---

fn path_exists(path: &str) -> bool {
    Path::new(path).exists()
}

fn is_regular_file(path: &str) -> bool {
    fs::metadata(path).map(|m| m.is_file()).unwrap_or(false)
}

fn is_directory(path: &str) -> bool {
    fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false)
}

fn is_symlink(path: &str) -> bool {
    fs::symlink_metadata(path).map(|m| m.file_type().is_symlink()).unwrap_or(false)
}

fn is_readable(path: &str) -> bool {
    fs::metadata(path).map(|_| true).unwrap_or(false)
}

fn is_writable(path: &str) -> bool {
    fs::metadata(path).map(|_| true).unwrap_or(false)
}

fn is_executable(path: &str) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(path).map(|m| m.permissions().mode() & 0o111 != 0).unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
}

fn is_nonzero_size(path: &str) -> bool {
    fs::metadata(path).map(|m| m.len() > 0).unwrap_or(false)
}

fn is_block_special(path: &str) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        fs::metadata(path).map(|m| m.file_type().is_block_device()).unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
}

fn is_char_special(path: &str) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        fs::metadata(path).map(|m| m.file_type().is_char_device()).unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
}

fn is_fifo(path: &str) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        fs::metadata(path).map(|m| m.file_type().is_fifo()).unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
}

fn is_socket(path: &str) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        fs::metadata(path).map(|m| m.file_type().is_socket()).unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
}

fn has_setuid(path: &str) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(path).map(|m| m.permissions().mode() & 0o4000 != 0).unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
}

fn has_setgid(path: &str) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(path).map(|m| m.permissions().mode() & 0o2000 != 0).unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
}

fn is_newer_than(a: &str, b: &str) -> bool {
    let mtime_a = fs::metadata(a).and_then(|m| m.modified().map_err(|e| io::Error::new(io::ErrorKind::Other, e)));
    let mtime_b = fs::metadata(b).and_then(|m| m.modified().map_err(|e| io::Error::new(io::ErrorKind::Other, e)));
    match (mtime_a, mtime_b) {
        (Ok(a), Ok(b)) => a > b,
        _ => false,
    }
}

fn is_older_than(a: &str, b: &str) -> bool {
    let mtime_a = fs::metadata(a).and_then(|m| m.modified().map_err(|e| io::Error::new(io::ErrorKind::Other, e)));
    let mtime_b = fs::metadata(b).and_then(|m| m.modified().map_err(|e| io::Error::new(io::ErrorKind::Other, e)));
    match (mtime_a, mtime_b) {
        (Ok(a), Ok(b)) => a < b,
        _ => false,
    }
}

fn same_file(a: &str, b: &str) -> bool {
    match (fs::metadata(a), fs::metadata(b)) {
        (Ok(ma), Ok(mb)) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                ma.dev() == mb.dev() && ma.ino() == mb.ino()
            }
            #[cfg(not(unix))]
            {
                let _ = (ma, mb);
                false
            }
        }
        _ => false,
    }
}
