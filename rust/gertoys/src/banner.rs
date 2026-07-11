// banner — Print large ASCII art banners.
//
// Port of BSD games/banner.
//
// Usage:
//   gertoys banner [-w width] [-d] [-t] [message]
//
// If no message is given, reads from stdin.

use std::io::{self, BufRead, Write};

const DWIDTH: usize = 132;
const NCHARS: usize = 128;

/// Pointers into data_table for each ASCII char.
const ASC_PTR: [usize; NCHARS] = [
/* ^@ */   0,      0,      0,      0,      0,      0,      0,      0,
/* ^H */   0,      0,      0,      0,      0,      0,      0,      0,
/* ^P */   0,      0,      0,      0,      0,      0,      0,      0,
/* ^X */   0,      0,      0,      0,      0,      0,      0,      0,
/*    */   1,      3,     50,     81,    104,    281,    483,    590,
/*  ( */ 621,    685,    749,    851,    862,    893,    898,    921,
/*  0 */1019,   1150,   1200,   1419,   1599,   1744,   1934,   2111,
/*  8 */2235,   2445,   2622,   2659,      0,   2708,      0,   2715,
/*  @ */2857,   3072,   3273,   3403,   3560,   3662,   3730,   3785,
/*  H */3965,   4000,   4015,   4115,   4281,   4314,   4432,   4548,
/*  P */4709,   4790,   4999,   5188,   5397,   5448,   5576,   5710,
/*  X */5892,   6106,   6257,      0,      0,      0,      0,      0,
/*  ` */  50,   6503,   6642,   6733,   6837,   6930,   7073,   7157,
/*  h */7380,   7452,   7499,   7584,   7689,   7702,   7797,   7869,
/*  p */7978,   8069,   8160,   8222,   8381,   8442,   8508,   8605,
/*  x */8732,   8888,   9016,      0,      0,      0,      0,      0,
];

/// Table of stuff to print. Format:
/// 128+n -> print current line n times.
/// 64+n  -> this is last byte of char.
/// else, put m chars at position n (where m is the next elt in array).
static DATA_TABLE: &[u8] = include_bytes!("../data/banner_table.bin");

fn banner_char(ch: u8, print_map: &[u8; DWIDTH], trace: bool) {
    if ch >= NCHARS as u8 || ASC_PTR[ch as usize] == 0 {
        eprintln!(
            "banner: The character '{}' is not in my character set",
            ch as char
        );
        return;
    }

    let mut line = [b' '; DWIDTH];
    let mut pc = ASC_PTR[ch as usize];
    let mut term = 0u32;
    let mut max = 0usize;
    let mut linen = 0usize;

    while term < 2 {
        if pc >= DATA_TABLE.len() {
            eprintln!("banner: bad pc: {}", pc);
            std::process::exit(1);
        }
        let x = DATA_TABLE[pc] as usize;
        if trace {
            eprintln!(
                "pc={}, term={}, max={}, linen={}, x={}",
                pc, term, max, linen, x
            );
        }
        if x >= 128 {
            if x > 192 {
                term += 1;
            }
            let count = x & 63;
            for _ in 0..count {
                if print_map[linen] != 0 {
                    let stdout = io::stdout();
                    let mut handle = stdout.lock();
                    for j in 0..=max {
                        if print_map[j] != 0 {
                            handle.write_all(&[line[j]]).ok();
                        }
                    }
                    handle.write_all(b"\n").ok();
                }
                linen += 1;
            }
            line = [b' '; DWIDTH];
            pc += 1;
        } else {
            let y = DATA_TABLE[pc + 1] as usize;
            max = x + y;
            for pos in x..max {
                line[pos] = b'#';
            }
            pc += 2;
            if trace {
                eprintln!("x={}, y={}, max={}", x, y, max);
            }
        }
    }
}

pub fn run(args: &[String]) {
    let mut trace = false;
    let mut width = DWIDTH;
    let mut i = 0;

    // Parse flags — stop at first non-flag argument
    while i < args.len() {
        match args[i].as_str() {
            "-d" => {
                // Debug mode — just acknowledge
            }
            "-t" => trace = true,
            "-w" => {
                i += 1;
                if i < args.len() {
                    width = args[i].parse().unwrap_or_else(|_| {
                        eprintln!("banner: illegal argument for -w option");
                        std::process::exit(1);
                    });
                    if width == 0 || width > DWIDTH {
                        eprintln!("banner: illegal argument for -w option");
                        std::process::exit(1);
                    }
                }
            }
            _ => {
                // First non-flag argument — stop flag parsing
                break;
            }
        }
        i += 1;
    }

    // Build print_map for width scrunching
    let mut print_map = [0u8; DWIDTH];
    for col in 0..width {
        let j = col * DWIDTH / width;
        if j < DWIDTH {
            print_map[j] = 1;
        }
    }

    // Get message — remaining args after flags, or stdin
    let message = if i < args.len() {
        args[i..].join(" ")
    } else {
        // Read from stdin
        let stdin = io::stdin();
        let mut msg = String::new();
        if stdin.lock().read_line(&mut msg).ok().unwrap_or(0) > 0 {
            msg.trim().to_string()
        } else {
            String::new()
        }
    };

    let message = message.trim().to_string();
    if message.is_empty() {
        eprintln!("banner: no message");
        eprintln!("usage: gertoys banner [-w width] [-d] [-t] [message]");
        std::process::exit(1);
    }

    // Validate message
    let mut bad = 0;
    for b in message.bytes() {
        if (b as usize) >= NCHARS || ASC_PTR[b as usize] == 0 {
            eprintln!(
                "banner: The character '{}' is not in my character set",
                b as char
            );
            bad += 1;
        }
    }
    if bad > 0 {
        std::process::exit(1);
    }

    if trace {
        eprintln!("Message '{}' is OK", message);
    }

    // Render each character
    for (ci, b) in message.bytes().enumerate() {
        if trace {
            eprintln!("Char #{}: {}", ci, b as char);
        }
        banner_char(b, &print_map, trace);
    }
}
