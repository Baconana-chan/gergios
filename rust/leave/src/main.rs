//! Rust port of the MINIX/NetBSD `leave` utility.
//!
//! Usage:
//!   leave [[+]hhmm]
//!
//! Reminds you when you have to leave.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let argv = &args[1..];

    let when = if argv.is_empty() {
        interactive_prompt()
    } else {
        parse_time(argv[0].as_str())
    };

    let (target_h, target_m) = match when {
        Some(t) => t,
        None => { eprintln!("leave: invalid time"); std::process::exit(1); }
    };

    println!("leave: reminder set for {:02}:{:02}", target_h, target_m);

    loop {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let now_h = (now / 3600) % 24;
        let now_m = (now / 60) % 60;

        if now_h > target_h || (now_h == target_h && now_m >= target_m) {
            println!("\x07Time to leave!");
            for _ in 0..5 {
                println!("\x07");
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
            break;
        }

        std::thread::sleep(std::time::Duration::from_secs(30));
    }
}

fn interactive_prompt() -> Option<(u64, u64)> {
    let mut input = String::new();
    print!("When do you have to leave? ");
    std::io::Write::flush(&mut std::io::stdout()).ok();
    if std::io::stdin().read_line(&mut input).ok()? == 0 { return None; }
    parse_time(input.trim())
}

fn parse_time(s: &str) -> Option<(u64, u64)> {
    let s = s.trim();
    if s.is_empty() { return None; }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let now_h = (now / 3600) % 24;
    let now_m = (now / 60) % 60;

    if s.starts_with('+') {
        // Relative time: +minutes
        let mins: u64 = s[1..].parse().ok()?;
        let total = now_h * 60 + now_m + mins;
        Some((total / 60 % 24, total % 60))
    } else {
        // Absolute time: hhmm
        let clean: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
        if clean.len() < 3 || clean.len() > 4 { return None; }
        let val: u64 = clean.parse().ok()?;
        let (h, m) = if clean.len() == 4 {
            (val / 100, val % 100)
        } else {
            (val / 100, val % 100)
        };
        if h > 23 || m > 59 { return None; }
        Some((h, m))
    }
}
