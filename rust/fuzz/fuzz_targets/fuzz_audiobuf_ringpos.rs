#![no_main]

use audio_buf::{RingPos, try_transfer};
use libfuzzer_sys::fuzz_target;

/// Fuzz the RingPos circular buffer implementation.
///
/// Exercises advance_fill, advance_read, try_transfer with arbitrary inputs
/// to ensure no panics or out-of-bounds access.
fuzz_target!(|data: &[u8]| {
    if data.len() < 4 {
        return;
    }

    let nr_fragments = (data[0] as usize % 256).max(1);
    let mut pos = match RingPos::new(nr_fragments) {
        Some(p) => p,
        None => return,
    };

    // Perform random fill/read operations based on input bytes
    for &b in &data[1..] {
        match b % 4 {
            0 => { let _ = pos.advance_fill(); }
            1 => { let _ = pos.advance_read(); }
            2 => { let _ = pos.peek_read(); }
            3 => {
                let mut src = match RingPos::new(nr_fragments) {
                    Some(p) => p,
                    None => continue,
                };
                src.advance_fill(); // put one fragment in
                let mut dst = RingPos::new(nr_fragments).unwrap();
                let _ = try_transfer(&mut src, &mut dst);
            }
            _ => unreachable!(),
        }
        // Invariants must hold
        assert!(pos.len() <= pos.capacity());
        assert_eq!(pos.is_empty(), pos.len() == 0);
        assert_eq!(pos.is_full(), pos.len() == pos.capacity());
    }
});
