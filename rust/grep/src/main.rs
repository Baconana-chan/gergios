//! Rust port of the MINIX/NetBSD `grep` utility.
//!
//! Supports: -E, -F, -G, -i, -v, -w, -x, -c, -l, -L, -n, -b, -o, -q, -s,
//!           -H, -h, -r/R, -a, -I, -U, -A, -B, -C, -e, -f, -Z,
//!           --line-buffered, --binary-files, context digits (0-9)
//!
//! No `unsafe` code except `Mmap::map`.

use std::borrow::Cow;
use std::io::{self, BufRead, BufReader, Read, Write, stdout};
use std::path::Path;
use std::process;
use std::fs::File;

use flate2::read::GzDecoder;
use memmap2::Mmap;
use regex::bytes::Regex;
use walkdir::WalkDir;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum BinaryBehavior { Bin, Skip, Text }

#[derive(Clone)]
struct GrepOpts {
    after_context: usize,
    before_context: usize,
    basic_regexp: bool,
    extended_regexp: bool,
    fixed_strings: bool,
    count: bool,
    ignore_case: bool,
    files_with_matches: bool,
    files_without_match: bool,
    line_number: bool,
    only_matching: bool,
    quiet: bool,
    silent: bool,
    invert_match: bool,
    word_regexp: bool,
    line_regexp: bool,
    byte_offset: bool,
    recursive: bool,
    always_filename: bool,
    no_filename: bool,
    decompress: bool,
    binary: BinaryBehavior,
    line_buffered: bool,
    patterns: Vec<String>,
    files: Vec<String>,
}

impl Default for GrepOpts {
    fn default() -> Self {
        Self {
            after_context: 0, before_context: 0,
            basic_regexp: false,
            extended_regexp: false, fixed_strings: false,
            count: false, ignore_case: false,
            files_with_matches: false, files_without_match: false,
            line_number: false, only_matching: false,
            quiet: false, silent: false,
            invert_match: false, word_regexp: false, line_regexp: false,
            byte_offset: false, recursive: false,
            always_filename: false, no_filename: false,
            decompress: false, binary: BinaryBehavior::Bin,
            line_buffered: false,
            patterns: Vec::new(), files: Vec::new(),
        }
    }
}

struct CompiledPattern {
    kind: PatternKind,
    qs_bc: Option<[usize; 256]>,
    pattern_len: usize,
    pattern: Vec<u8>,
    ignore_case: bool,
}

enum PatternKind {
    Fixed,
    Regex(Regex),
}

struct ContextQueue {
    capacity: usize,
    items: Vec<LineRef>,
}

struct LineRef {
    line_no: usize,
    offset: u64,
    data: Vec<u8>,
}

struct ProcessFileResult {
    match_count: usize,
    file_err: bool,
}

// ---------------------------------------------------------------------------
// Option parsing
// ---------------------------------------------------------------------------

fn bre_to_regex(pat: &str) -> String {
    let mut out = String::with_capacity(pat.len() + 4);
    let mut escape = false;
    for ch in pat.chars() {
        if escape {
            escape = false;
            match ch {
                '(' => out.push('('), ')' => out.push(')'),
                '{' => out.push('{'), '}' => out.push('}'),
                '?' => out.push('?'), '+' => out.push('+'), '|' => out.push('|'),
                _ => { out.push('\\'); out.push(ch); }
            }
        } else if ch == '\\' {
            escape = true;
        } else {
            out.push(ch);
        }
    }
    if escape { out.push('\\'); }
    out
}

fn build_regex_pattern(opts: &GrepOpts, pat: &str) -> String {
    let mut result = String::new();
    if opts.ignore_case { result.push_str("(?i)"); }

    let translated = if opts.extended_regexp {
        pat.to_string()
    } else {
        bre_to_regex(pat)
    };

    if opts.line_regexp {
        result.push('^');
        if opts.word_regexp { result.push_str("(?-u:\\b)"); }
        result.push_str(&translated);
        if opts.word_regexp { result.push_str("(?-u:\\b)"); }
        result.push('$');
    } else if opts.word_regexp {
        result.push_str("(?-u:\\b)");
        result.push_str(&translated);
        result.push_str("(?-u:\\b)");
    } else {
        result.push_str(&translated);
    }
    result
}

