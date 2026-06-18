//! Rust port of the MINIX/NetBSD `seq` utility.
//!
//! Usage:
//!   seq [-w] [-f format] [-s separator] [-t terminator] [first [incr]] last
//!
//! Prints a sequence of numbers from FIRST (default 1) to LAST by INCR (default 1).

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut opts = SeqOptions::default();

    // Parse options (manual, before positional args)
    let mut i = 1;
    while i < args.len() && args[i].starts_with('-') {
        let opt = args[i].clone();
        if opt == "--" { i += 1; break; }
        let mut chars = opt.chars();
        chars.next(); // skip '-'
        match chars.next() {
            Some('f') => {
                i += 1;
                if i < args.len() { opts.format = args[i].clone(); }
                else { eprint_usage(); std::process::exit(1); }
            }
            Some('s') => {
                i += 1;
                if i < args.len() { opts.separator = unescape(&args[i]); }
                else { eprint_usage(); std::process::exit(1); }
            }
            Some('t') => {
                i += 1;
                if i < args.len() { opts.terminator = Some(unescape(&args[i])); }
                else { eprint_usage(); std::process::exit(1); }
            }
            Some('w') => { opts.equalize = true; }
            Some('h') | _ => { eprint_usage(); std::process::exit(1); }
        }
        i += 1;
    }

    // Collect positional arguments
    let pos_args: Vec<&str> = args[i..].iter().map(String::as_str).collect();
    if pos_args.is_empty() || pos_args.len() > 3 {
        eprint_usage();
        std::process::exit(1);
    }

    // Parse numeric arguments
    let last: f64 = pos_args[pos_args.len() - 1].parse().unwrap_or_else(|_| {
        eprintln!("seq: invalid floating point argument: {}", pos_args[pos_args.len() - 1]);
        std::process::exit(2);
    });

    let first: f64 = if pos_args.len() > 1 {
        pos_args[0].parse().unwrap_or_else(|_| {
            eprintln!("seq: invalid floating point argument: {}", pos_args[0]);
            std::process::exit(2);
        })
    } else {
        1.0
    };

    let incr: f64 = if pos_args.len() > 2 {
        let val: f64 = pos_args[1].parse().unwrap_or_else(|_| {
            eprintln!("seq: invalid floating point argument: {}", pos_args[1]);
            std::process::exit(2);
        });
        if val == 0.0 {
            eprintln!("seq: zero increment");
            std::process::exit(1);
        }
        val
    } else {
        if first < last { 1.0 } else { -1.0 }
    };

    // Validate direction
    if incr > 0.0 && first > last {
        eprintln!("seq: needs positive increment");
        std::process::exit(1);
    }
    if incr < 0.0 && first < last {
        eprintln!("seq: needs negative decrement");
        std::process::exit(1);
    }

    // Determine format
    let fmt = if opts.format.is_empty() {
        if opts.equalize {
            generate_format(first, incr, last)
        } else {
            "%g".to_string()
        }
    } else {
        opts.format.clone()
    };

    // Validate format string
    if !valid_format(&fmt) {
        eprintln!("seq: invalid format string: `{fmt}'");
        std::process::exit(1);
    }

    // Print the sequence
    let sep = &opts.separator;
    let mut count = first;
    if incr > 0.0 {
        while count <= last + 1e-12 {
            print_value(&fmt, count, sep);
            count += incr;
        }
    } else {
        while count >= last - 1e-12 {
            print_value(&fmt, count, sep);
            count += incr;
        }
    }

    if let Some(ref term) = opts.terminator {
        print!("{term}");
    }
}

struct SeqOptions {
    format: String,
    separator: String,
    terminator: Option<String>,
    equalize: bool,
}

impl Default for SeqOptions {
    fn default() -> Self {
        SeqOptions {
            format: String::new(),
            separator: "\n".to_string(),
            terminator: None,
            equalize: false,
        }
    }
}

fn eprint_usage() {
    eprintln!("usage: seq [-w] [-f format] [-s string] [-t string] [first [incr]] last");
}

fn print_value(fmt: &str, val: f64, sep: &str) {
    if fmt == "%g" || fmt == "%f" {
        print!("{val}{sep}");
    } else if fmt == "%e" {
        print!("{val:e}{sep}");
    } else {
        // Parse printf-style format: %[width][.precision][conv]
        // Translate to Rust: {val:>width$.precision$}
        if let Some(pct) = fmt.find('%') {
            let spec = &fmt[pct + 1..];
            let digits_end = spec.find(|c: char| !c.is_ascii_digit()).unwrap_or(spec.len());
            let width_str = &spec[..digits_end];
            let rest = &spec[digits_end..];

            let prec_str = if rest.starts_with('.') {
                let pstart = 1;
                let pend = rest[pstart..]
                    .find(|c: char| !c.is_ascii_digit())
                    .unwrap_or(rest[pstart..].len());
                &rest[pstart..pstart + pend]
            } else {
                ""
            };

            let width: usize = width_str.parse().unwrap_or(0);
            let prec: usize = prec_str.parse().unwrap_or(6);

            if width > 0 {
                print!("{val:>width$.prec$}{sep}");
            } else if !prec_str.is_empty() && prec_str != "6" {
                print!("{val:.prec$}{sep}");
            } else if fmt.contains('e') || fmt.contains('E') {
                print!("{val:e}{sep}");
            } else {
                print!("{val}{sep}");
            }
        } else {
            print!("{val}{sep}");
        }
    }
}

fn valid_format(fmt: &str) -> bool {
    if fmt == "%g" || fmt == "%f" || fmt == "%e" {
        return true;
    }
    // Accept common variations: %w.pf, %wg, %we, %wd
    let mut chars = fmt.chars().peekable();
    if chars.next() != Some('%') { return false; }
    // Flags
    while let Some(&f) = chars.peek() {
        if "#0- +'".contains(f) { chars.next(); } else { break; }
    }
    // Width
    while let Some(&w) = chars.peek() {
        if w.is_ascii_digit() { chars.next(); } else { break; }
    }
    // .precision
    if chars.peek() == Some(&'.') {
        chars.next();
        while let Some(&p) = chars.peek() {
            if p.is_ascii_digit() { chars.next(); } else { break; }
        }
    }
    // Conversion specifier
    matches!(chars.next(), Some('A' | 'a' | 'E' | 'e' | 'F' | 'f' | 'g'))
}

fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('a') => out.push('\u{0007}'),
                Some('b') => out.push('\u{0008}'),
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

fn generate_format(first: f64, incr: f64, last: f64) -> String {
    let last_val = if first > last {
        first - incr * ((first - last) / incr).floor()
    } else {
        first + incr * ((last - first) / incr).floor()
    };

    let width1 = format!("{first}").len();
    let width2 = format!("{last_val}").len();
    let width = width1.max(width2);

    let dp1 = decimal_places(&format!("{first}"));
    let dp2 = decimal_places(&format!("{incr}"));
    let dp3 = decimal_places(&format!("{last_val}"));
    let precision = dp1.max(dp2.max(dp3));

    if precision > 0 {
        format!("%{w}.{p}f", w = width + 1 + precision, p = precision)
    } else {
        format!("%{w}g", w = width)
    }
}

fn decimal_places(s: &str) -> usize {
    if let Some(dot) = s.find('.') {
        let frac = &s[dot + 1..];
        let frac = frac.split(|c| c == 'e' || c == 'E').next().unwrap_or(frac);
        frac.trim_end_matches('0').len()
    } else {
        0
    }
}
