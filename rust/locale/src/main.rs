//! Rust port of the MINIX/NetBSD `locale` utility.
//!
//! Usage:
//!   locale [-a] [-m] [name ...]
//!
//! Shows locale settings and available locales.

use std::env;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];
    let mut all = false;
    let mut list_charmaps = false;
    let mut list_aliases = false;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        for ch in opt.chars().skip(1) {
            match ch {
                'a' => all = true,
                'm' => list_charmaps = true,
                'k' => list_aliases = true,
                _ => { eprintln!("usage: locale [-a] [-m] [name ...]"); std::process::exit(1); }
            }
        }
        argv = &argv[1..];
    }

    if all {
        // List available locales (simplified)
        let locales = [
            "C", "C.UTF-8", "en_US.UTF-8", "en_GB.UTF-8",
            "de_DE.UTF-8", "fr_FR.UTF-8", "ja_JP.UTF-8",
            "ru_RU.UTF-8", "zh_CN.UTF-8", "es_ES.UTF-8",
        ];
        for loc in &locales {
            println!("{}", loc);
        }
        return;
    }

    if list_charmaps {
        let charmaps = ["UTF-8", "ISO-8859-1", "ISO-8859-15", "ASCII", "KOI8-R"];
        for cm in &charmaps {
            println!("{}", cm);
        }
        return;
    }

    let names: Vec<&str> = if argv.is_empty() {
        vec!["LC_CTYPE", "LC_NUMERIC", "LC_TIME", "LC_COLLATE",
             "LC_MONETARY", "LC_MESSAGES", "LC_ALL", "LANG"]
    } else {
        argv.iter().map(|s| s.as_str()).collect()
    };

    for name in &names {
        let value = env::var(name).unwrap_or_else(|_| {
            env::var("LANG").unwrap_or_else(|_| "C".to_string())
        });
        println!("{}={}", name, value);
    }
}
