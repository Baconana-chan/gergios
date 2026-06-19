use std::env;
use std::process;

extern crate libc;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() == 1 {
        // No args: display current NIS domain name
        let mut buf = [0i8; 256];
        let result = unsafe { libc::getdomainname(buf.as_mut_ptr(), buf.len()) };
        if result != 0 {
            let err = unsafe { *libc::__errno() };
            if err == libc::ENOSYS {
                eprintln!("domainname: NIS not available");
            } else {
                eprintln!("domainname: error getting domain name (errno={})", err);
            }
            process::exit(1);
        }
        // Convert to string
        let name = unsafe {
            let cstr = std::ffi::CStr::from_ptr(buf.as_ptr());
            cstr.to_string_lossy().to_string()
        };
        if name.is_empty() {
            println!("(none)");
        } else {
            println!("{}", name);
        }
    } else {
        // Set domain name
        let name = &args[1];
        let cname = std::ffi::CString::new(name.as_bytes())
            .unwrap_or_else(|_| {
                eprintln!("domainname: domain name contains null byte");
                process::exit(1);
            });
        let result = unsafe { libc::setdomainname(cname.as_ptr(), name.len()) };
        if result != 0 {
            let err = unsafe { *libc::__errno() };
            if err == libc::EPERM {
                eprintln!("domainname: Permission denied");
            } else if err == libc::ENOSYS {
                eprintln!("domainname: NIS not available");
            } else {
                eprintln!("domainname: error setting domain name (errno={})", err);
            }
            process::exit(1);
        }
    }
}
