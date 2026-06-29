//! Rust port of the MINIX/NetBSD `getopt` utility.
//!
//! Usage:
//!   getopt optstring parameters
//!
//! Parses command-line options and outputs them in a canonical format
//! for use in shell scripts.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: getopt optstring parameters");
        std::process::exit(1);
    }

    let optstring = &args[1];
    let params: Vec<&str> = args[2..].iter().map(|s| s.as_str()).collect();

    // Parse optstring: letters with : for required arg, :: for optional
    let mut flags: Vec<char> = Vec::new();
    let mut has_arg: Vec<u32> = Vec::new();
    let mut chars = optstring.chars();
    while let Some(c) = chars.next() {
        if c == ':' {
            continue;
        }
        flags.push(c);
        match chars.clone().next() {
            Some(':') => {
                chars.next();
                match chars.clone().next() {
                    Some(':') => {
                        chars.next();
                        has_arg.push(2); // optional arg
                    }
                    _ => has_arg.push(1), // required arg
                }
            }
            _ => has_arg.push(0),
        }
    }

    let mut i = 0;
    let mut out_flags: Vec<String> = Vec::new();
    let mut non_opts: Vec<String> = Vec::new();
    let mut end_of_opts = false;

    while i < params.len() {
        let param = params[i];

        if param == "--" {
            end_of_opts = true;
            i += 1;
            break;
        }

        if end_of_opts || !param.starts_with('-') || param == "-" {
            non_opts.push(param.to_string());
            i += 1;
            continue;
        }

        // Parse options in this arg
        let chars: Vec<char> = param.chars().skip(1).collect();
        for (j, &flag) in chars.iter().enumerate() {
            if let Some(pos) = flags.iter().position(|&f| f == flag) {
                let needs = has_arg[pos];
                if needs == 1 {
                    if j + 1 < chars.len() {
                        // Rest of this arg is the value
                        let val: String = chars[j+1..].iter().collect();
                        out_flags.push(format!("-{} {}", flag, val));
                        break;
                    } else if i + 1 < params.len() {
                        i += 1;
                        out_flags.push(format!("-{} {}", flag, params[i]));
                    } else {
                        eprintln!("getopt: option requires an argument -- {}", flag);
                        std::process::exit(1);
                    }
                } else if needs == 2 {
                    if j + 1 < chars.len() {
                        let val: String = chars[j+1..].iter().collect();
                        out_flags.push(format!("-{} {}", flag, val));
                        break;
                    } else {
                        out_flags.push(format!("-{}", flag));
                    }
                } else {
                    out_flags.push(format!("-{}", flag));
                }
            } else {
                eprintln!("getopt: illegal option -- {}", flag);
                std::process::exit(1);
            }
        }
        i += 1;
    }

    // Remaining args are non-options
    for &p in &params[i..] {
        non_opts.push(p.to_string());
    }

    // Output: flags then -- then non-options
    for f in &out_flags {
        print!("{} ", f);
    }
    if !non_opts.is_empty() {
        print!("-- ");
        for n in &non_opts {
            print!("{} ", n);
        }
    }
    println!();
}
