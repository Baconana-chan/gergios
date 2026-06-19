use std::env;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

/// POSIX CRC32 polynomial
const CRC32_POLY: u32 = 0xedb88320;

/// Generate CRC32 lookup table
fn crc32_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    for i in 0..256u32 {
        let mut crc = i;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = CRC32_POLY ^ (crc >> 1);
            } else {
                crc >>= 1;
            }
        }
        table[i as usize] = crc;
    }
    table
}

/// Compute POSIX cksum CRC32 for given bytes.
/// Returns (crc, total_bytes).
fn cksum_bytes(data: &[u8]) -> (u32, usize) {
    let table = crc32_table();
    let mut crc = 0xffffffffu32;
    for &b in data {
        let idx = ((crc ^ (b as u32)) & 0xff) as usize;
        crc = table[idx] ^ (crc >> 8);
    }
    let crc = crc ^ 0xffffffff;
    (crc, data.len())
}

/// Format cksum output: "CRC BYTES FILENAME"
fn print_cksum(crc: u32, bytes: usize, filename: Option<&str>) {
    match filename {
        Some(name) => println!("{} {} {}", crc, bytes, name),
        None => println!("{} {}", crc, bytes),
    }
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let files: Vec<&str> = if args.len() > 1 {
        args[1..].iter().map(|s| s.as_str()).collect()
    } else {
        vec!["-"]
    };

    let mut had_error = false;

    for &filename in &files {
        if filename == "-" {
            let mut data = Vec::new();
            io::stdin().read_to_end(&mut data)?;
            let (crc, bytes) = cksum_bytes(&data);
            print_cksum(crc, bytes, None);
        } else {
            let path = Path::new(filename);
            match File::open(path) {
                Ok(mut file) => {
                    let mut data = Vec::new();
                    if let Err(e) = file.read_to_end(&mut data) {
                        eprintln!("cksum: {}: {}", filename, e);
                        had_error = true;
                        continue;
                    }
                    let (crc, bytes) = cksum_bytes(&data);
                    print_cksum(crc, bytes, Some(filename));
                }
                Err(e) => {
                    eprintln!("cksum: {}: {}", filename, e);
                    had_error = true;
                }
            }
        }
    }

    if had_error {
        std::process::exit(1);
    }
    Ok(())
}
