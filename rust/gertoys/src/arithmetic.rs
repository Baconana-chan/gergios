// arithmetic — Interactive arithmetic quiz with adaptive difficulty.
//
// Port of BSD games/arithmetic by Eamonn McManus.
//
// Uses Vec-based penalty storage instead of linked lists (more idiomatic Rust)
// and a custom LCG PRNG (MINIX may not have libc random()).
//
// Usage:
//   gertoys arithmetic [-o +-x/] [-r range]

use std::io::{self, BufRead, Write};
use std::time::{SystemTime, UNIX_EPOCH};

const KEYLIST: &[u8] = b"+-x/";
const DEFAULT_KEYS: &[u8] = b"+-";
const NQUESTS: usize = 20;
const WRONG_PENALTY: i32 = 5;

/// A penalty entry for a value in a particular operation/operand.
#[derive(Clone)]
struct Penalty {
    value: i32,
    count: i32, // remaining penalty count
}

/// Penalty store: [op_index][operand_index] → Vec of penalties.
struct PenaltyStore {
    penalties: [[Vec<Penalty>; 2]; 4],
    total: [[i32; 2]; 4], // sum of all penalty counts
}

impl PenaltyStore {
    fn new() -> Self {
        PenaltyStore {
            penalties: [
                [Vec::new(), Vec::new()],
                [Vec::new(), Vec::new()],
                [Vec::new(), Vec::new()],
                [Vec::new(), Vec::new()],
            ],
            total: [[0; 2]; 4],
        }
    }

    fn opnum(op: u8) -> usize {
        KEYLIST.iter().position(|&k| k == op).unwrap_or_else(|| {
            eprintln!("arithmetic: bug: op {} not in keylist", op as char);
            std::process::exit(1);
        })
    }

    fn penalise(&mut self, value: i32, op: u8, operand: usize) {
        let op_idx = Self::opnum(op);
        self.penalties[op_idx][operand].push(Penalty {
            value,
            count: WRONG_PENALTY,
        });
        self.total[op_idx][operand] += WRONG_PENALTY;
    }

    fn getrandom(&mut self, maxval: i32, op: u8, operand: usize, rng_state: &mut u64) -> i32 {
        let op_idx = Self::opnum(op);
        let total = maxval + self.total[op_idx][operand];

        if total <= 0 {
            return 0;
        }

        let r = fastrand(total as u64, rng_state);
        let mut value = (r % total as u64) as i32;

        if value < maxval {
            return value;
        }
        value -= maxval;

        // Find the penalty at position `value` in the list
        let list = &mut self.penalties[op_idx][operand];
        for i in 0..list.len() {
            if list[i].count > value {
                let ret = list[i].value;
                self.total[op_idx][operand] -= 1;
                list[i].count -= 1;
                if list[i].count <= 0 {
                    list.swap_remove(i);
                }
                return ret;
            }
            value -= list[i].count;
        }

        eprintln!("arithmetic: bug: inconsistent penalties");
        std::process::exit(1);
    }
}

// Simple LCG RNG (MMIX parameters)
fn fastrand(limit: u64, state: &mut u64) -> u64 {
    if limit <= 1 {
        return 0;
    }
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *state % limit
}

fn init_rng() -> u64 {
    let start = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    start.as_nanos() as u64 ^ (std::process::id() as u64).wrapping_shl(32)
}

