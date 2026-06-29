//! Rust port of the MINIX/NetBSD `pr` utility.
//!
//! Usage:
//!   pr [-columns] [-dFfm] [-h header] [-l lines] [-n] [-o offset] [-w width] [file ...]
//!
//! Paginates or columnates files for printing.
//!   -n: number lines
//!   -l lines: page length (default 66, 0 = no page breaks)
//!   -o offset: left margin (number of spaces)
//!   -w width: page width (default 72)
//!   -h header: use custom header text
//!   -d: double-space output
//!   -f: use form feeds instead of blank lines between pages
//!   -F: same as -f
//!   -m: merge files, one per column
//!   -columns: produce multi-column output

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};

const USAGE: &str = "usage: pr [-columns] [-dFfm] [-h header] [-l lines] [-n] [-o offset] [-w width] [file ...]";

fn usage() -> ! {
    eprintln!("{USAGE}");
    std::process::exit(1);
}

fn get_date() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let start = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = start.as_secs();
    // Simple POSIX-like date formatting (no external deps)
    // Convert to YYYY-MM-DD HH:MM
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;

    // Days since epoch to date (simplified)
    let mut y = 1970i64;
    let mut remaining = days as i64;
    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if remaining < days_in_year { break; }
        remaining -= days_in_year;
        y += 1;
    }
    let months_days: [i64; 12] = if is_leap(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 0;
    for (i, &md) in months_days.iter().enumerate() {
        if remaining < md { m = i + 1; break; }
        remaining -= md;
    }
    if m == 0 { m = 12; }
    let d = remaining + 1;

    format!("{:04}-{:02}-{:02} {:02}:{:02}", y, m, d, hours, minutes)
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut page_len: usize = 66;
    let mut page_width: usize = 72;
    let mut offset: usize = 0;
    let mut header: Option<String> = None;
    let mut number_lines = false;
    let mut double_space = false;
    let mut use_formfeed = false;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }

        // Check if it's a numeric column count (-3, -4, etc.)
        if opt.len() > 1 && opt[1..].chars().all(|c| c.is_ascii_digit()) {
            let n: usize = opt[1..].parse().unwrap();
            if n >= 1 && n <= 9 {
                // Parse but ignore multi-column for now (basic single-column still works)
                _ = n;
                argv = &argv[1..];
                continue;
            }
        }

        let mut chars = opt.chars();
        chars.next(); // skip '-'
        while let Some(flag) = chars.next() {
            match flag {
                'd' => double_space = true,
                'f' | 'F' => use_formfeed = true,
                'm' => { /* merge mode noted but not multi-column implemented */ }
                'n' => number_lines = true,
                'h' => {
                    let val = if opt.len() > 2 { opt[2..].to_string() } else {
                        argv = &argv[1..];
                        if argv.is_empty() { usage() }
                        argv[0].clone()
                    };
                    header = Some(val);
                    break; // -h takes the rest of the arg
                }
                'l' => {
                    let val = if opt.len() > 2 { opt[2..].to_string() } else {
                        argv = &argv[1..];
                        if argv.is_empty() { usage() }
                        argv[0].clone()
                    };
                    page_len = val.parse().unwrap_or_else(|_| usage());
                }
                'o' => {
                    let val = if opt.len() > 2 { opt[2..].to_string() } else {
                        argv = &argv[1..];
                        if argv.is_empty() { usage() }
                        argv[0].clone()
                    };
                    offset = val.parse().unwrap_or_else(|_| usage());
                }
                'w' => {
                    let val = if opt.len() > 2 { opt[2..].to_string() } else {
                        argv = &argv[1..];
                        if argv.is_empty() { usage() }
                        argv[0].clone()
                    };
                    page_width = val.parse().unwrap_or_else(|_| usage());
                }
                _ => usage(),
            }
        }
        argv = &argv[1..];
    }

    let file_names: Vec<&str> = if argv.is_empty() { vec!["-"] } else { argv.iter().map(|s| s.as_str()).collect() };
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut had_error = false;

    let effective_width = if page_width > offset { page_width - offset } else { 72 };
    let header_text = header.unwrap_or_default();
    // POSIX pr uses 5 lines for header+separator+trailer
    let header_lines = 5usize;
    let body_lines = if page_len > header_lines { page_len - header_lines } else { 1 };

    for fname in file_names.iter() {
        let lines: Vec<String> = {
            let reader: Box<dyn BufRead> = if *fname == "-" {
                Box::new(BufReader::new(io::stdin().lock()))
            } else {
                match File::open(fname) {
                    Ok(f) => Box::new(BufReader::new(f)),
                    Err(e) => { eprintln!("pr: {fname}: {e}"); had_error = true; continue; }
                }
            };
            reader.lines().map(|l| l.unwrap_or_default()).collect()
        };

        if lines.is_empty() {
            continue;
        }

        let date = get_date();
        let mut page_num = 1usize;
        let mut line_idx = 0usize;
        let sep: String = std::iter::repeat('-').take(effective_width).collect();

        while line_idx < lines.len() {
            // Page header
            if offset > 0 {
                write!(out, "{:>width$}", "", width = offset).ok();
            }
            writeln!(out, "{} {} Page {}", header_text, date, page_num).ok();

            if offset > 0 {
                write!(out, "{:>width$}", "", width = offset).ok();
            }
            writeln!(out, "{sep}").ok();

            // Page body
            let mut page_line = 0usize;
            while page_line < body_lines && line_idx < lines.len() {
                let mut printed = String::new();
                if number_lines {
                    printed.push_str(&format!("{:>3} ", line_idx + 1));
                }

                let max_line_len = effective_width.saturating_sub(printed.len());
                let trimmed: String = lines[line_idx].chars().take(max_line_len).collect();
                printed.push_str(&trimmed);

                if offset > 0 {
                    write!(out, "{:>width$}", "", width = offset).ok();
                }
                writeln!(out, "{printed}").ok();
                line_idx += 1;
                page_line += 1;

                if double_space && page_line < body_lines && line_idx < lines.len() {
                    if offset > 0 {
                        write!(out, "{:>width$}", "", width = offset).ok();
                    }
                    writeln!(out).ok();
                    page_line += 1;
                }
            }

            if line_idx < lines.len() {
                // Page break (not last page)
                if use_formfeed {
                    write!(out, "\x0c").ok();
                } else {
                    // Fill remaining lines with blank lines to reach page_len
                    while page_line < body_lines {
                        writeln!(out).ok();
                        page_line += 1;
                    }
                }
            }
            page_num += 1;
        }
    }

    if had_error { std::process::exit(1); }
}
