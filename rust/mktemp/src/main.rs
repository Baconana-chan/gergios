//! Rust port of the MINIX/NetBSD `mktemp` utility.
//!
//! Usage:
//!   mktemp [-d] [-p dir] [-q] [-t prefix] [-u] template ...
//!
//! Creates temporary files or directories safely.

use std::env;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut is_dir = false;
    let mut dry_run = false;
    let mut quiet = false;
    let mut tmpdir = env::temp_dir();
    let mut prefix: Option<String> = None;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        match opt.as_str() {
            "-d" => is_dir = true,
            "-u" => dry_run = true,
            "-q" => quiet = true,
            "-p" => {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("usage: mktemp [-d] [-p dir] [-q] [-t prefix] [-u] template ..."); std::process::exit(1); }
                tmpdir = Path::new(&argv[0]).to_path_buf();
            }
            "-t" => {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("usage: mktemp [-d] [-p dir] [-q] [-t prefix] [-u] template ..."); std::process::exit(1); }
                prefix = Some(argv[0].clone());
            }
            _ => { eprintln!("usage: mktemp [-d] [-p dir] [-q] [-t prefix] [-u] template ..."); std::process::exit(1); }
        }
        argv = &argv[1..];
    }

    let templates: Vec<&str> = if argv.is_empty() {
        vec!["tmp.XXXXXXXXXX"]
    } else {
        argv.iter().map(|s| s.as_str()).collect()
    };

    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    for tmpl in &templates {
        let resolved = if let Some(p) = &prefix {
            tmpdir.join(format!("{}.{}", p, tmpl.trim_start_matches("tmp.")))
        } else if tmpl.starts_with('/') || tmpl.starts_with('.') {
            Path::new(tmpl).to_path_buf()
        } else {
            tmpdir.join(tmpl)
        };

        let name = mktemp_internal(&resolved, is_dir, dry_run, seed);
        match name {
            Some(n) => println!("{}", n.display()),
            None => { if !quiet { eprintln!("mktemp: failed to create temp file"); } std::process::exit(1); }
        }
    }
}

fn mktemp_internal(template: &Path, is_dir: bool, dry_run: bool, seed: u128) -> Option<std::path::PathBuf> {
    let template_str = template.to_string_lossy().to_string();
    if !template_str.contains('X') {
        if template.exists() {
            return None;
        }
        if dry_run {
            return Some(template.to_path_buf());
        }
        if is_dir {
            fs::create_dir(template).ok()?;
        } else {
            fs::File::create(template).ok()?;
        }
        return Some(template.to_path_buf());
    }

    let mut rng = seed as u64;

    // Try up to 100 times
    for _ in 0..100 {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let suffix = format!("{:x}", rng);

        let mut name = template_str.clone();
        let mut suffix_chars = suffix.chars();
        for ch in &mut suffix_chars {
            if let Some(pos) = name.find('X') {
                name.replace_range(pos..=pos, &ch.to_string());
            }
        }
        // Replace remaining X's with '0'
        while let Some(pos) = name.find('X') {
            name.replace_range(pos..=pos, "0");
        }

        let path = Path::new(&name);
        if path.exists() { continue; }

        if !dry_run {
            if is_dir {
                fs::create_dir(path).ok()?;
            } else {
                fs::File::create(path).ok()?;
            }
        }
        return Some(path.to_path_buf());
    }

    None
}
