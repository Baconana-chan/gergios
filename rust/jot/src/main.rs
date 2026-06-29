//! Rust port of the MINIX/NetBSD `jot` utility.
//!
//! Usage:
//!   jot [count] [begin] [end] [step]
//!
//! Prints sequential or random data. Default: 100 numbers from 1 to 100.
//!
//! jot itself prints sequential data: count numbers starting at begin,
//! incrementing by step, ending at or before end.
//! If a single argument is given that is not a number, it is printed as
//! a template with 'x' replaced by sequential numbers.

use std::process;

fn usage() -> ! {
    eprintln!("usage: jot [count] [begin] [end] [step]");
    process::exit(1);
}

fn parse_double(s: &str) -> f64 {
    if s.to_lowercase() == "inf" {
        f64::INFINITY
    } else if s.to_lowercase() == "-inf" {
        f64::NEG_INFINITY
    } else {
        s.parse().unwrap_or_else(|_| usage())
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let argv = &args[1..];

    // If there's exactly one arg and it's not a number, treat as a format string
    if argv.len() == 1 && argv[0].parse::<f64>().is_err() && !argv[0].is_empty() {
        let fmt = &argv[0];
        for i in 1..=100 {
            let out = fmt.replace('x', &format!("{i}"))
                        .replace("X", &format!("{i}"));
            println!("{out}");
        }
        return;
    }

    let (count, begin, end, step) = match argv.len() {
        0 => (100.0, 1.0, 100.0, 1.0),
        1 => {
            let c = parse_double(&argv[0]);
            if c <= 0.0 { return; }
            let count_i = c as i64;
            if count_i == 0 { return; }
            if count_i == 1 { (1.0, 1.0, 1.0, 1.0) }
            else { (c, 1.0, c, 1.0) }
        }
        2 => {
            let c = parse_double(&argv[0]);
            let b = parse_double(&argv[1]);
            let e = if c <= 1.0 { b } else { b + c - 1.0 };
            (c, b, e, 1.0)
        }
        3 => {
            let c = parse_double(&argv[0]);
            let b = parse_double(&argv[1]);
            let e = parse_double(&argv[2]);
            (c, b, e, 1.0)
        }
        4 => {
            let c = parse_double(&argv[0]);
            let b = parse_double(&argv[1]);
            let e = parse_double(&argv[2]);
            let s = parse_double(&argv[3]);
            if s == 0.0 { usage(); }
            (c, b, e, s)
        }
        _ => usage(),
    };

    let count_i = count as i64;
    if count_i <= 0 {
        return;
    }

    for i in 0..count_i {
        let val = begin + (i as f64) * step;
        if step > 0.0 && val > end {
            break;
        }
        if step < 0.0 && val < end {
            break;
        }
        println!("{val}");
    }
}
