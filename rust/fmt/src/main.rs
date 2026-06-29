//! Rust port of the MINIX/NetBSD `fmt` utility.
//!
//! Usage:
//!   fmt [-width] [-c] [-s] [file ...]
//!
//! Simple text formatter that fills and joins lines to fit a specified width.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};

const USAGE: &str = "usage: fmt [-width] [-c] [-s] [file ...]";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut goal_width: usize = 72; // default: Mail format
    let mut crown_margin = false;
    let mut split_only = false;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }

        // Check for -width flag
        if opt.len() > 1 && opt[1..].chars().all(|c| c.is_ascii_digit()) {
            goal_width = opt[1..].parse().unwrap_or(72);
        } else {
            for ch in opt.chars().skip(1) {
                match ch {
                    'c' => crown_margin = true,
                    's' => split_only = true,
                    _ => { eprintln!("{USAGE}"); std::process::exit(1); }
                }
            }
        }
        argv = &argv[1..];
    }

    let files: Vec<&str> = if argv.is_empty() { vec!["-"] } else { argv.iter().map(|s| s.as_str()).collect() };
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for fname in &files {
        let reader: Box<dyn BufRead> = if *fname == "-" {
            Box::new(BufReader::new(io::stdin().lock()))
        } else {
            match File::open(fname) {
                Ok(f) => Box::new(BufReader::new(f)),
                Err(e) => { eprintln!("fmt: {fname}: {e}"); continue; }
            }
        };

        let mut paragraph: Vec<String> = Vec::new();
        let mut in_paragraph = false;

        for line_res in reader.lines() {
            let line = match line_res { Ok(l) => l, Err(_) => break };
            let trimmed = line.trim();

            if trimmed.is_empty() {
                // End of paragraph
                if in_paragraph {
                    // Flush paragraph
                    flush_paragraph(&paragraph, goal_width, crown_margin, &mut out);
                    paragraph.clear();
                    in_paragraph = false;
                }
                writeln!(out).ok();
                continue;
            }

            if split_only {
                writeln!(out, "{line}").ok();
                continue;
            }

            let starts_with_space = line.starts_with(' ');
            let indented = line.starts_with("  ");

            if crown_margin {
                // Crown margin mode: first line sets indent
                if in_paragraph {
                    paragraph.push(line);
                } else {
                    paragraph.push(line);
                    in_paragraph = true;
                }
            } else if indented {
                if in_paragraph {
                    flush_paragraph(&paragraph, goal_width, crown_margin, &mut out);
                    paragraph.clear();
                }
                paragraph.push(line);
                in_paragraph = true;
            } else if starts_with_space && !in_paragraph {
                paragraph.push(line);
                in_paragraph = true;
            } else if in_paragraph {
                paragraph.push(line);
            } else {
                // Single line, flush immediately
                flush_paragraph(&[line], goal_width, crown_margin, &mut out);
            }
        }

        if in_paragraph {
            flush_paragraph(&paragraph, goal_width, crown_margin, &mut out);
        }
    }
}

fn flush_paragraph(lines: &[String], width: usize, crown_margin: bool, out: &mut impl Write) {
    if lines.is_empty() { return; }

    // Collect all words
    let mut words: Vec<String> = Vec::new();
    let mut first_indent = 0usize;

    for (idx, line) in lines.iter().enumerate() {
        if idx == 0 {
            first_indent = line.len() - line.trim_start().len();
        }
        for word in line.split_whitespace() {
            words.push(word.to_string());
        }
    }

    if words.is_empty() { return; }

    let indent = if crown_margin { first_indent } else { 0 };

    // Fill lines to width
    let mut current_line = String::new();
    let current_indent = if crown_margin {
        // First line of paragraph may have different indent
        0
    } else {
        indent
    };

    for word in &words {
        let available = width.saturating_sub(current_line.len());
        if current_line.is_empty() {
            current_line = format!("{:indent$}{}", "", word, indent = current_indent);
        } else if word.len() + 1 <= available {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            writeln!(out, "{}", current_line).ok();
            current_line = format!("{:indent$}{}", "", word, indent = indent);
        }
    }

    if !current_line.is_empty() {
        writeln!(out, "{}", current_line).ok();
    }
}
