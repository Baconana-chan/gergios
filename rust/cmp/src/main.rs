use std::env;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::process;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut flags: Vec<char> = Vec::new();
    let mut files: Vec<String> = Vec::new();

    // Parse args
    for arg in &args[1..] {
        if arg.starts_with('-') && arg.len() > 1 && !arg.starts_with("--") {
            for c in arg.chars().skip(1) {
                match c {
                    'l' | 's' => flags.push(c),
                    _ => {
                        if c.is_ascii_digit() {
                            // This is probably a skip value argument
                            files.push(arg.clone());
                        } else {
                            eprintln!("cmp: unknown option -- {}", c);
                            process::exit(1);
                        }
                        break;
                    }
                }
            }
        } else if arg == "--" {
            // Everything after is a file
            for f in &args[args.iter().position(|x| x == "--").unwrap() + 1..] {
                files.push(f.clone());
            }
            break;
        } else {
            files.push(arg.clone());
        }
    }

    // Stop processing if we hit -- handling
    // Re-parse properly
    let real_files: Vec<String> = {
        let mut f = Vec::new();
        let mut after_double_dash = false;
        for arg in &args[1..] {
            if arg == "--" {
                after_double_dash = true;
                continue;
            }
            if after_double_dash {
                f.push(arg.clone());
            } else if !arg.starts_with('-') || arg.len() == 1 {
                f.push(arg.clone());
            } else if arg == "-l" || arg == "-s" {
                // flags already parsed
            } else if arg.starts_with('-') && arg.len() > 1 {
                // Could be combined flags or a numeric skip
                let chars: Vec<char> = arg.chars().collect();
                if chars[1].is_ascii_digit() {
                    f.push(arg.clone());
                } else {
                    // combined flags like -ls
                    for c in &chars[1..] {
                        match c {
                            'l' | 's' => {}
                            _ => {
                                eprintln!("cmp: unknown option -- {}", c);
                                process::exit(1);
                            }
                        }
                    }
                }
            } else {
                f.push(arg.clone());
            }
        }
        f
    };

    let real_flags: Vec<char> = {
        let mut f = Vec::new();
        for arg in &args[1..] {
            if arg == "--" {
                break;
            }
            if arg.starts_with('-') && arg.len() > 1 {
                for c in arg.chars().skip(1) {
                    match c {
                        'l' | 's' => {
                            if !f.contains(&c) {
                                f.push(c);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        f
    };

    let silent = real_flags.contains(&'s');
    let verbose = real_flags.contains(&'l');

    match real_files.len() {
        0 => {
            eprintln!("cmp: missing operand");
            process::exit(2);
        }
        1 => {
            // Compare with stdin
            cmp_files("-", &real_files[0], silent, verbose);
        }
        _ => {
            cmp_files(&real_files[0], &real_files[1], silent, verbose);
        }
    }

    Ok(())
}

fn cmp_files(file1: &str, file2: &str, silent: bool, verbose: bool) {
    let read_file = |name: &str| -> io::Result<Vec<u8>> {
        if name == "-" {
            let mut buf = Vec::new();
            io::stdin().read_to_end(&mut buf)?;
            Ok(buf)
        } else {
            let mut file = File::open(Path::new(name))
                .map_err(|e| io::Error::new(io::ErrorKind::NotFound, format!("{}: {}", name, e)))?;
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;
            Ok(buf)
        }
    };

    let data1 = match read_file(file1) {
        Ok(d) => d,
        Err(e) => {
            if !silent {
                eprintln!("cmp: {}", e);
            }
            process::exit(2);
        }
    };

    let data2 = match read_file(file2) {
        Ok(d) => d,
        Err(e) => {
            if !silent {
                eprintln!("cmp: {}", e);
            }
            process::exit(2);
        }
    };

    let min_len = data1.len().min(data2.len());
    for i in 0..min_len {
        if data1[i] != data2[i] {
            if silent {
                process::exit(1);
            } else if verbose {
                println!("{} {:3o} {:3o}", i + 1, data1[i], data2[i]);
            } else {
                // Default output: "FILE1 FILE2 differ: byte LINE, column COL"
                // Calculate line number
                let mut byte_pos = i;
                let mut line_num = 1;
                let mut col_num = 1;
                // Count newlines up to position i
                for j in 0..=i {
                    if data1[j] == b'\n' && j < i {
                        line_num += 1;
                        col_num = 1;
                    } else if j < i {
                        col_num += 1;
                    }
                }
                println!(
                    "{} {} differ: byte {}, line {}",
                    file1_display(file1),
                    file2_display(file2),
                    byte_pos + 1,
                    line_num
                );
            }
            process::exit(1);
        }
    }

    if data1.len() != data2.len() {
        if silent {
            process::exit(1);
        } else {
            // EOF on shorter file
            let shorter = if data1.len() < data2.len() { file1 } else { file2 };
            println!("cmp: EOF on {}", shorter);
            process::exit(1);
        }
    }

    // Files are identical
    process::exit(0);
}

fn file1_display(name: &str) -> &str {
    if name == "-" { "stdin" } else { name }
}

fn file2_display(name: &str) -> &str {
    if name == "-" { "stdin" } else { name }
}
