//! Rust port of the MINIX/NetBSD `calendar` utility.
//!
//! Usage:
//!   calendar [-a] [-t days] [-f calendarfile]
//!
//! Displays upcoming events from calendar files.

use std::fs;
use std::io::{BufRead, BufReader};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];
    let mut days_ahead: i32 = 0;
    let mut cal_file = "calendar".to_string();
    let mut all_users = false;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        if opt.starts_with("-t") {
            if opt.len() > 2 {
                days_ahead = opt[2..].parse().unwrap_or_else(|_| { eprintln!("usage: calendar [-a] [-t days] [-f file]"); std::process::exit(1); });
            } else {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("usage: calendar [-a] [-t days] [-f file]"); std::process::exit(1); }
                days_ahead = argv[0].parse().unwrap_or_else(|_| { eprintln!("calendar: invalid days"); std::process::exit(1); });
            }
        } else if opt == "-f" && opt.len() == 2 {
            argv = &argv[1..];
            if argv.is_empty() { eprintln!("usage: calendar [-a] [-t days] [-f file]"); std::process::exit(1); }
            cal_file = argv[0].clone();
        } else if opt == "-a" {
            all_users = true;
        } else {
            eprintln!("usage: calendar [-a] [-t days] [-f file]");
            std::process::exit(1);
        }
        argv = &argv[1..];
    }

    let (today_month, today_day) = today_date();

    if !all_users {
        check_calendar(&cal_file, today_month, today_day, days_ahead);
    }

    if all_users {
        // Try home directories
        for dir in &["/home", "/Users"] {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries {
                    if let Ok(entry) = entry {
                        let user_cal = entry.path().join(".calendar");
                        if user_cal.exists() {
                            print!("{}: ", entry.file_name().to_string_lossy());
                            check_calendar(user_cal.to_str().unwrap_or(""), today_month, today_day, days_ahead);
                        }
                    }
                }
            }
        }
    }
}

fn today_date() -> (u32, u32) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = now / 86400;
    let mut y = 1970i64;
    let mut remaining = days;
    loop {
        let ydays = if is_leap(y) { 366 } else { 365 };
        if remaining < ydays { break; }
        remaining -= ydays;
        y += 1;
    }
    let month_days = if is_leap(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 0;
    while m < 12 && remaining >= month_days[m] {
        remaining -= month_days[m];
        m += 1;
    }
    (m as u32 + 1, remaining as u32 + 1)
}

fn is_leap(y: i64) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

fn check_calendar(path: &str, month: u32, day: u32, ahead: i32) {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return,
    };
    let reader = BufReader::new(file);

    for line in reader.lines() {
        if let Ok(line) = line {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
                continue;
            }
            if let Some((date_part, event)) = trimmed.split_once('\t').or_else(|| trimmed.split_once("  ")) {
                if let Some((ev_month, ev_day)) = parse_date(date_part.trim()) {
                    if is_within(ev_month, ev_day, month, day, ahead) {
                        println!("{}\t{}", date_part.trim(), event.trim());
                    }
                }
            }
        }
    }
}

fn parse_date(s: &str) -> Option<(u32, u32)> {
    let months = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() < 2 { return None; }
    let mon_str = parts[0].to_lowercase();
    let day_str = parts[1].trim_end_matches(',').trim_end_matches('.');
    let month = months.iter().position(|&m| m.starts_with(&mon_str[..mon_str.len().min(3)]))? as u32 + 1;
    let day: u32 = day_str.parse().ok()?;
    Some((month, day))
}

fn is_within(ev_month: u32, ev_day: u32, cur_month: u32, cur_day: u32, ahead: i32) -> bool {
    if ahead >= 0 {
        let ev_days = ev_month * 100 + ev_day;
        let cur_days = cur_month * 100 + cur_day;
        ev_days >= cur_days && ev_days <= cur_days + ahead as u32
    } else {
        ev_month == cur_month && ev_day == cur_day
    }
}
