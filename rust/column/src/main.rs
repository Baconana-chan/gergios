//! Rust port of the MINIX/NetBSD `column` utility.
//!
//! Usage:
//!   column [-tx] [-c columns] [-s sep] [file ...]
//!
//! Columnates lists. -t: create table, -x: fill rows before columns.
//! -c: column width, -s: separator (default space/tab).

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};

const USAGE: &str = "usage: column [-tx] [-c columns] [-s sep] [file ...]";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut table_mode = false;
    let mut fill_rows = false;
    let mut col_width: Option<usize> = None;
    let mut separator: Vec<u8> = vec![b' ', b'\t'];

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        if opt.starts_with("-c") {
            let val = if opt.len() > 2 { opt[2..].to_string() } else {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("{USAGE}"); std::process::exit(1); }
                argv[0].clone()
            };
            col_width = Some(val.parse().unwrap_or_else(|_| { eprintln!("{USAGE}"); std::process::exit(1); }));
        } else if opt.starts_with("-s") {
            let val = if opt.len() > 2 { opt[2..].to_string() } else {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("{USAGE}"); std::process::exit(1); }
                argv[0].clone()
            };
            separator = val.bytes().collect();
        } else {
            for ch in opt.chars().skip(1) {
                match ch {
                    't' => table_mode = true,
                    'x' => fill_rows = true,
                    _ => { eprintln!("{USAGE}"); std::process::exit(1); }
                }
            }
        }
        argv = &argv[1..];
    }

    let files: Vec<&str> = if argv.is_empty() { vec!["-"] } else { argv.iter().map(|s| s.as_str()).collect() };
    let stdin = io::stdin();

    let mut all_items: Vec<Vec<String>> = Vec::new();
    let mut all_rows: Vec<Vec<String>> = Vec::new();

    for fname in &files {
        let reader: Box<dyn BufRead> = if *fname == "-" {
            Box::new(BufReader::new(stdin.lock()))
        } else {
            match File::open(fname) {
                Ok(f) => Box::new(BufReader::new(f)),
                Err(e) => { eprintln!("column: {fname}: {e}"); continue; }
            }
        };

        for line_res in reader.lines() {
            let line = match line_res { Ok(l) => l, Err(_) => break };
            let seps: Vec<char> = separator.iter().map(|&b| b as char).collect();
            let fields: Vec<String> = if table_mode {
                line.split(&seps[..]).filter(|s| !s.is_empty()).map(|s| s.to_string()).collect()
            } else {
                vec![line]
            };
            if table_mode {
                all_rows.push(fields);
            } else {
                for f in fields {
                    for item in f.split_whitespace() {
                        all_items.push(vec![item.to_string()]);
                    }
                }
            }
        }
    }

    let stdout = io::stdout();
    let mut out = stdout.lock();

    if table_mode {
        // Compute column widths
        if all_rows.is_empty() { return; }
        let max_cols = all_rows.iter().map(|r| r.len()).max().unwrap_or(1);
        let mut col_widths = vec![0usize; max_cols];
        for row in &all_rows {
            for (i, field) in row.iter().enumerate() {
                if i < col_widths.len() {
                    col_widths[i] = col_widths[i].max(field.len());
                }
            }
        }
        for row in &all_rows {
            for (i, field) in row.iter().enumerate() {
                write!(out, "{field}").ok();
                if i + 1 < row.len() {
                    let pad = if i < col_widths.len() { col_widths[i] - field.len() + 2 } else { 2 };
                    for _ in 0..pad { write!(out, " ").ok(); }
                }
            }
            writeln!(out).ok();
        }
    } else {
        // Non-table mode: arrange in columns
        if all_items.is_empty() { return; }
        let term_width = col_width.unwrap_or(80);
        let max_item_len = all_items.iter().map(|v| v[0].len()).max().unwrap_or(1);
        let col_w = max_item_len + 2;
        let num_cols = std::cmp::max(1, if col_w > 0 { term_width / col_w } else { 1 });
        let num_rows = (all_items.len() + num_cols - 1) / num_cols;

        if fill_rows {
            // Fill rows first (across)
            for r in 0..num_rows {
                for c in 0..num_cols {
                    let idx = r * num_cols + c;
                    if idx < all_items.len() {
                        write!(out, "{:<width$}", all_items[idx][0], width = col_w).ok();
                    }
                }
                writeln!(out).ok();
            }
        } else {
            // Fill columns first (down)
            for r in 0..num_rows {
                for c in 0..num_cols {
                    let idx = c * num_rows + r;
                    if idx < all_items.len() {
                        write!(out, "{:<width$}", all_items[idx][0], width = col_w).ok();
                    }
                }
                writeln!(out).ok();
            }
        }
    }
}