fn parse_args() -> GrepOpts {
    let args: Vec<String> = std::env::args().collect();
    let progname_s = Path::new(&args[0])
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("grep")
        .to_string();
    let progname = progname_s.as_str();

    let mut opts = GrepOpts::default();

    // Detect variant name (egrep, fgrep, zgrep, etc.)
    if let Some(first) = progname.chars().next() {
        match first {
            'e' => opts.extended_regexp = true,
            'f' => opts.fixed_strings = true,
            'z' => {
                opts.decompress = true;
                if let Some(second) = progname.chars().nth(1) {
                    match second {
                        'e' => opts.extended_regexp = true,
                        'f' => opts.fixed_strings = true,
                        'g' => opts.basic_regexp = true,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    let mut i = 1;
    let mut need_pattern = true;
    let mut exprs: Vec<String> = Vec::new();

    while i < args.len() {
        let arg = &args[i];
        if arg == "--" { i += 1; break; }
        if !arg.starts_with('-') || arg == "-" { break; }

        // Long options (--xxx)
        if arg.starts_with("--") {
            let rest = &arg[2..];
            match rest {
                "binary-files" => {
                    i += 1;
                    match args.get(i).map(|s| s.as_str()).unwrap_or("") {
                        "binary" => opts.binary = BinaryBehavior::Bin,
                        "without-match" => opts.binary = BinaryBehavior::Skip,
                        "text" => opts.binary = BinaryBehavior::Text,
                        _ => { eprintln!("{}: Unknown binary-files option", progname); process::exit(2); }
                    }
                }
                "help" => { usage(progname); process::exit(0); }
                "mmap" | "unix-byte-offsets" => {}
                "line-buffered" => opts.line_buffered = true,
                "after-context" => { opts.after_context = parse_long_num(&args, &mut i, progname, rest); }
                "before-context" => { opts.before_context = parse_long_num(&args, &mut i, progname, rest); }
                "context" => {
                    let val = if i + 1 < args.len() && !args[i+1].starts_with('-') {
                        i += 1; args[i].parse().unwrap_or(2)
                    } else { 2 };
                    opts.after_context = val; opts.before_context = val;
                }
                "quiet" | "silent" => opts.quiet = true,
                "recursive" => opts.recursive = true,
                "no-messages" => opts.silent = true,
                "with-filename" => opts.always_filename = true,
                "no-filename" => opts.no_filename = true,
                "ignore-case" => opts.ignore_case = true,
                "basic-regexp" => { opts.basic_regexp = true; opts.extended_regexp = false; opts.fixed_strings = false; }
                "extended-regexp" => { opts.extended_regexp = true; opts.fixed_strings = false; opts.basic_regexp = false; }
                "fixed-strings" => { opts.fixed_strings = true; opts.extended_regexp = false; opts.basic_regexp = false; }
                "files-without-match" => { opts.files_without_match = true; opts.quiet = true; }
                "files-with-matches" => { opts.files_with_matches = true; opts.quiet = true; }
                "count" => opts.count = true,
                "line-number" => opts.line_number = true,
                "byte-offset" => opts.byte_offset = true,
                "only-matching" => opts.only_matching = true,
                "revert-match" => opts.invert_match = true,
                "word-regexp" => opts.word_regexp = true,
                "line-regexp" => opts.line_regexp = true,
                "binary" => opts.binary = BinaryBehavior::Bin,
                "decompress" => opts.decompress = true,
                "text" => opts.binary = BinaryBehavior::Text,
                "version" => { eprintln!("grep version 1.0 (Rust port)"); process::exit(0); }
                "devices" => { if i + 1 < args.len() { i += 1; } }
                _ => {
                    eprintln!("{}: Unknown option -- {}", progname, rest);
                    usage(progname); process::exit(2);
                }
            }
            i += 1;
            continue;
        }

        // Short options (-x, -abc, etc.)
        let mut chars = arg[1..].chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '0'..='9' => {
                    let mut num = (c as u8 - b'0') as usize;
                    while let Some(&d) = chars.peek() {
                        if d.is_ascii_digit() {
                            num = num.saturating_mul(10).saturating_add((d as u8 - b'0') as usize);
                            chars.next();
                        } else { break; }
                    }
                    opts.after_context = num; opts.before_context = num;
                }
                'A' => { opts.after_context = parse_opt_num(&args, &mut i, &mut chars); }
                'B' => { opts.before_context = parse_opt_num(&args, &mut i, &mut chars); }
                'C' => {
                    let val = if chars.peek().is_some() {
                        parse_inline_digits(&mut chars)
                    } else if i + 1 < args.len() && !args[i+1].starts_with('-') {
                        i += 1; args[i].parse().unwrap_or(2)
                    } else { 2 };
                    opts.after_context = val; opts.before_context = val;
                }
                'D' => {
                    if chars.peek().is_some() { while chars.next().is_some() {} }
                    else if i + 1 < args.len() { i += 1; }
                }
                'E' => { opts.extended_regexp = true; opts.fixed_strings = false; opts.basic_regexp = false; }
                'F' => { opts.fixed_strings = true; opts.extended_regexp = false; opts.basic_regexp = false; }
                'G' => { opts.basic_regexp = true; opts.extended_regexp = false; opts.fixed_strings = false; }
                'H' => opts.always_filename = true,
                'I' => opts.binary = BinaryBehavior::Skip,
                'L' => { opts.files_without_match = true; opts.quiet = true; }
                'R' | 'r' => opts.recursive = true,
                'U' => opts.binary = BinaryBehavior::Bin,
                'V' => { eprintln!("grep version 1.0 (Rust port)"); process::exit(0); }
                'Z' => opts.decompress = true,
                'a' => opts.binary = BinaryBehavior::Text,
                'b' => opts.byte_offset = true,
                'c' => opts.count = true,
                'e' => {
                    need_pattern = false;
                    let rest: String = (&mut chars).collect();
                    if !rest.is_empty() { exprs.push(rest); }
                    else { i += 1; exprs.push(args[i].clone()); }
                }
                'f' => {
                    need_pattern = false;
                    let fname: String = (&mut chars).collect();
                    let fname = if !fname.is_empty() { fname }
                        else { i += 1; args[i].clone() };
                    match read_pattern_file(&fname) {
                        Ok(pats) => opts.patterns.extend(pats),
                        Err(e) => {
                            if !opts.silent { eprintln!("{}: {}: {}", progname, fname, e); }
                            process::exit(2);
                        }
                    }
                }
                'h' => opts.no_filename = true,
                'i' | 'y' => opts.ignore_case = true,
                'l' => { opts.files_with_matches = true; opts.quiet = true; }
                'n' => opts.line_number = true,
                'o' => opts.only_matching = true,
                'q' => opts.quiet = true,
                's' => opts.silent = true,
                'v' => opts.invert_match = true,
                'w' => opts.word_regexp = true,
                'x' => opts.line_regexp = true,
                _ => {}
            }
        }
        i += 1;
    }

    let remaining: Vec<String> = args[i..].to_vec();

    if !exprs.is_empty() {
        opts.patterns.splice(0..0, exprs);
    }

    if need_pattern {
        if remaining.is_empty() {
            eprintln!("{}: No pattern specified", progname);
            usage(progname); process::exit(2);
        }
        opts.patterns.push(remaining[0].clone());
        opts.files.extend(remaining[1..].iter().cloned());
    } else {
        opts.files.extend(remaining);
    }

    if !opts.always_filename {
        if (opts.files.len() <= 1 && !opts.recursive) || opts.no_filename {
            opts.no_filename = true;
        }
    }

    opts
}

fn parse_opt_num(args: &[String], i: &mut usize, chars: &mut std::iter::Peekable<std::str::Chars>) -> usize {
    let rest: String = chars.collect();
    if !rest.is_empty() { return rest.parse().unwrap_or(2); }
    if *i + 1 < args.len() && !args[*i + 1].starts_with('-') {
        *i += 1; return args[*i].parse().unwrap_or(2);
    }
    2
}

fn parse_inline_digits(chars: &mut std::iter::Peekable<std::str::Chars>) -> usize {
    let mut num = 0usize;
    while let Some(&d) = chars.peek() {
        if d.is_ascii_digit() {
            num = num.saturating_mul(10).saturating_add((d as u8 - b'0') as usize);
            chars.next();
        } else { break; }
    }
    if num == 0 { 2 } else { num }
}

fn parse_long_num(args: &[String], i: &mut usize, progname: &str, opt: &str) -> usize {
    if *i + 1 < args.len() && !args[*i + 1].starts_with('-') {
        *i += 1;
        args[*i].parse().unwrap_or_else(|_| {
            eprintln!("{}: {} requires a numeric argument", progname, opt);
            process::exit(2);
        })
    } else {
        eprintln!("{}: {} requires an argument", progname, opt);
        process::exit(2);
    }
}

fn read_pattern_file(path: &str) -> io::Result<Vec<String>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut patterns = Vec::new();
    for line in reader.lines() {
        let line = line?;
        patterns.push(line.trim_end_matches('\n').to_string());
    }
    Ok(patterns)
}

fn usage(progname: &str) {
    eprintln!(
        "usage: {} [-abcEFGhHiIlnnoqRsUVvwxZ] [-A num] [-B num] [-C[num]]\n\
         \t[-e pattern] [-f file] [--binary-files=value] [--context[=num]]\n\
         \t[--line-buffered] [pattern] [file ...]",
        progname
    );
}

// ---------------------------------------------------------------------------
// Pattern compilation
// ---------------------------------------------------------------------------

fn compile_patterns(opts: &GrepOpts) -> Vec<CompiledPattern> {
    let mut compiled = Vec::with_capacity(opts.patterns.len());
    for pat in &opts.patterns {
        if opts.fixed_strings {
            compiled.push(compile_fixed(pat, opts.ignore_case));
        } else {
            compiled.push(compile_regex(opts, pat));
        }
    }
    compiled
}

fn compile_fixed(pat: &str, ignore_case: bool) -> CompiledPattern {
    let pattern = if ignore_case {
        pat.to_uppercase().into_bytes()
    } else {
        pat.as_bytes().to_vec()
    };
    let pattern_len = pattern.len();
    let mut qs_bc = [pattern_len; 256];
    for i in 1..pattern_len {
        qs_bc[pattern[i] as usize] = pattern_len - i;
        if ignore_case {
            qs_bc[pattern[i].to_ascii_lowercase() as usize] = pattern_len - i;
        }
    }
    CompiledPattern {
        kind: PatternKind::Fixed,
        qs_bc: Some(qs_bc),
        pattern_len,
        pattern,
        ignore_case,
    }
}

fn compile_regex(opts: &GrepOpts, pat: &str) -> CompiledPattern {
    let re_str = build_regex_pattern(opts, pat);
    match Regex::new(&re_str) {
        Ok(re) => CompiledPattern {
            kind: PatternKind::Regex(re),
            qs_bc: None,
            pattern_len: 0,
            pattern: Vec::new(),
            ignore_case: opts.ignore_case,
        },
        Err(e) => {
            eprintln!("grep: Invalid regex pattern: {} ({})", pat, e);
            process::exit(2);
        }
    }
}

// ---------------------------------------------------------------------------
// Quick Search (for -F mode)
// ---------------------------------------------------------------------------

/// Quick Search using precomputed shift table, starting from offset.
fn quick_search_first_at(cp: &CompiledPattern, data: &[u8], start: usize) -> Option<usize> {
    let pattern = &cp.pattern;
    let pattern_len = cp.pattern_len;
    let qs_bc = cp.qs_bc.as_ref()?;

    let haystack = if cp.ignore_case { Cow::Owned(data.to_ascii_uppercase()) } else { Cow::Borrowed(data) };

    let data_len = haystack.len();
    if start + pattern_len > data_len || pattern_len == 0 { return None; }

    let mut j = start;
    while j + pattern_len <= data_len {
        let mut matched = true;
        for k in 0..pattern_len {
            if pattern[k] != haystack[j + k] { matched = false; break; }
        }
        if matched { return Some(j); }
        if j + pattern_len == data_len { break; }
        j += qs_bc[haystack[j + pattern_len] as usize];
    }
    None
}

/// Quick Search returning ALL match positions.
fn quick_search_all(cp: &CompiledPattern, data: &[u8]) -> Vec<usize> {
    let qs_bc = match cp.qs_bc.as_ref() { Some(t) => t, None => return Vec::new() };
    let pattern = &cp.pattern;
    let pattern_len = cp.pattern_len;

    let haystack = if cp.ignore_case { Cow::Owned(data.to_ascii_uppercase()) } else { Cow::Borrowed(data) };
    let data_len = haystack.len();

    if data_len < pattern_len || pattern_len == 0 { return Vec::new(); }

    let mut positions = Vec::new();
    let mut j = 0usize;
    while j + pattern_len <= data_len {
        let mut matched = true;
        for k in 0..pattern_len {
            if pattern[k] != haystack[j + k] { matched = false; break; }
        }
        if matched {
            positions.push(j);
            j += pattern_len;
            if j >= data_len { break; }
        }
        if j + pattern_len >= data_len { break; }
        j += qs_bc[haystack[j + pattern_len] as usize];
    }
    positions
}

// ---------------------------------------------------------------------------
// Matching
// ---------------------------------------------------------------------------

fn is_word_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn check_word_boundary(line: &[u8], start: usize, end: usize) -> bool {
    let left_ok = start == 0 || !is_word_char(line[start - 1]);
    let right_ok = end >= line.len() || !is_word_char(line[end]);
    left_ok && right_ok && start < end && is_word_char(line[start]) && is_word_char(line[end - 1])
}

fn line_matches(cp: &CompiledPattern, line: &[u8], opts: &GrepOpts) -> (bool, Vec<(usize, usize)>) {
    match &cp.kind {
        PatternKind::Fixed => {
            if opts.only_matching && !opts.invert_match {
                let positions = quick_search_all(cp, line);
                if positions.is_empty() { return (false, Vec::new()); }
                let matches: Vec<(usize, usize)> = positions.into_iter()
                    .filter(|&p| !opts.word_regexp || check_word_boundary(line, p, p + cp.pattern_len))
                    .map(|p| (p, p + cp.pattern_len))
                    .collect();
                if matches.is_empty() { return (false, Vec::new()); }
                return (true, matches);
            }
            // Normal mode: find first match (scan all for -w)
            let mut offset = 0usize;
            loop {
                let pos = match quick_search_first_at(cp, line, offset) {
                    Some(p) => p,
                    None => return (false, Vec::new()),
                };
                if opts.line_regexp && (pos != 0 || pos + cp.pattern_len != line.len()) {
                    return (false, Vec::new());
                }
                if !opts.word_regexp || check_word_boundary(line, pos, pos + cp.pattern_len) {
                    return (true, vec![(pos, pos + cp.pattern_len)]);
                }
                // Try next occurrence for -w
                offset = pos + 1;
                if offset >= line.len() { return (false, Vec::new()); }
            }
        }
        PatternKind::Regex(re) => {
            if opts.only_matching && !opts.invert_match {
                let matches: Vec<(usize, usize)> = re.find_iter(line)
                    .filter(|m| m.start() < m.end())
                    .filter(|m| !opts.word_regexp || check_word_boundary(line, m.start(), m.end()))
                    .map(|m| (m.start(), m.end()))
                    .collect();
                if matches.is_empty() { return (false, Vec::new()); }
                return (true, matches);
            }
            let matched = re.is_match(line);
            if matched && opts.word_regexp {
                for m in re.find_iter(line) {
                    if check_word_boundary(line, m.start(), m.end()) {
                        return (true, Vec::new());
                    }
                }
                return (false, Vec::new());
            }
            (matched, Vec::new())
        }
    }
}

// ---------------------------------------------------------------------------
// File I/O
// ---------------------------------------------------------------------------

/// A unified line reader that handles stdin, regular files, gzip, and mmap.
enum FileReader {
    Stdio {
        reader: BufReader<Box<dyn Read>>,
        buf: Vec<u8>,
        pos: usize,
    },
    Gzip(BufReader<Box<dyn Read>>),
    Mmap { data: Vec<u8>, pos: usize },
}

impl FileReader {
    fn open(path: &Path, decompress: bool) -> io::Result<Self> {
        if decompress {
            let file = File::open(path)?;
            let decoder = GzDecoder::new(file);
            Ok(FileReader::Gzip(BufReader::new(Box::new(decoder) as Box<dyn Read>)))
        } else {
            let file = File::open(path)?;
            // Try mmap first
            if let Ok(mmap) = unsafe { Mmap::map(&file) } {
                let data = mmap[..].to_vec();
                return Ok(FileReader::Mmap { data, pos: 0 });
            }
            Ok(FileReader::Stdio {
                reader: BufReader::new(Box::new(file) as Box<dyn Read>),
                buf: Vec::new(),
                pos: 0,
            })
        }
    }

    fn open_stdin() -> Self {
        FileReader::Stdio {
            reader: BufReader::new(Box::new(io::stdin()) as Box<dyn Read>),
            buf: Vec::new(),
            pos: 0,
        }
    }

    fn is_non_seekable(&self) -> bool {
        matches!(self, FileReader::Gzip(_))
    }

    /// Read next line (including trailing \n). Returns (bytes_read, is_new).
    fn read_line(&mut self, out: &mut Vec<u8>) -> io::Result<Option<usize>> {
        out.clear();
        match self {
            FileReader::Stdio { reader, buf, pos } => {
                if *pos < buf.len() {
                    // We have buffered data from binary detection
                    let data = &buf[*pos..];
                    let mut end = 0;
                    while end < data.len() && data[end] != b'\n' { end += 1; }
                    if end < data.len() {
                        // Found \n in buffer
                        out.extend_from_slice(&data[..=end]);
                        *pos += end + 1;
                        return Ok(Some(out.len()));
                    }
                    // No \n in buffer — consume all and continue reading
                    out.extend_from_slice(data);
                    *pos = buf.len();
                }
                // Read more from source
                let n = reader.read_until(b'\n', out)?;
                if n == 0 { return Ok(None); }
                Ok(Some(n))
            }
            FileReader::Gzip(reader) => {
                let n = reader.read_until(b'\n', out)?;
                if n == 0 { return Ok(None); }
                Ok(Some(n))
            }
            FileReader::Mmap { data, pos } => {
                if *pos >= data.len() { return Ok(None); }
                let start = *pos;
                while *pos < data.len() && data[*pos] != b'\n' { *pos += 1; }
                out.extend_from_slice(&data[start..*pos]);
                if *pos < data.len() && data[*pos] == b'\n' {
                    out.push(b'\n');
                    *pos += 1;
                }
                if out.is_empty() { return Ok(None); }
                Ok(Some(out.len()))
            }
        }
    }

    /// Read a chunk for binary detection. Buffered internally.
    fn read_chunk(&mut self, size: usize) -> io::Result<Vec<u8>> {
        match self {
            FileReader::Stdio { reader, buf, .. } => {
                let mut tmp = vec![0u8; size];
                let n = reader.read(&mut tmp)?;
                tmp.truncate(n);
                buf.extend_from_slice(&tmp);
                Ok(tmp)
            }
            FileReader::Gzip(reader) => {
                let mut tmp = vec![0u8; size];
                let n = reader.read(&mut tmp)?;
                tmp.truncate(n);
                Ok(tmp)
            }
            FileReader::Mmap { data, pos: _ } => {
                let n = std::cmp::min(size, data.len());
                let chunk = data[..n].to_vec();
                Ok(chunk)
            }
        }
    }
}

fn is_binary(data: &[u8]) -> bool {
    data.iter().any(|&b| b == 0)
}

// ---------------------------------------------------------------------------
// Context Queue
// ---------------------------------------------------------------------------

impl ContextQueue {
    fn new(capacity: usize) -> Self {
        Self { capacity, items: Vec::with_capacity(capacity) }
    }

    fn enqueue(&mut self, line: Vec<u8>, line_no: usize, offset: u64) {
        if self.capacity == 0 { return; }
        if self.items.len() >= self.capacity { self.items.remove(0); }
        self.items.push(LineRef { line_no, offset, data: line });
    }

    fn drain(&mut self) -> Vec<LineRef> { self.items.drain(..).collect() }
    fn clear(&mut self) { self.items.clear(); }
    fn len(&self) -> usize { self.items.len() }
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

fn print_line(
    out: &mut impl Write, line: &[u8], line_no: usize, offset: u64,
    fname: &str, show_filename: bool, opts: &GrepOpts, sep: char,
) {
    let mut n = 0;
    if show_filename {
        let _ = write!(out, "{}", fname);
        n += 1;
    }
    if opts.line_number {
        if n > 0 { let _ = write!(out, "{}", sep); }
        let _ = write!(out, "{}", line_no);
        n += 1;
    }
    if opts.byte_offset {
        if n > 0 { let _ = write!(out, "{}", sep); }
        let _ = write!(out, "{}", offset);
        n += 1;
    }
    if n > 0 { let _ = write!(out, "{}", sep); }
    let _ = out.write_all(line);
    let _ = writeln!(out);
}

// ---------------------------------------------------------------------------
// Main processing
// ---------------------------------------------------------------------------

fn proc_file(opts: &GrepOpts, patterns: &[CompiledPattern], path: Option<&Path>) -> ProcessFileResult {
    let fname = path.and_then(|p| p.to_str()).unwrap_or("(standard input)");
    let show_filename = !opts.no_filename &&
        (opts.files.len() > 1 || opts.recursive || opts.always_filename);

    let mut reader: FileReader = if let Some(p) = path {
        match FileReader::open(p, opts.decompress) {
            Ok(r) => r,
            Err(e) => {
                if !opts.silent { eprintln!("grep: {}: {}", fname, e); }
                return ProcessFileResult { match_count: 0, file_err: true };
            }
        }
    } else {
        FileReader::open_stdin()
    };

    // Binary detection: read first chunk for seekable files
    let mut is_binary_file = false;
    if !reader.is_non_seekable() {
        let chunk = reader.read_chunk(8192).unwrap_or_default();
        if !chunk.is_empty() && is_binary(&chunk) {
            is_binary_file = true;
            if opts.binary == BinaryBehavior::Skip {
                return ProcessFileResult { match_count: 0, file_err: false };
            }
        }
    }

    let mut match_count = 0;
    let mut file_err = false;
    let mut context_queue = ContextQueue::new(opts.before_context);
    let mut line_no: usize = 0;
    let mut byte_offset: u64 = 0;
    let mut tail = 0usize;
    let mut has_printed_context = false;
    let mut out = stdout().lock();
    let mut line_buf = Vec::new();

    loop {
        let n = match reader.read_line(&mut line_buf) {
            Ok(Some(n)) => n,
            Ok(None) => break,
            Err(e) => {
                if !opts.silent { eprintln!("grep: {}: {}", fname, e); }
                file_err = true;
                break;
            }
        };

        // Strip trailing \n for matching
        let raw = line_buf.as_slice();
        let line = if raw.ends_with(b"\n") { &raw[..raw.len() - 1] } else { raw };

        line_no += 1;
        let current_offset = byte_offset;
        byte_offset += n as u64;

        // Binary check for non-seekable files (check line by line)
        if reader.is_non_seekable() && !is_binary_file && line.contains(&0) {
            is_binary_file = true;
        }
        if is_binary_file && opts.binary == BinaryBehavior::Skip { continue; }

        // Check all patterns
        let mut line_matched = false;
        let mut match_positions = Vec::new();

        for pat in patterns {
            let (matched, positions) = line_matches(pat, line, opts);
            if matched {
                line_matched = true;
                match_positions = positions;
                if !opts.only_matching || opts.invert_match { break; }
            }
        }

        if opts.invert_match { line_matched = !line_matched; }
        if line_matched { match_count += 1; }
        if opts.quiet && line_matched { process::exit(0); }

        // Output logic
        if line_matched {
            if has_printed_context && tail == 0 && context_queue.len() > 0
                && (opts.before_context > 0 || opts.after_context > 0)
                && !opts.count && !opts.files_with_matches && !opts.files_without_match
            {
                let _ = writeln!(out, "--");
            }
            has_printed_context = true;

            if opts.before_context > 0 && !opts.count && !opts.files_with_matches && !opts.files_without_match {
                for item in context_queue.drain() {
                    print_line(&mut out, &item.data, item.line_no, item.offset, fname, show_filename, opts, '-');
                }
            }
            context_queue.clear();

            if !opts.count && !opts.files_with_matches && !opts.files_without_match {
                if opts.only_matching && !opts.invert_match {
                    if match_positions.is_empty() {
                        print_line(&mut out, line, line_no, current_offset, fname, show_filename, opts, ':');
                    } else {
                        for (start, end) in &match_positions {
                            let segment = &line[*start..*end];
                            print_line(&mut out, segment, line_no, current_offset + *start as u64, fname, show_filename, opts, ':');
                        }
                    }
                } else {
                    print_line(&mut out, line, line_no, current_offset, fname, show_filename, opts, ':');
                }
            }
            tail = opts.after_context;
        } else if tail > 0 {
            tail -= 1;
            if !opts.count && !opts.files_with_matches && !opts.files_without_match {
                print_line(&mut out, line, line_no, current_offset, fname, show_filename, opts, '-');
            }
        } else if opts.before_context > 0 {
            context_queue.enqueue(line.to_vec(), line_no, current_offset);
        }

        if opts.line_buffered { let _ = out.flush(); }
    }

    context_queue.clear();

    if opts.count {
        if show_filename { let _ = write!(out, "{}:", fname); }
        let _ = writeln!(out, "{}", match_count);
    }
    if opts.files_with_matches && match_count > 0 {
        let _ = writeln!(out, "{}", fname);
    }
    if opts.files_without_match && match_count == 0 {
        let _ = writeln!(out, "{}", fname);
    }
    if match_count > 0 && is_binary_file && opts.binary == BinaryBehavior::Bin
        && !opts.count && !opts.files_with_matches && !opts.files_without_match && !opts.quiet
    {
        let _ = writeln!(out, "Binary file {} matches", fname);
    }

    let _ = out.flush();
    ProcessFileResult { match_count, file_err }
}

// ---------------------------------------------------------------------------
// Recursive search
// ---------------------------------------------------------------------------

fn search_recursive(opts: &GrepOpts, patterns: &[CompiledPattern]) -> ProcessFileResult {
    let mut total = 0usize;
    let mut had_err = false;
    for file_path in &opts.files {
        let root = Path::new(file_path);
        if !root.exists() {
            if !opts.silent { eprintln!("grep: {}: No such file or directory", file_path); }
            had_err = true;
            continue;
        }
        for entry in WalkDir::new(root).follow_links(true) {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            if entry.file_type().is_dir() { continue; }
            let result = proc_file(opts, patterns, Some(entry.path()));
            total += result.match_count;
            if result.file_err { had_err = true; }
        }
    }
    ProcessFileResult { match_count: total, file_err: had_err }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let opts = parse_args();
    let patterns = compile_patterns(&opts);

    let result = if opts.recursive {
        search_recursive(&opts, &patterns)
    } else if opts.files.is_empty() {
        proc_file(&opts, &patterns, None)
    } else {
        let mut total = 0usize;
        let mut had_err = false;
        for f in &opts.files {
            let r = proc_file(&opts, &patterns, Some(Path::new(f)));
            total += r.match_count;
            if r.file_err { had_err = true; }
        }
        ProcessFileResult { match_count: total, file_err: had_err }
    };

    let exit_code = if result.match_count > 0 {
        if result.file_err && !opts.quiet { 2 } else { 0 }
    } else {
        if result.file_err { 2 } else { 1 }
    };

    process::exit(exit_code);
}
