//! Rust port of the MINIX/NetBSD `cal` utility.
//!
//! Usage:
//!   cal [-m] [-y] [[month] year]
//!
//! Displays a calendar. -m: Monday as first day of week.
//! -y: display calendar for current year.

use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

const MONTHS: [&str; 12] = [
    "January", "February", "March", "April", "May", "June",
    "July", "August", "September", "October", "November", "December",
];

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i64, month: usize) -> usize {
    match month {
        1 => if is_leap(year) { 29 } else { 28 },
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Zeller's congruence to get day of week (0=Sunday, 1=Monday, ..., 6=Saturday)
fn day_of_week(year: i64, month: usize, day: usize) -> usize {
    let m = if month < 3 { month + 12 } else { month };
    let y = if month < 3 { year - 1 } else { year };
    let y_mod = (y % 100) as usize;
    let y_cent = (y / 100) as usize;
    let dow = (day + (13 * (m + 1)) / 5 + y_mod + y_mod / 4 + y_cent / 4 + 5 * y_cent) % 7;
    (dow + 6) % 7 // 0=Sunday → 0=Sunday, 1=Monday etc for Zeller
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut monday_first = false;
    let mut show_year = false;

    while !argv.is_empty() && argv[0].starts_with('-') {
        let opt = argv[0].clone();
        for ch in opt.chars().skip(1) {
            match ch {
                'm' => monday_first = true,
                'y' => show_year = true,
                _ => { eprintln!("usage: cal [-m] [-y] [[month] year]"); std::process::exit(1); }
            }
        }
        argv = &argv[1..];
    }

    // Get current year/month
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let days_since = (now.as_secs() / 86400) as i64;
    let mut y = 1970i64;
    let mut remaining = days_since;
    loop {
        let diy = if is_leap(y) { 366 } else { 365 };
        if remaining < diy { break; }
        remaining -= diy;
        y += 1;
    }
    let cyear = y;
    let months_days = [31, if is_leap(cyear) { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut cmonth = 1usize;
    let mut cd = remaining + 1;
    for &md in &months_days {
        if cd as usize <= md { break; }
        cd -= md as i64;
        cmonth += 1;
    }

    let (month, year) = match argv.len() {
        0 => (cmonth, cyear),
        1 => {
            let val: i64 = argv[0].parse().unwrap_or_else(|_| { eprintln!("cal: invalid argument"); std::process::exit(1); });
            if val >= 1 && val <= 12 { (val as usize, cyear) }
            else { (cmonth, val) }
        }
        2 => {
            let m: usize = argv[0].parse().unwrap_or_else(|_| { eprintln!("cal: invalid month"); std::process::exit(1); });
            let y: i64 = argv[1].parse().unwrap_or_else(|_| { eprintln!("cal: invalid year"); std::process::exit(1); });
            if m < 1 || m > 12 { eprintln!("cal: bad month"); std::process::exit(1); }
            (m, y)
        }
        _ => { eprintln!("usage: cal [-m] [-y] [[month] year]"); std::process::exit(1); }
    };

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    if show_year {
        // Print entire year
        for y_chunk in (1..=12).step_by(3) {
            // Month headers (3 across)
            for off in 0..3 {
                if y_chunk + off <= 12 {
                    let m_name = MONTHS[y_chunk + off - 1];
                    write!(out, "        {}          ", m_name).ok();
                }
            }
            writeln!(out).ok();
            // Day headers
            for _ in 0..3 {
                if monday_first {
                    write!(out, "Mo Tu We Th Fr Sa Su  ").ok();
                } else {
                    write!(out, "Su Mo Tu We Th Fr Sa  ").ok();
                }
            }
            writeln!(out).ok();

            // Calendar grid for 3 months
            let mut days_grid: [Vec<Option<usize>>; 3] = Default::default();
            for off in 0..3 {
                if y_chunk + off <= 12 {
                    let m = y_chunk + off;
                    let dim = days_in_month(year, m);
                    let first_dow = day_of_week(year, m, 1);
                    let mut v = Vec::new();
                    for _ in 0..first_dow { v.push(None); }
                    for d in 1..=dim { v.push(Some(d)); }
                    days_grid[off] = v;
                }
            }

            let max_rows = days_grid.iter().map(|g| g.len()).max().unwrap_or(0);
            let mut row = 0;
            while row < max_rows {
                for off in 0..3 {
                    let g = &days_grid[off];
                    for col in 0..7 {
                        let idx = row * 7 + col;
                        if idx < g.len() {
                            if let Some(d) = g[idx] {
                                write!(out, "{:>2} ", d).ok();
                            } else {
                                write!(out, "   ").ok();
                            }
                        } else {
                            write!(out, "   ").ok();
                        }
                    }
                    write!(out, " ").ok();
                }
                writeln!(out).ok();
                row += 1;
            }
        }
    } else {
        // Single month
        let dim = days_in_month(year, month);
        let first_dow = day_of_week(year, month, 1);

        let header = format!("{} {}", MONTHS[month - 1], year);
        let pad = (20 - header.len()) / 2;
        writeln!(out, "{:>width$}{}", "", header, width = pad).ok();
        if monday_first {
            writeln!(out, "Mo Tu We Th Fr Sa Su").ok();
        } else {
            writeln!(out, "Su Mo Tu We Th Fr Sa").ok();
        }

        let mut day = 1;
        for row in 0..6 {
            if day > dim { break; }
            for col in 0..7 {
                if row == 0 && col < first_dow {
                    write!(out, "   ").ok();
                } else if day <= dim {
                    write!(out, "{:>2} ", day).ok();
                    day += 1;
                }
            }
            writeln!(out).ok();
        }
    }
}
