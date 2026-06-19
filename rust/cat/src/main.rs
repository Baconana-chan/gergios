//! Rust port of the MINIX/NetBSD `cat` utility.
//!
//! Usage:
//!   cat [-benstuv] [file ...]
//!
//! Concatenates files and prints them to standard output.
//! If no file or `-` is given, reads from standard input.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut bflag = false; // -b: number non-blank lines
    let mut eflag = false; // -e: show $ at end of lines
    let mut nflag = false; // -n: number all lines
    let mut sflag = false; // -s: suppress repeated empty lines
    let mut tflag = false; // -t: show ^I for tabs
    let mut uflag = false; // -u: unbuffered (ignored in Rust)

    // Parse options (POSIX-style)
    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" && argv[0].len() > 1 {
        for c in argv[0].chars().skip(1) {
            match c {
                'b' => bflag = true,
                'e' => eflag = true,
                'n' => nflag = true,
                's' => sflag = true,
                't' => tflag = true,
                'u' => uflag = true,
                '-' => { argv = &argv[1..]; break; }
                _ => {
                    eprintln!("cat: unknown option -- {c}");
                    eprintln!("usage: cat [-benstuv] [file ...]");
                    std::process::exit(1);
                }
            }
        }
        argv = &argv[1..];
    }

    // -b implies -n
    if bflag {
        nflag = true;
    }

    let files = if argv.is_empty() {
        vec!["-"]
    } else {
        argv.iter().map(|s| s.as_str()).collect()
    };

    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut line_num: u64 = 0;
    let mut prev_blank = false;
    let mut had_error = false;

    for filename in files {
        let result: io::Result<()> = if filename == "-" {
            // Read from stdin
            if bflag || eflag || nflag || sflag || tflag {
                process_lines(BufReader::new(io::stdin()), &mut out, &mut line_num, &mut prev_blank, bflag, eflag, nflag, sflag, tflag)
            } else {
                let mut stdin = io::stdin().lock();
                io::copy(&mut stdin, &mut out)?;
                Ok(())
            }
        } else {
            let path = Path::new(filename);
            match File::open(path) {
                Ok(file) => {
                    if bflag || eflag || nflag || sflag || tflag {
                        process_lines(BufReader::new(file), &mut out, &mut line_num, &mut prev_blank, bflag, eflag, nflag, sflag, tflag)
                    } else {
                        let mut file = file;
                        io::copy(&mut file, &mut out)?;
                        Ok(())
                    }
                }
                Err(e) => {
                    eprintln!("cat: {}: {e}", filename);
                    had_error = true;
                    Ok(())
                }
            }
        };
        if let Err(e) = result {
            eprintln!("cat: {filename}: {e}");
            had_error = true;
        }
    }

    if had_error {
        std::process::exit(1);
    }
}

fn process_lines<R: Read>(
    reader: BufReader<R>,
    out: &mut io::StdoutLock<'_>,
    line_num: &mut u64,
    prev_blank: &mut bool,
    bflag: bool,
    eflag: bool,
    nflag: bool,
    sflag: bool,
    tflag: bool,
) -> io::Result<()> {
    for line_result in reader.lines() {
        let line = line_result?;
        let is_blank = line.is_empty();

        // -s: suppress repeated empty lines
        if sflag && is_blank && *prev_blank {
            continue;
        }
        *prev_blank = is_blank;

        // -b: number only non-blank lines
        if nflag && (!bflag || !is_blank) {
            *line_num += 1;
            write!(out, "{:>6}\t", *line_num)?;
        }

        // Process characters for -e and -t flags
        if eflag || tflag {
            for c in line.chars() {
                match c {
                    '\t' if tflag => write!(out, "^I")?,
                    c => write!(out, "{c}")?,
                }
            }
            if eflag {
                writeln!(out, "$")?;
            } else {
                writeln!(out)?;
            }
        } else {
            writeln!(out, "{line}")?;
        }
    }
    Ok(())
}
