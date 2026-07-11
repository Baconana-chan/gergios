// pig — Convert text to Pig Latin.
//
// Reads from stdin, writes Pig Latin to stdout.
// Handles: vowel-start → way, consonant-start → move to end + ay,
// "qu" treated as a unit, preserves case.

fn pig_out(buf: &[u8]) {
    if buf.is_empty() {
        return;
    }

    let len = buf.len();
    let all_upper = buf.iter().all(|&b| b.is_ascii_uppercase());
    let first_upper = buf[0].is_ascii_uppercase();

    // Vowel check
    let vowels = b"aeiouAEIOU";
    if vowels.contains(&buf[0]) {
        let suffix = if all_upper { "WAY" } else { "way" };
        print!("{}{}", std::str::from_utf8(buf).unwrap_or(""), suffix);
        return;
    }

    // Copy leading consonants to end
    let mut word = buf.to_vec();
    let mut start = 0usize;
    let consonant_vowels = b"aeiouyAEIOUY";

    while start < len && !consonant_vowels.contains(&word[start]) {
        let ch = word[start];
        word.push(ch);
        start += 1;
        // Handle "qu" unit
        if (ch == b'q' || ch == b'Q') && start < len
            && (word[start] == b'u' || word[start] == b'U')
        {
            let u = word[start];
            word.push(u);
            start += 1;
        }
    }

    if first_upper && start < word.len() {
        word[start] = word[start].to_ascii_uppercase();
    }

    let suffix = if all_upper { "AY" } else { "ay" };
    print!("{}{}",
        std::str::from_utf8(&word[start..start + len]).unwrap_or(""),
        suffix);
}

pub fn run(args: &[String]) {
    // No options expected (original pig has none)
    if !args.is_empty() && args.iter().any(|a| a.starts_with('-')) {
        eprintln!("usage: gertoys pig");
        std::process::exit(1);
    }

    use std::io::Read;
    let mut buf = [0u8; 1024];
    let mut word = Vec::new();
    let mut stdin = std::io::stdin();

    loop {
        let n = stdin.read(&mut buf).unwrap_or(0);
        if n == 0 {
            if !word.is_empty() {
                pig_out(&word);
            }
            break;
        }
        for &b in &buf[..n] {
            if b.is_ascii_alphabetic() {
                word.push(b);
            } else {
                if !word.is_empty() {
                    pig_out(&word);
                    word.clear();
                }
                print!("{}", b as char);
            }
        }
    }
}
