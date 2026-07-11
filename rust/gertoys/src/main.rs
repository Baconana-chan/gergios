// gertoys — Consolidated BSD games utilities (bcd, morse, caesar,
// number, pig, ppt, random, arithmetic, factor, banner, fortune).
//
// Usage:
//   gertoys <subcommand> [args...]

mod arithmetic;
mod banner;
mod bcd;
mod caesar;
mod factor;
mod fortune;
mod morse;
mod number;
mod pig;
mod ppt;
mod r#random;

fn print_usage() {
    eprintln!("Usage: gertoys <subcommand> [args...]");
    eprintln!();
    eprintln!("Subcommands:");
    eprintln!("  arithmetic Interactive arithmetic quiz");
    eprintln!("  banner     Print large ASCII banners");
    eprintln!("  bcd        Print text as a punch card");
    eprintln!("  caesar     Caesar cipher (auto-guess rotation)");
    eprintln!("  factor     Factor numbers into primes");
    eprintln!("  fortune    Display a random adage");
    eprintln!("  morse      Encode/decode Morse code");
    eprintln!("  number     Convert numbers to English words");
    eprintln!("  pig        Convert text to Pig Latin");
    eprintln!("  ppt        Print/read paper tape");
    eprintln!("  random     Random line filter or exit code");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        std::process::exit(1);
    }

    let subcmd = &args[1];
    let sub_args: Vec<String> = args[2..].to_vec();

    match subcmd.as_str() {
        "arithmetic" => arithmetic::run(&sub_args),
        "banner" => banner::run(&sub_args),
        "bcd" => bcd::run(&sub_args),
        "caesar" => caesar::run(&sub_args),
        "factor" => factor::run(&sub_args),
        "fortune" => fortune::run(&sub_args),
        "morse" => morse::run(&sub_args),
        "number" => number::run(&sub_args),
        "pig" => pig::run(&sub_args),
        "ppt" => ppt::run(&sub_args),
        "random" => r#random::run(&sub_args),
        _ => {
            eprintln!("gertoys: unknown subcommand '{}'", subcmd);
            print_usage();
            std::process::exit(1);
        }
    }
}
