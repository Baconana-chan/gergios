// random — Random line filter or random exit code.
//
// Without arguments: selects ~1/2 of lines from stdin (randomly).
// With denominator N: selects ~1/N of lines.
// -e: return random exit code (0 to N-1) instead of filtering.
// -r: unbuffered output.

use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::io::{BufRead, Read, Write};

static RND_STATE: AtomicU64 = AtomicU64::new(1);

fn fast_srand(seed: u64) {
    RND_STATE.store(seed, Ordering::Relaxed);
}

fn fast_rand() -> i32 {
    let old = RND_STATE.load(Ordering::Relaxed);
    let new = old.wrapping_mul(6364136223846793005)
                 .wrapping_add(1442695040888963407);
    RND_STATE.store(new, Ordering::Relaxed);
    (new >> 33) as i32 & 0x7FFFFFFF
}

fn pick(denom: f64) -> bool {
    (denom * fast_rand() as f64 / 0x7FFFFFFF as f64) as i32 == 0
}

pub fn run(args: &[String]) {
    let mut random_exit = false;
    let mut unbuffer = false;
    let mut denom: f64 = 2.0;

    for arg in args {
        match arg.as_str() {
            "-e" => random_exit = true,
            "-r" => unbuffer = true,
            a if a.starts_with('-') => {
                for ch in a.chars().skip(1) {
                    match ch {
                        'e' => random_exit = true,
                        'r' => unbuffer = true,
                        _ => {
                            eprintln!("usage: gertoys random [-er] [denominator]");
                            std::process::exit(1);
                        }
                    }
                }
            }
            _ => {
                denom = match arg.parse() {
                    Ok(v) if v > 0.0 => v,
                    _ => {
                        eprintln!("random: denominator is not valid.");
                        std::process::exit(1);
                    }
                };
            }
        }
    }

    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    fast_srand(seed);

    if random_exit {
        let r = (denom * fast_rand() as f64 / 0x7FFFFFFF as f64) as i32;
        std::process::exit(r);
    }

    let mut selected = pick(denom);

    if unbuffer {
        let stdout = std::io::stdout();
        let mut out = stdout.lock();
        let mut buf = [0u8; 8192];
        let mut stdin = std::io::stdin();
        loop {
            let n = stdin.read(&mut buf).unwrap_or(0);
            if n == 0 {
                break;
            }
            for &b in &buf[..n] {
                if selected {
                    let _ = out.write(&[b]);
                }
                if b == b'\n' {
                    selected = pick(denom);
                }
            }
        }
    } else {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            let line = line.unwrap_or_default();
            if selected {
                println!("{}", line);
            }
            selected = pick(denom);
        }
    }
}
