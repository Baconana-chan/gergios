//! Rust port of the MINIX/NetBSD `whois` utility.
//!
//! Usage:
//!   whois [-h host] [-p port] name ...
//!
//! Queries WHOIS server for domain/network information.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];
    let mut server = "whois.iana.org".to_string();
    let mut port: u16 = 43;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        if opt == "-h" && opt.len() == 2 {
            argv = &argv[1..];
            if argv.is_empty() { eprintln!("usage: whois [-h host] [-p port] name ..."); std::process::exit(1); }
            server = argv[0].clone();
        } else if opt == "-p" && opt.len() == 2 {
            argv = &argv[1..];
            if argv.is_empty() { eprintln!("usage: whois [-h host] [-p port] name ..."); std::process::exit(1); }
            port = argv[0].parse().unwrap_or_else(|_| { eprintln!("whois: invalid port"); std::process::exit(1); });
        } else {
            eprintln!("usage: whois [-h host] [-p port] name ...");
            std::process::exit(1);
        }
        argv = &argv[1..];
    }

    if argv.is_empty() {
        eprintln!("usage: whois [-h host] [-p port] name ...");
        std::process::exit(1);
    }

    for name in argv {
        query_whois(name, &server, port);
    }
}

fn query_whois(query: &str, server: &str, port: u16) {
    let addr_str = format!("{}:{}", server, port);
    let addr = match addr_str.to_socket_addrs() {
        Ok(mut addrs) => {
            match addrs.next() {
                Some(a) => a,
                None => { eprintln!("whois: cannot resolve {server}"); std::process::exit(1); }
            }
        }
        Err(e) => { eprintln!("whois: {server}: {e}"); std::process::exit(1); }
    };

    let mut stream = match TcpStream::connect(addr) {
        Ok(s) => s,
        Err(e) => { eprintln!("whois: {server}: {e}"); std::process::exit(1); }
    };

    // Send query
    let request = format!("{query}\r\n");
    if stream.write_all(request.as_bytes()).is_err() {
        eprintln!("whois: write error");
        std::process::exit(1);
    }

    // Read response
    let mut response = String::new();
    if stream.read_to_string(&mut response).is_err() {
        eprintln!("whois: read error");
        std::process::exit(1);
    }

    print!("{}", response);
}
