// fortune — Print a random, interesting adage.
//
// Simplified Rust port of BSD games/fortune.
// Embeds a curated set of classic fortunes rather than reading
// from external fortune database files.
//
// Usage:
//   gertoys fortune [-s] [-l]

use std::time::{SystemTime, UNIX_EPOCH};

/// Curated collection of classic Unix fortunes.
const FORTUNES: &[&str] = &[
    "The fewer the facts, the stronger the opinion.",
    "A day without sunshine is like, you know, night.",
    "Experience is something you don't get until just after you need it.",
    "Never test the depth of the water with both feet.",
    "If you can't convince them, confuse them.",
    "The early bird gets the worm, but the second mouse gets the cheese.",
    "A conclusion is simply the place where you got tired of thinking.",
    "Artificial intelligence is no match for natural stupidity.",
    "The best way to accelerate a computer is to boot it up at 9 am.",
    "There are 10 types of people in the world: those who understand binary, and those who don't.",
    "UNIX is user-friendly. It's just very particular about who its friends are.",
    "Everything should be made as simple as possible, but no simpler.",
    "The difference between theory and practice is that in theory, there is no difference.",
    "Programming today is a race between software engineers striving to build bigger and better idiot-proof programs, and the Universe trying to produce bigger and better idiots. So far, the Universe is winning.",
    "In a world without walls and fences, who needs Windows and Gates?",
    "Real programmers don't document. If it was hard to write, it should be hard to understand.",
    "Why do we never have time to do it right, but always have time to do it over?",
    "Any sufficiently advanced bug is indistinguishable from a feature.",
    "The most likely way for the world to be destroyed, most experts agree, is by accident. That's where we come in; we're computer professionals. We cause accidents.",
    "There is no such thing as a \"self-made\" man. You will achieve your goals only with the help of others.",
    "The longest journey begins with a single step — and a map.",
    "If you think education is expensive, try ignorance.",
    "Computer science: the only discipline in which 'viewing the source' is considered a virtue.",
    "The internet: where men are men, women are men, and children are FBI agents.",
    "I'm not anti-social; I'm just not user-friendly.",
    "I before E, except when your foreign neighbor Keith receives eight counterfeit sleighs from feisty caffeinated weightlifters.",
    "Support bacteria — they're the only culture most people have.",
    "What happens if you get scared half to death twice?",
    "Time is what keeps everything from happening at once.",
    "I would rather have questions I can't answer, than answers I can't question.",
    "The problem with troubleshooting is that trouble shoots back.",
    "Don't worry about the world coming to an end today. It's already tomorrow in Australia.",
    "A computer lets you make more mistakes faster than any invention in human history — with the possible exceptions of handguns and tequila.",
    "Facts are stubborn things, but statistics are more pliable.",
    "If debugging is the process of removing bugs, then programming must be the process of putting them in.",
    "It's not a bug — it's an undocumented feature.",
    "I think Microsoft named .Net so it wouldn't show up in a Unix directory listing.",
    "C is quirky, flawed, and an enormous success. While accidents of history surely helped, it evidently satisfied a need for a system implementation language efficient enough to displace assembly language.",
    "A hacker is someone who knows that the perfect is the enemy of the good, and that things sometimes need to be fixed just to keep them working.",
    "The most dangerous phrase in the language is: 'We've always done it this way.'",
    "Any program that runs right is obsolete.",
    "Weeks of programming can save you hours of planning.",
    "The most important property of a program is whether it accomplishes the intention of its user.",
    "One man's constant is another man's variable.",
    "The number of UNIX installations has grown to 10, with more expected.",
    "Unix was not designed to stop you from doing stupid things, because that would also stop you from doing clever things.",
    "It has long been an axiom of mine that the little things are infinitely the most important.",
    "Ninety-ninety rule: The first 90 percent of the code accounts for the first 90 percent of the development time. The remaining 10 percent of the code accounts for the other 90 percent of the development time.",
    "The value of a program is inversely proportional to the weight of its output.",
    "If you have a procedure with ten parameters, you probably missed some.",
    "Some people, when confronted with a problem, think 'I know, I'll use regular expressions.' Now they have two problems.",
    "A programming language is low-level when its programs require attention to the irrelevant.",
    "It is easier to change the specification to fit the program than vice versa.",
    "Beware of bugs in the above code; I have only proved it correct, not tried it.",
    "The best way to predict the future is to invent it.",
    "The key to performance is elegance, not battalions of special cases.",
    "The cheapest, fastest, and most reliable components are those that aren't there.",
    "Always trust a computer scientist to tell you when your idea is impossible — they have a lot of practice.",
    "There are two ways of constructing a software design: one way is to make it so simple that there are obviously no deficiencies, and the other way is to make it so complicated that there are no obvious deficiencies.",
    "The best performance improvement is the transition from the nonworking state to the working state.",
    "The first principle is that you must not fool yourself — and you are the easiest person to fool.",
    "A computer would deserve to be called intelligent if it could deceive a human into believing that it was human.",
    "A LISP programmer knows the value of everything, but the cost of nothing.",
    "I think there is a world market for maybe five computers.",
    "The wonderful thing about standards is that there are so many to choose from.",
    "The term 'user-friendly' is almost always a euphemism for 'completely non-customizable'.",
    "A computer is like air conditioning: it becomes useless when you open Windows.",
    "Perl — The only language that looks the same before and after RSA encryption.",
    "In order to understand recursion, one must first understand recursion.",
    "The best thing about a boolean is that even if you are wrong, you are only off by a bit.",
    "There's no place like 127.0.0.1.",
    "When in doubt, use brute force.",
    "I'm not a complete idiot, some parts are missing.",
    "Talk is cheap. Show me the code.",
    "Before software can be reusable it first has to be usable.",
    "The best code is no code at all.",
    "A good programmer is someone who always looks both ways before crossing a one-way street.",
    "Measuring programming progress by lines of code is like measuring aircraft building progress by weight.",
    "Debugging is twice as hard as writing the code in the first place. Therefore, if you write the code as cleverly as possible, you are, by definition, not smart enough to debug it.",
    "Simplicity is prerequisite for reliability.",
];

fn init_rng() -> u64 {
    let start = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    start.as_nanos() as u64
}

/// Simple fast PRNG (xorshift32).
fn xorshift(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

pub fn run(args: &[String]) {
    let mut short_only = false;
    let mut long_only = false;
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "-s" => {
                short_only = true;
                long_only = false;
            }
            "-l" => {
                long_only = true;
                short_only = false;
            }
            "-h" | "--help" => {
                eprintln!("Usage: gertoys fortune [-s] [-l]");
                eprintln!("  -s    Short fortunes only (< 160 chars)");
                eprintln!("  -l    Long fortunes only (>= 160 chars)");
                return;
            }
            _ => {
                eprintln!("Usage: gertoys fortune [-s] [-l]");
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let mut rng = init_rng();

    // Filter fortunes by length if needed
    if short_only || long_only {
        let filtered: Vec<&&str> = FORTUNES
            .iter()
            .filter(|f| {
                let len = f.len();
                if short_only {
                    len < 160
                } else {
                    len >= 160
                }
            })
            .collect();

        if filtered.is_empty() {
            eprintln!("fortune: no matching fortunes");
            std::process::exit(1);
        }

        let idx = xorshift(&mut rng) as usize % filtered.len();
        println!("{}", filtered[idx]);
    } else {
        // Random fortune
        let idx = xorshift(&mut rng) as usize % FORTUNES.len();
        println!("{}", FORTUNES[idx]);
    }
}
