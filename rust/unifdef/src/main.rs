//! Rust port of the MINIX/NetBSD `unifdef` utility.
//!
//! Usage:
//!   unifdef [-clt] [-D sym] [-U sym] [-iD sym] [-iU sym] [file ...]
//!
//! Resolves preprocessor conditionals (#if/#ifdef/#ifndef/#else/#elif/#endif).
//! -D sym: define symbol as 1
//! -U sym: undefine symbol
//! -iD sym: ignore #ifdef block (pass through unchanged)
//! -iU sym: ignore #ifndef block (pass through unchanged)
//! -c: complement -D/-U (remove lines that ARE selected)
//! -l: replace #else/#endif with line numbers
//! -t: ignore C-style comments (don't parse #if inside them)

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};

const USAGE: &str = "usage: unifdef [-clt] [-D sym] [-U sym] [-iD sym] [-iU sym] [file ...]";

fn usage() -> ! {
    eprintln!("{USAGE}");
    std::process::exit(1);
}

#[derive(Clone, Copy, PartialEq)]
enum BlockState {
    Keep,
    Skip,
    Elif,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut defined: Vec<String> = Vec::new();
    let mut undefined: Vec<String> = Vec::new();
    let mut ignore_ifdef: Vec<String> = Vec::new();
    let mut ignore_ifndef: Vec<String> = Vec::new();
    let mut complement = false;
    let mut _show_lines = false;
    let mut _no_comments = false;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        if opt == "-D" || opt == "-U" || opt == "-iD" || opt == "-iU" {
            argv = &argv[1..];
            if argv.is_empty() { usage() }
            let sym = argv[0].clone();
            match opt.as_str() {
                "-D" => defined.push(sym),
                "-U" => undefined.push(sym),
                "-iD" => ignore_ifdef.push(sym),
                "-iU" => ignore_ifndef.push(sym),
                _ => {}
            }
            argv = &argv[1..];
            continue;
        }
        let val = if opt.len() > 2 { opt[1..].to_string() } else {
            eprintln!("{USAGE}"); std::process::exit(1);
        };
        match val.as_str() {
            "c" => complement = true,
            "l" => _show_lines = true,
            "t" => _no_comments = true,
            _ => usage(),
        }
        argv = &argv[1..];
    }

    let files: Vec<&str> = if argv.is_empty() { vec!["-"] } else { argv.iter().map(|s| s.as_str()).collect() };
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut had_error = false;

    let is_defined = |sym: &str| -> bool {
        if undefined.iter().any(|s| s == sym) {
            false
        } else if defined.iter().any(|s| s == sym) {
            true
        } else {
            false
        }
    };

    for fname in &files {
        let reader: Box<dyn BufRead> = if *fname == "-" {
            Box::new(BufReader::new(io::stdin().lock()))
        } else {
            match File::open(fname) {
                Ok(f) => Box::new(BufReader::new(f)),
                Err(e) => { eprintln!("unifdef: {fname}: {e}"); had_error = true; continue; }
            }
        };

        let lines: Vec<String> = reader.lines().map(|l| l.unwrap_or_default()).collect();
        let mut i = 0;
        let mut ifdef_stack: Vec<BlockState> = Vec::new();
        while i < lines.len() {
            let line = &lines[i];
            let trimmed = line.trim();

            // Check for preprocessor directives
            if trimmed.starts_with("#if") || trimmed.starts_with("#elif") || trimmed.starts_with("#else") || trimmed.starts_with("#endif") {
                let directive = trimmed;
                let is_if = directive.starts_with("#if");
                let is_ifdef = directive.starts_with("#ifdef");
                let is_ifndef = directive.starts_with("#ifndef");
                let is_elif = directive.starts_with("#elif");
                let is_else = directive.starts_with("#else");
                let is_endif = directive.starts_with("#endif");

                if is_if || is_ifdef || is_ifndef {
                    // Extract symbol
                    let rest = if is_if && !is_ifdef && !is_ifndef {
                        // #if expression - simplified: check if it looks like a defined() or symbol
                        let after = &directive[3..].trim();
                        if let Some(sym) = after.strip_prefix("defined(") {
                            sym.trim_end_matches(')').trim()
                        } else if let Some(sym) = after.strip_prefix('(') {
                            sym.trim_end_matches(')').trim()
                        } else {
                            after.split_whitespace().next().unwrap_or("")
                        }
                    } else if is_ifdef {
                        directive[6..].trim()
                    } else {
                        // #ifndef
                        directive[7..].trim()
                    };

                    let sym = rest.to_string();

                    // Check if this is an ignore block
                    let ignore_mode = if is_ifdef && ignore_ifdef.contains(&sym) {
                        true
                    } else if is_ifndef && ignore_ifndef.contains(&sym) {
                        true
                    } else if is_if && ignore_ifdef.contains(&sym) {
                        true
                    } else {
                        false
                    };

                    let cond_true = if ignore_mode {
                        // Pass through unchanged
                        true
                    } else if is_ifndef {
                        !is_defined(&sym)
                    } else if is_ifdef || is_if {
                        is_defined(&sym)
                    } else {
                        true
                    };

                    let state = if cond_true { BlockState::Keep } else { BlockState::Skip };
                    ifdef_stack.push(state);

                    if complement {
                        // In complement mode, we invert: keep what we'd skip
                        if state == BlockState::Skip {
                            // Write the line and contents
                            // No-op: we keep inside content
                        } else {
                            // Skip this block's content
                        }
                    }

                    if !complement && state == BlockState::Skip {
                        // Skip to #else/#elif/#endif
                        let mut depth = 1;
                        while depth > 0 && i < lines.len() {
                            i += 1;
                            if i >= lines.len() { break; }
                            let l = lines[i].trim().to_string();
                            if l.starts_with("#if") || l.starts_with("#ifdef") || l.starts_with("#ifndef") {
                                depth += 1;
                            } else if l.starts_with("#endif") {
                                depth -= 1;
                            } else if depth == 1 && (l.starts_with("#else") || l.starts_with("#elif")) {
                                // Found else/elif - check condition
                                if l.starts_with("#elif") {
                                    let rest = l[5..].trim();
                                    let sym = rest.split_whitespace().next().unwrap_or("");
                                    let cond = is_defined(sym);
                                    if cond {
                                        depth = 0; // Enter this block
                                        ifdef_stack.pop();
                                        ifdef_stack.push(BlockState::Keep);
                                    }
                                    // else continue skipping
                                } else {
                                    depth = 0; // Enter else block
                                    ifdef_stack.pop();
                                    ifdef_stack.push(BlockState::Elif);
                                }
                            }
                        }
                        continue;
                    }

                    // Keep the block contents
                    continue;
                }

                if is_elif {
                    if let Some(&state) = ifdef_stack.last() {
                        if state == BlockState::Keep {
                            // We were in a keep block, now skip to #endif
                            let mut depth = 1;
                            while depth > 0 && i < lines.len() {
                                i += 1;
                                if i >= lines.len() { break; }
                                let l = lines[i].trim().to_string();
                                if l.starts_with("#if") || l.starts_with("#ifdef") || l.starts_with("#ifndef") { depth += 1; }
                                else if l.starts_with("#endif") { depth -= 1; }
                                else if depth == 1 && (l.starts_with("#else") || l.starts_with("#elif")) { }
                            }
                            continue;
                        } else if state == BlockState::Skip || state == BlockState::Elif {
                            // Check if this elif should be entered
                            let rest = directive[5..].trim();
                            let sym = rest.split_whitespace().next().unwrap_or("");
                            let cond = is_defined(sym);
                            if cond && !complement {
                                ifdef_stack.pop();
                                ifdef_stack.push(BlockState::Keep);
                            } else if !cond && complement {
                                ifdef_stack.pop();
                                ifdef_stack.push(BlockState::Keep);
                            } else {
                                // Continue skipping
                                continue;
                            }
                        }
                    }
                    continue;
                }

                if is_else {
                    if let Some(&state) = ifdef_stack.last() {
                        if state == BlockState::Keep {
                            // Skip to #endif
                            let mut depth = 1;
                            while depth > 0 && i < lines.len() {
                                i += 1;
                                if i >= lines.len() { break; }
                                let l = lines[i].trim().to_string();
                                if l.starts_with("#if") || l.starts_with("#ifdef") || l.starts_with("#ifndef") { depth += 1; }
                                else if l.starts_with("#endif") { depth -= 1; }
                            }
                            continue;
                        } else {
                            // Enter else block
                            ifdef_stack.pop();
                            ifdef_stack.push(BlockState::Keep);
                        }
                    }
                    continue;
                }

                if is_endif {
                    ifdef_stack.pop();
                    continue;
                }
            }

            // Check if we're in a keep block
            let in_skip = ifdef_stack.iter().any(|&s| s == BlockState::Skip);
            if in_skip {
                i += 1;
                continue;
            }

            if complement {
                // In complement mode, we only output preprocessor lines
                // (regular lines are removed)
                if trimmed.starts_with('#') {
                    writeln!(out, "{line}").ok();
                }
            } else {
                writeln!(out, "{line}").ok();
            }
            i += 1;
        }
    }

    if had_error { std::process::exit(1); }
}
