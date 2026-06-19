//! Rust port of the MINIX/NetBSD `tr` utility.
//!
//! Usage:
//!   tr [-cds] string1 [string2]
//!
//! Translates, squashes, and/or deletes characters from stdin.

use std::io::{self, Read, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut complement = false;
    let mut delete = false;
    let mut squeeze = false;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        for c in argv[0].chars().skip(1) {
            match c {
                'c' => complement = true,
                'd' => delete = true,
                's' => squeeze = true,
                _ => {
                    eprintln!("tr: unknown option -- {c}");
                    std::process::exit(1);
                }
            }
        }
        argv = &argv[1..];
    }

    if argv.is_empty() || (delete && argv.len() < 1) || (!delete && argv.len() < 2) {
        eprintln!("usage: tr [-cds] string1 [string2]");
        std::process::exit(1);
    }

    let s1 = expand_set(&argv[0]);
    let s2 = if argv.len() > 1 { expand_set(&argv[1]) } else { String::new() };

    if delete {
        let delete_chars: Vec<char> = if complement {
            // Delete everything NOT in s1
            let s1_chars: Vec<char> = s1.chars().collect();
            // Iterate over all bytes 0-255
            (0u8..=255u8)
                .map(|b| b as char)
                .filter(|c| !s1_chars.contains(c))
                .collect()
        } else {
            s1.chars().collect()
        };

        let stdin = io::stdin().lock();
        let stdout = io::stdout();
        let mut out = stdout.lock();

        for byte in stdin.bytes() {
            let byte = match byte { Ok(b) => b, Err(_) => break };
            let c = byte as char;
            if !delete_chars.contains(&c) {
                let _ = out.write_all(&[byte]);
            }
        }
        return;
    }

    // Translation
    let s1_chars: Vec<char> = if complement {
        let s1_set: Vec<char> = s1.chars().collect();
        let last_s2 = s2.chars().last().unwrap_or_default();
        // Build translation map: everything NOT in s1 -> last char of s2
        let mut map = Vec::new();
        for c in 0u8..=255 {
            let ch = c as char;
            if s1_set.contains(&ch) {
                map.push(ch); // Keep original
            } else {
                let idx = s1_set.iter().position(|&x| x == ch).unwrap_or(s1_set.len().saturating_sub(1));
                map.push(s2.chars().nth(idx.min(s2.chars().count().saturating_sub(1))).unwrap_or(last_s2));
            }
        }
        map
    } else {
        s1.chars().collect()
    };

    let s2_chars: Vec<char> = s2.chars().collect();

    let stdin = io::stdin().lock();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut last_char: Option<char> = None;

    for byte in stdin.bytes() {
        let byte = match byte { Ok(b) => b, Err(_) => break };
        let c = byte as char;

        let translated = if let Some(idx) = s1_chars.iter().position(|&x| x == c) {
            if idx < s2_chars.len() { s2_chars[idx] } else { s2_chars.last().copied().unwrap_or(c) }
        } else {
            c
        };

        if squeeze && last_char == Some(translated) && s2_chars.contains(&translated) {
            continue;
        }

        let _ = out.write_all(&[translated as u8]);
        last_char = Some(translated);
    }
}

fn expand_set(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('a') => out.push('\u{0007}'),
                Some('b') => out.push('\u{0008}'),
                Some('f') => out.push('\u{000c}'),
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some('v') => out.push('\u{000b}'),
                Some('\\') => out.push('\\'),
                Some(oct @ '0'..='7') => {
                    let mut num = String::from(oct);
                    for _ in 0..2 {
                        match chars.peek() {
                            Some(d @ '0'..='7') => { num.push(*d); chars.next(); }
                            _ => break,
                        }
                    }
                    if let Ok(val) = u32::from_str_radix(&num, 8) {
                        out.push(char::from_u32(val).unwrap_or('?'));
                    }
                }
                Some(other) => { out.push('\\'); out.push(other); }
                None => out.push('\\'),
            }
        } else if c == '-' {
            // Range: a-z
            let prev = out.chars().last();
            let next = chars.next();
            if let (Some(p), Some(n)) = (prev, next) {
                for r in (p as u8 + 1)..(n as u8) {
                    out.push(r as char);
                }
                out.push(n);
            } else {
                out.push('-');
                if let Some(n) = next { out.push(n); }
            }
        } else if c == '[' {
            // Character class like [:alpha:] — skip for now
            let mut class = String::from('[');
            loop {
                match chars.next() {
                    Some(']') => { class.push(']'); break; }
                    Some(c) => class.push(c),
                    None => break,
                }
            }
            // Simple: just output as-is
            out.push_str(&class);
        } else {
            out.push(c);
        }
    }
    out
}
