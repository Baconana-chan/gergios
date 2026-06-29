//! Rust port of the MINIX/NetBSD `units` utility.
//!
//! Usage:
//!   units [-f unitsfile] [-H] [-q] [from-unit to-unit]
//!
//! Converts between units of measurement using a built-in database.

use std::collections::HashMap;
use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut quiet = false;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        for ch in opt.chars().skip(1) {
            match ch {
                'q' => quiet = true,
                'H' | 'f' => {
                    if ch == 'f' {
                        argv = &argv[1..];
                        if argv.is_empty() { eprintln!("usage: units [-f file] [-q] [from to]"); std::process::exit(1); }
                    }
                    argv = &argv[1..];
                    break;
                }
                _ => { eprintln!("usage: units [-f file] [-q] [from to]"); std::process::exit(1); }
            }
        }
        argv = &argv[1..];
    }

    let db = build_units_db();
    let interactive = argv.len() < 2;

    if interactive {
        loop {
            let mut input = String::new();
            if !quiet { print!("You have: "); }
            std::io::stdout().flush().ok();
            if std::io::stdin().read_line(&mut input).ok().unwrap_or(0) == 0 { break; }
            let from = input.trim().to_lowercase();
            if from.is_empty() { break; }

            let mut input2 = String::new();
            if !quiet { print!("You want: "); }
            std::io::stdout().flush().ok();
            if std::io::stdin().read_line(&mut input2).ok().unwrap_or(0) == 0 { break; }
            let to = input2.trim().to_lowercase();
            if to.is_empty() { break; }

            convert(&from, &to, &db, true);
        }
    } else {
        let from = argv[0].to_lowercase();
        let to = argv[1].to_lowercase();
        convert(&from, &to, &db, !quiet);
    }
}

fn convert(from: &str, to: &str, db: &HashMap<String, f64>, verbose: bool) {
    let from_val = resolve_unit(from, db);
    let to_val = resolve_unit(to, db);

    match (from_val, to_val) {
        (Some(f), Some(t)) if t != 0.0 => {
            let result = f / t;
            if verbose {
                println!("\t* {:.10}", result);
                println!("\t/ {:.10}", 1.0 / result);
            } else {
                println!("{:.10}", result);
            }
        }
        _ => {
            if verbose {
                if from_val.is_none() { eprintln!("Unknown unit: {from}"); }
                if to_val.is_none() { eprintln!("Unknown unit: {to}"); }
            }
            if !verbose { println!("0"); }
        }
    }
}

fn resolve_unit(name: &str, db: &HashMap<String, f64>) -> Option<f64> {
    if let Some(&val) = db.get(name) { return Some(val); }
    let plural = format!("{}s", name);
    if let Some(&val) = db.get(&plural) { return Some(val); }

    let prefixes = ["micro", "milli", "centi", "deci", "deka", "hecto", "kilo", "mega", "giga", "tera"];
    for prefix in &prefixes {
        if let Some(stripped) = name.strip_prefix(prefix) {
            let factor = match *prefix {
                "micro" => 1e-6,
                "milli" => 1e-3,
                "centi" => 1e-2,
                "deci" => 1e-1,
                "deka" => 1e1,
                "hecto" => 1e2,
                "kilo" => 1e3,
                "mega" => 1e6,
                "giga" => 1e9,
                "tera" => 1e12,
                _ => 1.0,
            };
            return resolve_unit(stripped, db).map(|v| v * factor);
        }
    }

    None
}

