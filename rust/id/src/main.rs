//! Rust port of the MINIX/NetBSD `id` utility.
//!
//! Usage:
//!   id [user]
//!   id -G [-n] [user]
//!   id -g [-nr] [user]
//!   id -u [-nr] [user]
//!
//! Print user and group identity information.

use std::io::{self, Write};
use std::ffi::{CStr, CString};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut flag_g = false;
    let mut flag_G = false;
    let mut flag_n = false;
    let mut flag_r = false;
    let mut flag_u = false;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        for c in argv[0].chars().skip(1) {
            match c {
                'g' => flag_g = true,
                'G' => flag_G = true,
                'n' => flag_n = true,
                'r' => flag_r = true,
                'u' => flag_u = true,
                _ => {
                    eprintln!("id: unknown option -- {c}");
                    std::process::exit(1);
                }
            }
        }
        argv = &argv[1..];
    }

    let target_user = argv.first().map(|s| s.as_str());

    // If -u, -g, or -G is specified, print just that info (POSIX id behavior)
    if flag_u {
        let uid = if flag_r { unsafe { libc::getuid() } } else { unsafe { libc::geteuid() } };
        if flag_n {
            let pw = unsafe { libc::getpwuid(uid) };
            if !pw.is_null() {
                let name = unsafe { CStr::from_ptr((*pw).pw_name) }.to_str().unwrap_or("?");
                println!("{name}");
            } else {
                println!("{uid}");
            }
        } else {
            println!("{uid}");
        }
        return;
    }

    if flag_g {
        let gid = if flag_r { unsafe { libc::getgid() } } else { unsafe { libc::getegid() } };
        if flag_n {
            let gr = unsafe { libc::getgrgid(gid) };
            if !gr.is_null() {
                let name = unsafe { CStr::from_ptr((*gr).gr_name) }.to_str().unwrap_or("?");
                println!("{name}");
            } else {
                println!("{gid}");
            }
        } else {
            println!("{gid}");
        }
        return;
    }

    if flag_G {
        let uid = if let Some(user) = target_user {
            let cuser = CString::new(user.as_bytes()).unwrap();
            let pw = unsafe { libc::getpwnam(cuser.as_ptr()) };
            if pw.is_null() {
                eprintln!("id: {user}: no such user");
                std::process::exit(1);
            }
            unsafe { (*pw).pw_uid }
        } else {
            unsafe { libc::getuid() }
        };

    if flag_n {
        // Print group names
        let ngroups = unsafe { libc::getgroups(0, std::ptr::null_mut()) };
        if ngroups > 0 {
            let mut groups = vec![0 as libc::gid_t; ngroups as usize];
            unsafe { libc::getgroups(ngroups, groups.as_mut_ptr()); }
            let names: Vec<String> = groups.iter().map(|&g| {
                let gr = unsafe { libc::getgrgid(g) };
                if !gr.is_null() {
                    unsafe { CStr::from_ptr((*gr).gr_name) }.to_str().unwrap_or("?").to_string()
                } else {
                    g.to_string()
                }
            }).collect();
            println!("{}", names.join(" "));
        }
    } else {
        // Print group IDs
        let ngroups = unsafe { libc::getgroups(0, std::ptr::null_mut()) };
        if ngroups > 0 {
            let mut groups = vec![0 as libc::gid_t; ngroups as usize];
            unsafe { libc::getgroups(ngroups, groups.as_mut_ptr()); }
            let ids: Vec<String> = groups.iter().map(|g| g.to_string()).collect();
            println!("{}", ids.join(" "));
        } else {
            let gid = unsafe { libc::getgid() };
            println!("{gid}");
        }
    }
        return;
    }

    // Default: print full identity
    let uid = unsafe { libc::getuid() };
    let euid = unsafe { libc::geteuid() };
    let gid = unsafe { libc::getgid() };
    let egid = unsafe { libc::getegid() };

    let uname = username(uid).unwrap_or_else(|| uid.to_string());
    let euname = username(euid).unwrap_or_else(|| euid.to_string());
    let gname = groupname(gid).unwrap_or_else(|| gid.to_string());
    let egname = groupname(egid).unwrap_or_else(|| egid.to_string());

    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let _ = write!(handle, "uid={uid}({uname}) gid={gid}({gname})");
    if euid != uid {
        let _ = write!(handle, " euid={euid}({euname})");
    }
    if egid != gid {
        let _ = write!(handle, " egid={egid}({egname})");
    }

    // Supplementary groups
    let ngroups = unsafe { libc::getgroups(0, std::ptr::null_mut()) };
    if ngroups > 0 {
        let mut groups = vec![0u32; ngroups as usize];
        unsafe { libc::getgroups(ngroups, groups.as_mut_ptr() as *mut libc::gid_t); }
        let _ = write!(handle, " groups=");
        for (i, &g) in groups.iter().enumerate() {
            if i > 0 { let _ = write!(handle, ","); }
            let gn = groupname(g).unwrap_or_else(|| g.to_string());
            let _ = write!(handle, "{g}({gn})");
        }
    }
    let _ = writeln!(handle);
}

fn username(uid: u32) -> Option<String> {
    unsafe {
        let pw = libc::getpwuid(uid);
        if pw.is_null() { return None; }
        CStr::from_ptr((*pw).pw_name).to_str().ok().map(|s| s.to_string())
    }
}

fn groupname(gid: u32) -> Option<String> {
    unsafe {
        let gr = libc::getgrgid(gid);
        if gr.is_null() { return None; }
        CStr::from_ptr((*gr).gr_name).to_str().ok().map(|s| s.to_string())
    }
}
