//! Rust port of the MINIX/NetBSD `printf` utility.
//!
//! Usage:
//!   printf format [arguments ...]
//!
//! Formats and prints data according to a format string (POSIX printf).

use std::io::{self, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: printf format [arguments ...]");
        std::process::exit(1);
    }

    let format = args[1].clone();
    let mut fmt_args: Vec<&str> = args[2..].iter().map(|s| s.as_str()).collect();

    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Parse format string and expand escapes
    let format = expand_escapes(&format);
    let mut arg_idx = 0;
    let mut chars = format.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            // Escape sequence (already handled in expand_escapes)
            continue;
        }
        if ch != '%' {
            let _ = write!(out, "{ch}");
            continue;
        }

        // Format specifier: %[flags][width][.precision][length]conversion
        let mut spec = String::from("%");
        let mut flags_done = false;
        let mut width_str = String::new();
        let mut prec_str = String::new();
        let mut in_prec = false;

        loop {
            match chars.next() {
                Some(c) if !flags_done && matches!(c, '-' | '+' | ' ' | '#' | '0' | '\'') => {
                    spec.push(c);
                }
                Some(c) if c == '.' => {
                    in_prec = true;
                    spec.push(c);
                    flags_done = true;
                }
                Some(c) if c.is_ascii_digit() => {
                    if in_prec { prec_str.push(c); } else { width_str.push(c); }
                    spec.push(c);
                    flags_done = true;
                }
                Some(c) if matches!(c, 'l' | 'h' | 'z' | 't' | 'j' | 'L') => {
                    spec.push(c);
                }
                Some(c @ '$') => {
                    spec.push(c);
                }
                Some(c @ ('d' | 'i' | 'o' | 'u' | 'x' | 'X' | 'f' | 'F' | 'e' | 'E' | 'g' | 'G' | 'a' | 'A' | 'c' | 's' | '%' | 'b')) => {
                    spec.push(c);
                    if c == 'c' {
                        let arg = fmt_args.get(arg_idx).map(|s| s.chars().next().unwrap_or('\0')).unwrap_or('\0');
                        let _ = write!(out, "{arg}");
                        arg_idx += 1;
                    } else if c == 's' {
                        let arg = fmt_args.get(arg_idx).unwrap_or(&"");
                        let _ = write!(out, "{arg}");
                        arg_idx += 1;
                    } else if c == '%' {
                        let _ = write!(out, "%");
                    } else if c == 'b' {
                        // %b: expand escapes in argument
                        let arg = fmt_args.get(arg_idx).unwrap_or(&"");
                        let _ = write!(out, "{}", expand_escapes(arg));
                        arg_idx += 1;
                    } else {
                        // Numeric format
                        let arg = fmt_args.get(arg_idx).copied().unwrap_or("0");
                        // Try to parse as number
                        let num: f64 = arg.parse().unwrap_or(0.0);
                        // Apply format via Rust's format system
                        // Strip length modifiers from spec
                        let rust_spec = spec.replace("ll", "").replace('l', "").replace('h', "").replace('z', "").replace('t', "").replace('j', "").replace('L', "");
                        let output = if rust_spec.ends_with('f') || rust_spec.ends_with('F') || rust_spec.ends_with('e') || rust_spec.ends_with('E') || rust_spec.ends_with('g') || rust_spec.ends_with('G') || rust_spec.ends_with('a') || rust_spec.ends_with('A') {
                            // Floating point
                            if rust_spec == "%f" || rust_spec == "%F" {
                                format!("{num}")
                            } else if rust_spec == "%e" || rust_spec == "%E" {
                                format!("{num:e}")
                            } else if rust_spec == "%g" || rust_spec == "%G" {
                                format!("{num:g}")
                            } else {
                                format!("{rust_spec}", num = num)
                            }
                        } else if rust_spec.ends_with('d') || rust_spec.ends_with('i') {
                            let num_i = num as i64;
                            format!("{rust_spec}", num_i = num_i)
                        } else if rust_spec.ends_with('u') {
                            let num_u = num as u64;
                            format!("{rust_spec}", num_u = num_u)
                        } else if rust_spec.ends_with('o') {
                            let num_u = num as u64;
                            match rust_spec.as_str() {
                                "%#o" => format!("{num_u:o}"),
                                _ => format!("{num_u:o}"),
                            }
                        } else if rust_spec.ends_with('x') {
                            let num_u = num as u64;
                            format!("{num_u:x}")
                        } else if rust_spec.ends_with('X') {
                            let num_u = num as u64;
                            format!("{num_u:X}")
                        } else {
                            format!("{rust_spec}", num = num)
                        };
                        let _ = write!(out, "{output}");
                        arg_idx += 1;
                    }
                    break;
                }
                Some(c) => {
                    let _ = write!(out, "{c}");
                    break;
                }
                None => break,
            }
        }
    }

    let _ = out.flush();
}

fn expand_escapes(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('a') => out.push('\u{0007}'),
                Some('b') => out.push('\u{0008}'),
                Some('c') => { std::process::exit(0); } // \c: exit
                Some('e') => out.push('\u{001b}'),
                Some('f') => out.push('\u{000c}'),
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some('v') => out.push('\u{000b}'),
                Some('\\') => out.push('\\'),
                Some('\'') => out.push('\''),
                Some('\"') => out.push('\"'),
                Some(d @ '0'..='7') => {
                    let mut oct = String::from(d);
                    for _ in 0..2 {
                        match chars.next() {
                            Some(o @ '0'..='7') => oct.push(o),
                            _ => break,
                        }
                    }
                    if let Ok(val) = u32::from_str_radix(&oct, 8) {
                        out.push(char::from_u32(val).unwrap_or('?'));
                    }
                }
                Some('x') => {
                    let mut hex = String::new();
                    for _ in 0..2 {
                        match chars.next() {
                            Some(d @ '0'..='9' | d @ 'a'..='f' | d @ 'A'..='F') => hex.push(d),
                            _ => break,
                        }
                    }
                    if let Ok(val) = u32::from_str_radix(&hex, 16) {
                        out.push(char::from_u32(val).unwrap_or('?'));
                    }
                }
                Some(other) => { out.push('\\'); out.push(other); }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}