fn build_units_db() -> HashMap<String, f64> {
    let mut db = HashMap::new();

    // Base units
    db.insert("meter".to_string(), 1.0);
    db.insert("metre".to_string(), 1.0);
    db.insert("m".to_string(), 1.0);
    db.insert("second".to_string(), 1.0);
    db.insert("s".to_string(), 1.0);
    db.insert("gram".to_string(), 1.0);
    db.insert("g".to_string(), 1.0);
    db.insert("radian".to_string(), 1.0);
    db.insert("rad".to_string(), 1.0);

    // Length
    db.insert("inch".to_string(), 0.0254);
    db.insert("in".to_string(), 0.0254);
    db.insert("foot".to_string(), 0.3048);
    db.insert("ft".to_string(), 0.3048);
    db.insert("feet".to_string(), 0.3048);
    db.insert("yard".to_string(), 0.9144);
    db.insert("yd".to_string(), 0.9144);
    db.insert("mile".to_string(), 1609.344);
    db.insert("mi".to_string(), 1609.344);
    db.insert("nauticalmile".to_string(), 1852.0);
    db.insert("nmi".to_string(), 1852.0);
    db.insert("fathom".to_string(), 1.8288);
    db.insert("chain".to_string(), 20.1168);
    db.insert("furlong".to_string(), 201.168);
    db.insert("angstrom".to_string(), 1e-10);
    db.insert("au".to_string(), 1.495978707e11);
    db.insert("lightyear".to_string(), 9.4607304725808e15);
    db.insert("parsec".to_string(), 3.085677581e16);
    db.insert("pc".to_string(), 3.085677581e16);
    db.insert("micron".to_string(), 1e-6);

    // Area
    db.insert("acre".to_string(), 4046.8564224);
    db.insert("hectare".to_string(), 10000.0);
    db.insert("ha".to_string(), 10000.0);

    // Volume
    db.insert("liter".to_string(), 0.001);
    db.insert("litre".to_string(), 0.001);
    db.insert("l".to_string(), 0.001);
    db.insert("gallon".to_string(), 0.003785411784);
    db.insert("gal".to_string(), 0.003785411784);
    db.insert("quart".to_string(), 0.000946352946);
    db.insert("qt".to_string(), 0.000946352946);
    db.insert("pint".to_string(), 0.000473176473);
    db.insert("pt".to_string(), 0.000473176473);
    db.insert("cup".to_string(), 0.0002365882365);
    db.insert("fluidounce".to_string(), 2.95735295625e-5);
    db.insert("floz".to_string(), 2.95735295625e-5);
    db.insert("tablespoon".to_string(), 1.478676478125e-5);
    db.insert("tbsp".to_string(), 1.478676478125e-5);
    db.insert("teaspoon".to_string(), 4.92892159375e-6);
    db.insert("tsp".to_string(), 4.92892159375e-6);

    // Mass
    db.insert("kilogram".to_string(), 1000.0);
    db.insert("kg".to_string(), 1000.0);
    db.insert("pound".to_string(), 453.59237);
    db.insert("lb".to_string(), 453.59237);
    db.insert("ounce".to_string(), 28.349523125);
    db.insert("oz".to_string(), 28.349523125);
    db.insert("ton".to_string(), 907184.74);
    db.insert("metricton".to_string(), 1000000.0);
    db.insert("tonne".to_string(), 1000000.0);

    // Time
    db.insert("minute".to_string(), 60.0);
    db.insert("min".to_string(), 60.0);
    db.insert("hour".to_string(), 3600.0);
    db.insert("hr".to_string(), 3600.0);
    db.insert("day".to_string(), 86400.0);
    db.insert("week".to_string(), 604800.0);
    db.insert("year".to_string(), 31557600.0);
    db.insert("yr".to_string(), 31557600.0);

    // Speed
    db.insert("kph".to_string(), 0.2777777777777778);
    db.insert("kmh".to_string(), 0.2777777777777778);
    db.insert("mph".to_string(), 0.44704);
    db.insert("knot".to_string(), 0.5144444444444445);
    db.insert("kt".to_string(), 0.5144444444444445);

    // Force
    db.insert("newton".to_string(), 1.0);
    db.insert("n".to_string(), 1.0);
    db.insert("dyne".to_string(), 1e-5);
    db.insert("poundforce".to_string(), 4.4482216152605);
    db.insert("lbf".to_string(), 4.4482216152605);

    // Energy
    db.insert("joule".to_string(), 1.0);
    db.insert("j".to_string(), 1.0);
    db.insert("calorie".to_string(), 4.184);
    db.insert("cal".to_string(), 4.184);
    db.insert("btu".to_string(), 1055.06);
    db.insert("ev".to_string(), 1.602176634e-19);
    db.insert("electronvolt".to_string(), 1.602176634e-19);
    db.insert("kwh".to_string(), 3600000.0);

    // Power
    db.insert("watt".to_string(), 1.0);
    db.insert("w".to_string(), 1.0);
    db.insert("horsepower".to_string(), 745.69987158227022);
    db.insert("hp".to_string(), 745.69987158227022);

    // Pressure
    db.insert("pascal".to_string(), 1.0);
    db.insert("pa".to_string(), 1.0);
    db.insert("bar".to_string(), 100000.0);
    db.insert("atm".to_string(), 101325.0);
    db.insert("torr".to_string(), 133.32236842105263);
    db.insert("psi".to_string(), 6894.75729316836);
    db.insert("mmhg".to_string(), 133.32236842105263);

    // Computer
    db.insert("byte".to_string(), 1.0);
    db.insert("b".to_string(), 1.0);
    db.insert("bit".to_string(), 0.125);
    db.insert("kilobyte".to_string(), 1000.0);
    db.insert("kb".to_string(), 1000.0);
    db.insert("kibibyte".to_string(), 1024.0);
    db.insert("kib".to_string(), 1024.0);
    db.insert("megabyte".to_string(), 1000000.0);
    db.insert("mb".to_string(), 1000000.0);
    db.insert("mebibyte".to_string(), 1048576.0);
    db.insert("mib".to_string(), 1048576.0);
    db.insert("gigabyte".to_string(), 1000000000.0);
    db.insert("gb".to_string(), 1000000000.0);
    db.insert("gibibyte".to_string(), 1073741824.0);
    db.insert("gib".to_string(), 1073741824.0);
    db.insert("terabyte".to_string(), 1000000000000.0);
    db.insert("tb".to_string(), 1000000000000.0);
    db.insert("tebibyte".to_string(), 1099511627776.0);
    db.insert("tib".to_string(), 1099511627776.0);

    // Misc
    db.insert("pi".to_string(), std::f64::consts::PI);
    db.insert("dozen".to_string(), 12.0);
    db.insert("score".to_string(), 20.0);
    db.insert("percent".to_string(), 0.01);
    db.insert("%".to_string(), 0.01);
    db.insert("pair".to_string(), 2.0);

    db
}
