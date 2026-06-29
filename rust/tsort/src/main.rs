//! Rust port of the MINIX/NetBSD `tsort` utility.
//!
//! Usage:
//!   tsort [file]
//!
//! Topological sort of input pairs (partial ordering).
//! Each pair represents a dependency: first element must come before second.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::File;
use std::io::{self, BufRead, BufReader};

const USAGE: &str = "usage: tsort [file]";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let argv = &args[1..];

    let file_name = if argv.len() > 1 {
        eprintln!("{USAGE}");
        std::process::exit(1);
    } else if argv.is_empty() {
        "-"
    } else {
        &argv[0]
    };

    let reader: Box<dyn BufRead> = if file_name == "-" {
        Box::new(BufReader::new(io::stdin().lock()))
    } else {
        match File::open(file_name) {
            Ok(f) => Box::new(BufReader::new(f)),
            Err(e) => { eprintln!("tsort: {file_name}: {e}"); std::process::exit(1); }
        }
    };

    // Build graph: node -> set of successors (dependencies)
    let mut successors: HashMap<String, HashSet<String>> = HashMap::new();
    let mut predecessors: HashMap<String, HashSet<String>> = HashMap::new();
    let mut all_nodes: HashSet<String> = HashSet::new();

    for line_res in reader.lines() {
        let line = match line_res { Ok(l) => l, Err(_) => break };
        let trimmed = line.trim().to_string();
        if trimmed.is_empty() { continue; }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() < 2 { continue; }

        let from = parts[0].to_string();
        let to = parts[1].to_string();

        all_nodes.insert(from.clone());
        all_nodes.insert(to.clone());
        successors.entry(from.clone()).or_default().insert(to.clone());
        predecessors.entry(to.clone()).or_default().insert(from.clone());
        // Ensure 'from' has an entry in predecessors too (for nodes with no predecessors)
        predecessors.entry(from.clone()).or_default();
    }

    // Kahn's algorithm for topological sort
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    for node in &all_nodes {
        in_degree.insert(node.clone(), predecessors.get(node).map_or(0, |p| p.len()));
    }

    let mut queue: VecDeque<String> = VecDeque::new();
    for (node, degree) in &in_degree {
        if *degree == 0 {
            queue.push_back(node.clone());
        }
    }

    let mut sorted: Vec<String> = Vec::new();
    while let Some(node) = queue.pop_front() {
        sorted.push(node.clone());
        if let Some(succs) = successors.get(&node) {
            for succ in succs {
                if let Some(degree) = in_degree.get_mut(succ) {
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push_back(succ.clone());
                    }
                }
            }
        }
    }

    // If there's a cycle, do a simple stable sort as fallback
    if sorted.len() < all_nodes.len() {
        let mut remaining: Vec<String> = all_nodes.iter()
            .filter(|n| !sorted.contains(n))
            .cloned()
            .collect();
        remaining.sort();
        sorted.extend(remaining);
    }

    for node in &sorted {
        println!("{node}");
    }
}
