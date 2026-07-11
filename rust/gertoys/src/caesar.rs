// caesar — Caesar cipher with optional auto-guess.
//
// With no arguments, reads stdin and auto-guesses the rotation using
// English letter frequency analysis. With a rotation argument, applies
// that rotation.

use std::io::Read;

/// Standard English letter frequencies (percent).
static STDF: [f64; 26] = [
    7.97, 1.35, 3.61, 4.78, 12.37, 2.01, 1.46, 4.49, 6.39, 0.04,
    0.42, 3.81, 2.69, 5.92, 6.96,  2.91, 0.08, 6.63, 8.77, 9.68,
    2.62, 0.81, 1.88, 0.23, 2.07,  0.06,
];

fn rotate_byte(b: u8, rot: usize) -> u8 {
    match b {
        b'A'..=b'Z' => {
            let idx = (b - b'A') as usize;
            b'A' + ((idx + rot) % 26) as u8
        }
        b'a'..=b'z' => {
            let idx = (b - b'a') as usize;
            b'a' + ((idx + rot) % 26) as u8
        }
        _ => b,
    }
}

fn apply_rotation(input: &[u8], rot: usize) {
    for &b in input {
        print!("{}", rotate_byte(b, rot) as char);
    }
}

fn process_stdin(rot: usize) {
    let mut buf = Vec::new();
    std::io::stdin().read_to_end(&mut buf).unwrap_or(0);
    apply_rotation(&buf, rot);
}

fn guess_and_rotate(input: &[u8]) {
    let mut obs = [0u64; 26];
    for &b in input {
        let idx = if b >= b'A' && b <= b'Z' {
            (b - b'A') as usize
        } else if b >= b'a' && b <= b'z' {
            (b - b'a') as usize
        } else {
            continue;
        };
        obs[idx] += 1;
    }

    // Adjusted frequencies (log-weighted)
    let mut adj = [0.0f64; 26];
    for i in 0..26 {
        adj[i] = (STDF[i] / 100.0).ln() + (26.0f64 / 100.0).ln();
    }

    // Find best rotation via dot product
    let mut winner = 0usize;
    let mut best_dot = f64::NEG_INFINITY;
    for try_rot in 0..26 {
        let mut dot = 0.0;
        for i in 0..26 {
            let shifted = (i + try_rot) % 26;
            dot += (obs[i] as f64) * adj[shifted];
        }
        if try_rot == 0 || dot > best_dot {
            best_dot = dot;
            winner = try_rot;
        }
    }

    apply_rotation(input, winner);
}

pub fn run(args: &[String]) {
    if args.is_empty() {
        let mut input = Vec::new();
        std::io::stdin().read_to_end(&mut input).unwrap_or(0);
        guess_and_rotate(&input);
    } else if args.len() == 1 {
        let rot: usize = match args[0].parse() {
            Ok(n) => n,
            Err(_) => {
                eprintln!("caesar: Bad rotation value '{}'", args[0]);
                std::process::exit(1);
            }
        };
        process_stdin(rot);
    } else {
        eprintln!("usage: gertoys caesar [rotation]");
        std::process::exit(1);
    }
}
