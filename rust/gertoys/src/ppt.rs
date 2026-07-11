// ppt — Paper tape encoder/decoder.
//
// Without -d: encode text to visual paper tape format (8-bit rows with feed hole).
// With -d: decode paper tape format back to text.

use std::io::Read;

const EDGE: &str = "___________";

fn put_ppt(c: u8) {
    print!("|");
    for i in (0..8).rev() {
        if i == 2 {
            print!("."); // feed hole
        }
        if (c >> i) & 1 != 0 {
            print!("o");
        } else {
            print!(" ");
        }
    }
    println!("|");
}

fn get_ppt(buf: &str) -> Option<u8> {
    let bytes = buf.as_bytes();
    let p = buf.find('.')?;
    let len = bytes.len();

    let mut c = 0u8;
    // Bits 7-3: columns left of the feed hole ('.')
    // p[-5] → bit 7 (MSB), p[-4] → bit 6, ..., p[-1] → bit 3
    if p >= 5 && bytes[p - 5] != b' ' { c |= 0x80; }
    if p >= 4 && bytes[p - 4] != b' ' { c |= 0x40; }
    if p >= 3 && bytes[p - 3] != b' ' { c |= 0x20; }
    if p >= 2 && bytes[p - 2] != b' ' { c |= 0x10; }
    if p >= 1 && bytes[p - 1] != b' ' { c |= 0x08; }
    // Bits 2-0: columns right of the feed hole
    if p + 1 < len && bytes[p + 1] != b' ' { c |= 0x04; }
    if p + 2 < len && bytes[p + 2] != b' ' { c |= 0x02; }
    if p + 3 < len && bytes[p + 3] != b' ' { c |= 0x01; }

    Some(c)
}

pub fn run(args: &[String]) {
    let mut dflag = false;
    let mut text_args: Vec<&str> = Vec::new();

    for arg in args {
        match arg.as_str() {
            "-d" => dflag = true,
            a if a.starts_with('-') && a != "-d" => {
                eprintln!("usage: gertoys ppt [-d] [string ...]");
                std::process::exit(1);
            }
            _ => text_args.push(arg.as_str()),
        }
    }

    if dflag {
        if !text_args.is_empty() {
            eprintln!("usage: gertoys ppt [-d] [string ...]");
            std::process::exit(1);
        }

        let mut start = false;
        let mut line = String::new();
        loop {
            line.clear();
            let n = std::io::stdin().read_line(&mut line).unwrap_or(0);
            if n == 0 {
                break;
            }
            let trimmed = line.trim_end_matches('\n').trim_end_matches('\r');
            if let Some(c) = get_ppt(trimmed) {
                start = true;
                print!("{}", c as char);
            } else if start {
                // Lost sync
                println!();
                return;
            }
        }
        println!();
    } else {
        println!("{}", EDGE);
        if !text_args.is_empty() {
            for (i, arg) in text_args.iter().enumerate() {
                if i > 0 {
                    put_ppt(b' ');
                }
                for &b in arg.as_bytes() {
                    put_ppt(b);
                }
            }
        } else {
            let mut buf = [0u8; 1];
            let mut stdin = std::io::stdin();
            loop {
                let n = stdin.read(&mut buf).unwrap_or(0);
                if n == 0 {
                    break;
                }
                put_ppt(buf[0]);
            }
        }
        println!("{}", EDGE);
    }
}
