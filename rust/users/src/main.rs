//! Rust port of the MINIX/NetBSD `users` utility.
//!
//! Usage:
//!   users [file]
//!
//! Lists logged-in users from utmp file.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let utmp_file = if args.len() > 1 { &args[1] } else { "/var/run/utmp" };

    #[cfg(unix)]
    {
        use std::fs::File;
        use std::io::BufRead;

        match File::open(utmp_file) {
            Ok(f) => {
                use std::io::BufReader;
                let reader = BufReader::new(f);
                let mut users_found: Vec<String> = Vec::new();
                for line in reader.split(b'\n') {
                    if let Ok(chunk) = line {
                        if chunk.len() >= 32 {
                            let end = chunk.iter().position(|&b| b == 0).unwrap_or(chunk.len());
                            let name = String::from_utf8_lossy(&chunk[..end]).to_string();
                            if !name.is_empty() && name != "LOGIN" && name != "shutdown" {
                                if !users_found.contains(&name) {
                                    users_found.push(name);
                                }
                            }
                        }
                    }
                }
                println!("{}", users_found.join(" "));
            }
            Err(_) => {}
        }
    }

    #[cfg(not(unix))]
    {
        let _ = utmp_file;
        eprintln!("users: not supported on this platform");
        std::process::exit(1);
    }
}
