// number — Convert numbers to English words.
//
// Supports up to 65 digits (vigintillions) and decimal fractions.
// -l flag: line mode (compact output, one line per number).

const MAXDIGITS: usize = 65;

static NAME1: [&str; 20] = [
    "", "one", "two", "three", "four", "five", "six", "seven",
    "eight", "nine", "ten", "eleven", "twelve", "thirteen",
    "fourteen", "fifteen", "sixteen", "seventeen", "eighteen", "nineteen",
];

static NAME2: [&str; 10] = [
    "", "ten", "twenty", "thirty", "forty", "fifty",
    "sixty", "seventy", "eighty", "ninety",
];

static NAME3: [&str; 22] = [
    "hundred", "thousand", "million", "billion", "trillion",
    "quadrillion", "quintillion", "sextillion", "septillion",
    "octillion", "nonillion", "decillion", "undecillion",
    "duodecillion", "tredecillion", "quattuordecillion",
    "quindecillion", "sexdecillion", "septendecillion",
    "octodecillion", "novemdecillion", "vigintillion",
];

/// Process a group of 1-3 digits and print the English words.
/// Returns true if any non-zero digit was printed.
fn number_group(p: &[u8], len: usize) -> bool {
    let mut rval = false;

    // Handle hundreds
    if len >= 3 && p[0] != b'0' {
        rval = true;
        print!("{} hundred", NAME1[(p[0] - b'0') as usize]);
    }

    // Handle tens and ones (the tens_start index handles the fallthrough
    // from case 3 → case 2 that the original C uses via switch fallthrough)
    let tens_start = if len >= 3 { 1 } else { 0 };
    let remaining = len - tens_start;

    if remaining > 0 {
        let val = if remaining == 2 {
            (p[tens_start] - b'0') as usize * 10 + (p[tens_start + 1] - b'0') as usize
        } else {
            (p[tens_start] - b'0') as usize
        };

        if val > 0 {
            if rval {
                print!(" ");
            }
            if val < 20 {
                print!("{}", NAME1[val]);
            } else {
                print!("{}", NAME2[val / 10]);
                if val % 10 > 0 {
                    print!("-{}", NAME1[val % 10]);
                }
            }
            rval = true;
        }
    }

    rval
}

fn convert_number(line: &str, lflag: bool) {
    let line = line.trim();
    if line.is_empty() {
        return;
    }

    let (neg, rest) = if line.starts_with('-') {
        (true, &line[1..])
    } else {
        (false, line)
    };

    let (int_part, frac_part) = if let Some(pos) = rest.find('.') {
        (&rest[..pos], &rest[pos + 1..])
    } else {
        (rest, "")
    };

    // Validate: only digits allowed
    if !int_part.chars().all(|c| c.is_ascii_digit())
        || (!frac_part.is_empty() && !frac_part.chars().all(|c| c.is_ascii_digit()))
    {
        eprintln!("number: illegal number: {}", line);
        std::process::exit(1);
    }

    if int_part.len() > MAXDIGITS || frac_part.len() > MAXDIGITS {
        eprintln!("number: number too large, max {} digits", MAXDIGITS);
        std::process::exit(1);
    }

    if neg {
        print!("minus{}", if lflag { " " } else { "\n" });
    }

    let bytes = int_part.as_bytes();
    let len = bytes.len();
    let mut rval = false;

    if len > 3 {
        let mut remaining = len;
        let mut offset = 0;

        if len % 3 != 0 {
            let chunk = len % 3;
            remaining -= chunk;
            if number_group(&bytes[offset..offset + chunk], chunk) {
                rval = true;
                let idx = remaining / 3;
                print!(" {}{}", NAME3[idx], if lflag { " " } else { "\n" });
            }
            offset += chunk;
        }
        while remaining > 3 {
            remaining -= 3;
            if number_group(&bytes[offset..offset + 3], 3) {
                rval = true;
                let idx = remaining / 3;
                print!(" {}{}", NAME3[idx], if lflag { " " } else { "\n" });
            }
            offset += 3;
        }
    }
    if number_group(&bytes[len - (len.min(3))..], len.min(3)) {
        if !lflag {
            println!(".");
        }
        rval = true;
    }

    // Handle fractional part
    if !frac_part.is_empty() && frac_part.bytes().any(|b| b != b'0') {
        if rval {
            print!("{}and{}", if lflag { " " } else { "" }, if lflag { " " } else { "\n" });
        }
        rval = true;
        let flen = frac_part.len();
        if number_group(frac_part.as_bytes(), flen) {
            if lflag {
                print!(" ");
            }
            match flen {
                1 => println!("tenths."),
                2 => println!("hundredths."),
                _ => {
                    let pref = match flen % 3 {
                        1 => "ten-",
                        2 => "hundred-",
                        _ => "",
                    };
                    println!("{}{}ths.", pref, NAME3[flen / 3]);
                }
            }
        }
    }

    if !rval {
        print!("zero{}", if lflag { "" } else { ".\n" });
    }
    if lflag {
        println!();
    }
}

pub fn run(args: &[String]) {
    let mut lflag = false;
    let mut number_args: Vec<&str> = Vec::new();

    for arg in args {
        if arg == "-l" {
            lflag = true;
        } else {
            number_args.push(arg.as_str());
        }
    }

    if number_args.is_empty() {
        let mut first = true;
        let mut line = String::new();
        loop {
            line.clear();
            let n = std::io::stdin().read_line(&mut line).unwrap_or(0);
            if n == 0 {
                break;
            }
            let trimmed = line.trim_end_matches('\n').trim_end_matches('\r');
            if trimmed.len() > 255 {
                eprintln!("number: line too long.");
                std::process::exit(1);
            }
            if !first {
                println!("...");
            }
            convert_number(trimmed, lflag);
            first = false;
        }
    } else {
        let mut first = true;
        for n in &number_args {
            if !first {
                println!("...");
            }
            convert_number(n, lflag);
            first = false;
        }
    }
}