pub fn run(args: &[String]) {
    let mut rng = init_rng();

    let mut keys: Vec<u8> = DEFAULT_KEYS.to_vec();
    let mut rangemax: i32 = 10;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                i += 1;
                if i < args.len() {
                    let s = args[i].as_bytes().to_vec();
                    for &b in &s {
                        if !KEYLIST.contains(&b) {
                            eprintln!("arithmetic: unknown key '{}'", b as char);
                            std::process::exit(1);
                        }
                    }
                    keys = s;
                } else {
                    eprintln!("arithmetic: -o requires an argument");
                    std::process::exit(1);
                }
            }
            "-r" => {
                i += 1;
                if i < args.len() {
                    rangemax = args[i].parse::<i32>().unwrap_or_else(|_| {
                        eprintln!("arithmetic: invalid range");
                        std::process::exit(1);
                    });
                    if rangemax <= 0 {
                        eprintln!("arithmetic: invalid range");
                        std::process::exit(1);
                    }
                } else {
                    eprintln!("arithmetic: -r requires an argument");
                    std::process::exit(1);
                }
            }
            _ => {
                eprintln!("Usage: gertoys arithmetic [-o +-x/] [-r range]");
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let mut ps = PenaltyStore::new();
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut nright: i32 = 0;
    let mut nwrong: i32 = 0;
    let mut qtime: u64 = 0;

    'outer: loop {
        for _ in 0..NQUESTS {
            let op_idx = fastrand(keys.len() as u64, &mut rng) as usize;
            let op = keys[op_idx];

            // Generate problem
            let (left, right, result) = loop {
                #[allow(unused_assignments)]
                let (mut left, mut right, mut result) = (0, 0, 0);

                if op != b'/' {
                    right = ps.getrandom(rangemax + 1, op, 1, &mut rng);
                }

                match op {
                    b'+' => {
                        left = ps.getrandom(rangemax + 1, op, 0, &mut rng);
                        result = left + right;
                    }
                    b'-' => {
                        result = ps.getrandom(rangemax + 1, op, 0, &mut rng);
                        left = right + result;
                    }
                    b'x' => {
                        left = ps.getrandom(rangemax + 1, op, 0, &mut rng);
                        result = left * right;
                    }
                    b'/' => {
                        right = ps.getrandom(rangemax, op, 1, &mut rng) + 1;
                        result = ps.getrandom(rangemax + 1, op, 0, &mut rng);
                        left = right * result + (fastrand(right as u64, &mut rng) % right as u64) as i32;
                    }
                    _ => unreachable!(),
                }

                if result >= 0 && left >= 0 {
                    break (left, right, result);
                }
            };

            print!("{} {} {} =   ", left, op as char, right);
            stdout.flush().ok();

            let start = SystemTime::now();

            'question: loop {
                let mut line = String::new();
                match stdin.lock().read_line(&mut line) {
                    Ok(0) => {
                        println!();
                        break 'outer;
                    }
                    Ok(_) => {}
                    Err(_) => break 'outer,
                }

                let trimmed = line.trim();
                let answer_start = trimmed
                    .as_bytes()
                    .iter()
                    .position(|&c| !c.is_ascii_whitespace());
                let answer_str = match answer_start {
                    Some(pos) => &trimmed[pos..],
                    None => "",
                };

                if answer_str.is_empty()
                    || !answer_str
                        .as_bytes()
                        .first()
                        .map_or(false, |c| c.is_ascii_digit())
                {
                    println!("Please type a number.");
                    continue 'question;
                }

                let guess: i32 = match answer_str
                    .split_whitespace()
                    .next()
                    .unwrap_or("0")
                    .parse()
                {
                    Ok(n) => n,
                    Err(_) => {
                        println!("Please type a number.");
                        continue 'question;
                    }
                };

                if guess == result {
                    println!("Right!");
                    nright += 1;
                    break;
                }

                println!("What?");
                nwrong += 1;
                ps.penalise(right, op, 1);
                if op == b'x' || op == b'+' {
                    ps.penalise(left, op, 0);
                } else {
                    ps.penalise(result, op, 0);
                }
            }

            if let Ok(elapsed) = SystemTime::now().duration_since(start) {
                qtime += elapsed.as_secs();
            }
        }

        // Show stats
        let total = nright + nwrong;
        if total > 0 {
            let score = 100 * nright / total;
            println!(
                "\n\nRights {}; Wrongs {}; Score {}%",
                nright, nwrong, score
            );
            if nright > 0 && qtime > 0 {
                println!(
                    "Total time {} seconds; {:.1} seconds per problem\n",
                    qtime,
                    qtime as f64 / nright as f64
                );
            }
        }
        println!("Press RETURN to continue...");
        stdout.flush().ok();
        let mut pause = String::new();
        stdin.lock().read_line(&mut pause).ok();
        println!();
    }

    // Print final stats on exit
    let total = nright + nwrong;
    if total > 0 {
        let score = 100 * nright / total;
        println!(
            "\nRights {}; Wrongs {}; Score {}%",
            nright, nwrong, score
        );
    }
}
