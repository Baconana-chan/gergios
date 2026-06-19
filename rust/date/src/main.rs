use std::env;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

/// Month names
const MONTHS: &[&str] = &["Jan", "Feb", "Mar", "Apr", "May", "Jun",
                          "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// Weekday names
const WEEKDAYS: &[&str] = &["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/// Days in month (non-leap)
const DAYS_IN_MONTH: &[u64] = &[31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

fn is_leap(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Convert Unix timestamp to (year, month, day, hour, min, sec, weekday)
fn unix_to_ymd(ts: u64) -> (u64, u64, u64, u64, u64, u64, u64) {
    // Start from 1970-01-01 (Thursday = 4)
    let mut days = ts / 86400;
    let time_sec = ts % 86400;
    let hour = time_sec / 3600;
    let min = (time_sec % 3600) / 60;
    let sec = time_sec % 60;

    // Weekday: 1970-01-01 was Thursday (4 in 0=Mon..6=Sun)
    let weekday = (days + 3) % 7;

    let mut year: u64 = 1970;
    loop {
        let year_days = if is_leap(year) { 366 } else { 365 };
        if days < year_days {
            break;
        }
        days -= year_days;
        year += 1;
    }

    let mut month: u64 = 0;
    for (i, &dim) in DAYS_IN_MONTH.iter().enumerate() {
        let dim_adj = if i == 1 && is_leap(year) { 29 } else { dim };
        if days < dim_adj {
            month = i as u64;
            break;
        }
        days -= dim_adj;
    }
    let day = days + 1; // 1-indexed

    (year, month, day, hour, min, sec, weekday)
}

/// Format according to strftime-like format string
fn format_date(fmt: &str, ts: u64) -> String {
    let (year, month, day, hour, min, sec, weekday) = unix_to_ymd(ts);
    let mut result = String::new();
    let mut chars = fmt.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            match chars.next() {
                Some('a') => result.push_str(WEEKDAYS[weekday as usize]),
                Some('b') => result.push_str(MONTHS[month as usize]),
                Some('c') => {
                    // Locale's date and time - simplified
                    result.push_str(&format!(
                        "{} {} {:02} {:02}:{:02}:{:02} {}",
                        WEEKDAYS[weekday as usize],
                        MONTHS[month as usize],
                        day, hour, min, sec, year
                    ));
                }
                Some('d') => result.push_str(&format!("{:02}", day)),
                Some('H') => result.push_str(&format!("{:02}", hour)),
                Some('I') => {
                    let h12 = if hour == 0 { 12 } else if hour > 12 { hour - 12 } else { hour };
                    result.push_str(&format!("{:02}", h12));
                }
                Some('j') => {
                    // Day of year
                    let mut doy = 0u64;
                    for m in 0..month as usize {
                        let dim = if m == 1 && is_leap(year) { 29 } else { DAYS_IN_MONTH[m] };
                        doy += dim;
                    }
                    doy += day;
                    result.push_str(&format!("{:03}", doy));
                }
                Some('m') => result.push_str(&format!("{:02}", month + 1)),
                Some('M') => result.push_str(&format!("{:02}", min)),
                Some('p') => result.push_str(if hour < 12 { "AM" } else { "PM" }),
                Some('S') => result.push_str(&format!("{:02}", sec)),
                Some('T') => result.push_str(&format!("{:02}:{:02}:{:02}", hour, min, sec)),
                Some('u') => result.push_str(&format!("{}", if weekday == 0 { 7 } else { weekday })),
                Some('w') => result.push_str(&format!("{}", weekday)),
                Some('y') => result.push_str(&format!("{:02}", year % 100)),
                Some('Y') => result.push_str(&format!("{}", year)),
                Some('D') => result.push_str(&format!("{:02}/{:02}/{:02}", month + 1, day, year % 100)),
                Some('s') => result.push_str(&format!("{}", ts)),
                Some('%') => result.push('%'),
                Some(c) => {
                    result.push('%');
                    result.push(c);
                }
                None => result.push('%'),
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Default date format: "%a %b %e %T %Z %Y" -> "Mon Jan  2 14:05:34 UTC 2024"
fn default_format(ts: u64) -> String {
    let (year, month, day, hour, min, sec, weekday) = unix_to_ymd(ts);
    format!(
        "{} {} {:2} {:02}:{:02}:{:02} UTC {}",
        WEEKDAYS[weekday as usize],
        MONTHS[month as usize],
        day, hour, min, sec, year
    )
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut utc = false;
    let mut format_str: Option<String> = None;
    let mut rest_args: Vec<String> = Vec::new();
    let mut after_double_dash = false;

    for arg in &args[1..] {
        if arg == "--" {
            after_double_dash = true;
            continue;
        }
        if !after_double_dash && arg.starts_with('-') && arg.len() > 1 {
            let chars: Vec<char> = arg.chars().collect();
            if chars[1] == '+' {
                format_str = Some(arg[2..].to_string());
            } else if arg == "-u" || arg == "--utc" || arg == "--universal" {
                utc = true;
            } else if arg == "-R" {
                // RFC 2822 format
                format_str = Some("%a, %d %b %Y %H:%M:%S %z".to_string());
            } else if arg == "-r" {
                // Last modified time of file - handle next arg
                // skip, handled below
            } else {
                eprintln!("date: unknown option -- {}", chars[1]);
                process::exit(1);
            }
        } else if arg == "-r" {
            // -r needs a file argument, ignore for now
            // Actually handle it properly
            continue;
        } else {
            rest_args.push(arg.clone());
        }
    }

    // Get current time
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let ts = now;

    // Check for -r flag (last modified time of file)
    let mut ref_time = ts;
    let mut use_ref_file = false;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "-r" && i + 1 < args.len() {
            let fname = &args[i + 1];
            if let Ok(meta) = std::fs::metadata(fname) {
                if let Ok(modified) = meta.modified() {
                    if let Ok(d) = modified.duration_since(UNIX_EPOCH) {
                        ref_time = d.as_secs();
                        use_ref_file = true;
                    }
                }
            } else {
                eprintln!("date: {}: No such file or directory", fname);
                process::exit(1);
            }
            i += 2;
        } else {
            i += 1;
        }
    }

    let ts = if use_ref_file { ref_time } else { ts };

    if let Some(ref fmt) = format_str {
        println!("{}", format_date(fmt, ts));
    } else {
        println!("{}", default_format(ts));
    }
}
