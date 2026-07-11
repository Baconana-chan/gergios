// morse — Encode/decode Morse code.
//
// Without flags: encode stdin/args to Morse as "dit"/"daw".
// -s: encode using dots and dashes instead of words.
// -d: decode Morse (dots/dashes) back to text.

use std::io::Read;

static DIGIT: [&str; 10] = [
    "-----", ".----", "..---", "...--", "....-",
    ".....", "-....", "--...", "---..", "----.",
];

static ALPH: [&str; 26] = [
    ".-",   "-...", "-.-.", "-..",  ".",
    "..-.", "--.",  "....", "..",   ".---",
    "-.-",  ".-..", "--",   "-.",   "---",
    ".--.", "--.-", ".-.",  "...",  "-",
    "..-",  "...-", ".--",  "-..-", "-.--",
    "--..",
];

struct Punct {
    c: char,
    morse: &'static str,
}

static OTHER: &[Punct] = &[
    Punct { c: '.', morse: ".-.-.-" },
    Punct { c: ',', morse: "--..--" },
    Punct { c: ':', morse: "---..." },
    Punct { c: '?', morse: "..--.." },
    Punct { c: '\'', morse: ".----." },
    Punct { c: '-', morse: "-....-" },
    Punct { c: '/', morse: "-..-." },
    Punct { c: '(', morse: "-.--." },
    Punct { c: ')', morse: "-.--.-" },
    Punct { c: '"', morse: ".-..-." },
    Punct { c: '=', morse: "-...-" },
    Punct { c: '+', morse: ".-.-." },
];

fn encode_char(c: char) -> Option<&'static str> {
    if c.is_ascii_alphabetic() {
        let idx = c.to_ascii_uppercase() as usize - 'A' as usize;
        Some(ALPH[idx])
    } else if c.is_ascii_digit() {
        let idx = c as usize - '0' as usize;
        Some(DIGIT[idx])
    } else if c.is_ascii_whitespace() {
        Some("")  // word separator
    } else {
        for p in OTHER {
            if p.c == c {
                return Some(p.morse);
            }
        }
        None
    }
}

fn encode(text: &str, symbols: bool) {
    if symbols {
        // Symbols mode: space-separated dots and dashes
        let mut first = true;
        for ch in text.chars() {
            if let Some(code) = encode_char(ch) {
                if !first && !code.is_empty() {
                    print!(" ");
                }
                print!("{}", code);
                first = code.is_empty();
            }
        }
    } else {
        // Human-readable mode: "dit" and "daw" words
        for ch in text.chars() {
            if let Some(code) = encode_char(ch) {
                for c in code.chars() {
                    if c == '.' {
                        print!(" dit");
                    } else if c == '-' {
                        print!(" daw");
                    }
                }
                if code.is_empty() {
                    // Word separator: new paragraph line
                    println!();
                }
            }
        }
        // End of transmission marker
        println!(" ...-.-");
    }
}

fn decode_char(morse: &str) -> Option<char> {
    for (i, &m) in DIGIT.iter().enumerate() {
        if m == morse {
            return Some((b'0' + i as u8) as char);
        }
    }
    for (i, &m) in ALPH.iter().enumerate() {
        if m == morse {
            return Some((b'A' + i as u8) as char);
        }
    }
    for p in OTHER {
        if p.morse == morse {
            return Some(p.c);
        }
    }
    None
}

fn decode_text(text: &str) {
    let mut word = String::new();

    for ch in text.chars() {
        match ch {
            '.' | '-' => word.push(ch),
            ' ' => {
                if !word.is_empty() {
                    if let Some(c) = decode_char(&word) {
                        print!("{}", c);
                    }
                    word.clear();
                }
            }
            '\n' => {
                if !word.is_empty() {
                    if let Some(c) = decode_char(&word) {
                        print!("{}", c);
                    }
                    word.clear();
                }
                println!();
            }
            _ => {} // skip other chars
        }
    }
    if !word.is_empty() {
        if let Some(c) = decode_char(&word) {
            print!("{}", c);
        }
    }
    println!();
}

pub fn run(args: &[String]) {
    let mut dflag = false;
    let mut sflag = false;
    let mut text_args: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-d" => dflag = true,
            "-s" => sflag = true,
            a if a.starts_with('-') => {
                for ch in a.chars().skip(1) {
                    match ch {
                        'd' => dflag = true,
                        's' => sflag = true,
                        _ => {
                            eprintln!("usage: gertoys morse [-ds] [string ...]");
                            std::process::exit(1);
                        }
                    }
                }
            }
            _ => text_args.push(args[i].clone()),
        }
        i += 1;
    }

    if dflag {
        if !text_args.is_empty() {
            for t in &text_args {
                decode_text(t);
            }
        } else {
            let mut input = String::new();
            std::io::stdin().read_to_string(&mut input).unwrap_or(0);
            decode_text(&input);
        }
    } else {
        if !text_args.is_empty() {
            for t in &text_args {
                encode(t, sflag);
            }
        } else {
            let mut input = String::new();
            std::io::stdin().read_to_string(&mut input).unwrap_or(0);
            encode(&input, sflag);
        }
    }
}
