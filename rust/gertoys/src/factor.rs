// factor — Factor numbers into their prime factors.
//
// Port of BSD games/factor by Landon Curt Noll.
//
// Usage:
//   gertoys factor [number ...]
//
// Reads numbers from stdin if none given on command line.

use std::io::{self, BufRead};

#[inline]
fn is_prime(n: u64) -> bool {
    if n < 2 {
        return false;
    }
    if n % 2 == 0 {
        return n == 2;
    }
    if n % 3 == 0 {
        return n == 3;
    }
    if n % 5 == 0 {
        return n == 5;
    }
    let mut i = 7u64;
    let mut step = 4;
    while i * i <= n && i * i > i {
        if n % i == 0 {
            return false;
        }
        i += step;
        step = 6 - step; // 7, 11, 13, 17, 19, 23, ...
    }
    true
}

fn factor(mut n: u64) -> Vec<u64> {
    let mut factors = Vec::new();

    // Handle small primes
    while n % 2 == 0 {
        factors.push(2);
        n /= 2;
    }
    while n % 3 == 0 {
        factors.push(3);
        n /= 3;
    }
    while n % 5 == 0 {
        factors.push(5);
        n /= 5;
    }

    let mut i = 7u64;
    let mut step = 4;
    while i * i <= n {
        while n % i == 0 {
            factors.push(i);
            n /= i;
        }
        i += step;
        step = 6 - step;
    }

    if n > 1 {
        factors.push(n);
    }

    factors
}

fn pr_fact(val: u64) {
    if val <= 1 {
        eprintln!("factor: numbers <= 1 aren't permitted.");
        return;
    }

    if is_prime(val) {
        println!("{}: {}", val, val);
        return;
    }

    let factors = factor(val);
    print!("{}:", val);
    for f in &factors {
        print!(" {}", f);
    }
    println!();
}

pub fn run(args: &[String]) {
    if args.is_empty() {
        // Read from stdin
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(l) => {
                    let trimmed = l.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if trimmed.starts_with('-') {
                        eprintln!("factor: negative numbers aren't permitted.");
                        continue;
                    }
                    match trimmed.parse::<u64>() {
                        Ok(n) => pr_fact(n),
                        Err(_) => eprintln!("factor: '{}': illegal numeric format.", trimmed),
                    }
                }
                Err(_) => break,
            }
        }
    } else {
        for arg in args {
            if arg.starts_with('-') {
                eprintln!("factor: numbers <= 1 aren't permitted.");
                continue;
            }
            match arg.parse::<u64>() {
                Ok(n) => pr_fact(n),
                Err(_) => eprintln!("factor: '{}': illegal numeric format.", arg),
            }
        }
    }
}
