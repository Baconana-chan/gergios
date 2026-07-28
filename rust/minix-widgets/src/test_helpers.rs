//! Test helpers for widget tests.
//!
//! Provides a minimal valid TrueType font so that text-based widget
//! tests (Button, Label, etc.) can actually render text.

/// Build a minimal valid TrueType font for testing.
/// Based on the same approach used in minix-compositor's font tests.
pub fn minimal_ttf() -> alloc::vec::Vec<u8> {
    let upem: u16 = 1000;

    // head table (54 bytes)
    let head = {
        let mut d = alloc::vec::Vec::new();
        d.extend_from_slice(&u32::to_be_bytes(0x00010000));
        d.extend_from_slice(&u32::to_be_bytes(0));
        d.extend_from_slice(&u32::to_be_bytes(0));
        d.extend_from_slice(&u32::to_be_bytes(0x5F0F3CF5));
        d.extend_from_slice(&u16::to_be_bytes(0));
        d.extend_from_slice(&u16::to_be_bytes(upem));
        d.extend_from_slice(&i64::to_be_bytes(0));
        d.extend_from_slice(&i64::to_be_bytes(0));
        d.extend_from_slice(&i16::to_be_bytes(-100));
        d.extend_from_slice(&i16::to_be_bytes(0));
        d.extend_from_slice(&i16::to_be_bytes(1000));
        d.extend_from_slice(&i16::to_be_bytes(1000));
        d.extend_from_slice(&u16::to_be_bytes(0));
        d.extend_from_slice(&u16::to_be_bytes(3));
        d.extend_from_slice(&i16::to_be_bytes(2));
        d.extend_from_slice(&i16::to_be_bytes(0));
        d.extend_from_slice(&i16::to_be_bytes(0));
        d
    };

    // hhea table (36 bytes)
    let hhea = {
        let mut d = alloc::vec::Vec::new();
        d.extend_from_slice(&u32::to_be_bytes(0x00010000));
        d.extend_from_slice(&i16::to_be_bytes(800));
        d.extend_from_slice(&i16::to_be_bytes(-200));
        d.extend_from_slice(&i16::to_be_bytes(0));
        d.extend_from_slice(&u16::to_be_bytes(0));
        d.extend_from_slice(&i16::to_be_bytes(0));
        d.extend_from_slice(&i16::to_be_bytes(0));
        d.extend_from_slice(&i16::to_be_bytes(1000));
        d.extend_from_slice(&i16::to_be_bytes(1));
        d.extend_from_slice(&i16::to_be_bytes(0));
        d.extend_from_slice(&i16::to_be_bytes(0));
        d.extend_from_slice(&[0u8; 8]);
        d.extend_from_slice(&i16::to_be_bytes(0));
        d.extend_from_slice(&u16::to_be_bytes(1));
        d
    };

    // maxp table (32 bytes)
    let maxp = {
        let mut d = alloc::vec![0u8; 32];
        d[0..4].copy_from_slice(&u32::to_be_bytes(0x00010000));
        d[4..6].copy_from_slice(&u16::to_be_bytes(1));
        d
    };

    // cmap table: Format 0
    let cmap = {
        let mut d = alloc::vec::Vec::new();
        d.extend_from_slice(&u16::to_be_bytes(0));
        d.extend_from_slice(&u16::to_be_bytes(1));
        d.extend_from_slice(&u16::to_be_bytes(3));
        d.extend_from_slice(&u16::to_be_bytes(1));
        d.extend_from_slice(&u32::to_be_bytes(12));
        d.extend_from_slice(&u16::to_be_bytes(0));
        d.extend_from_slice(&u16::to_be_bytes(262));
        d.extend_from_slice(&u16::to_be_bytes(0));
        d.extend_from_slice(&[0u8; 256]);
        d
    };

    // hmtx table (4 bytes)
    let hmtx = {
        let mut d = alloc::vec::Vec::new();
        d.extend_from_slice(&u16::to_be_bytes(500));
        d.extend_from_slice(&i16::to_be_bytes(0));
        d
    };

    // glyf table with triangle glyph (29+ bytes)
    let glyf = {
        let mut d = alloc::vec::Vec::new();
        d.extend_from_slice(&1i16.to_be_bytes());   // numberOfContours = 1
        d.extend_from_slice(&100i16.to_be_bytes()); // xMin
        d.extend_from_slice(&100i16.to_be_bytes()); // yMin
        d.extend_from_slice(&900i16.to_be_bytes()); // xMax
        d.extend_from_slice(&900i16.to_be_bytes()); // yMax
        d.extend_from_slice(&2u16.to_be_bytes());   // endPtsOfContours[0] = 2
        d.extend_from_slice(&0u16.to_be_bytes());   // instructionLength = 0
        d.push(0x01); d.push(0x01); d.push(0x01); // flags: on-curve
        d.extend_from_slice(&100i16.to_be_bytes()); // x1
        d.extend_from_slice(&400i16.to_be_bytes()); // dx2
        d.extend_from_slice(&400i16.to_be_bytes()); // dx3
        d.extend_from_slice(&100i16.to_be_bytes()); // y1
        d.extend_from_slice(&800i16.to_be_bytes()); // dy2
        d.extend_from_slice(&(-800i16).to_be_bytes()); // dy3
        if d.len() % 2 != 0 { d.push(0); }
        d
    };

    let glyf_loca_words = ((glyf.len() + 1) / 2) as u16;
    let loca = {
        let mut d = alloc::vec::Vec::new();
        d.extend_from_slice(&u16::to_be_bytes(0));
        d.extend_from_slice(&u16::to_be_bytes(glyf_loca_words));
        d
    };

    // name table (8 bytes, empty)
    let name = alloc::vec![0u8; 8];

    // OS/2 table (78 bytes)
    let os2 = {
        let mut d = alloc::vec![0u8; 78];
        d[0..2].copy_from_slice(&u16::to_be_bytes(4));
        d[2..4].copy_from_slice(&u16::to_be_bytes(500));
        d[4..6].copy_from_slice(&u16::to_be_bytes(400));
        d[6..8].copy_from_slice(&u16::to_be_bytes(5));
        d
    };

    // post table (32 bytes)
    let post = {
        let mut d = alloc::vec![0u8; 32];
        d[0..4].copy_from_slice(&u32::to_be_bytes(0x00030000));
        d
    };

    let tables: alloc::vec::Vec<(&[u8], &[u8])> = alloc::vec![
        (b"OS/2", &os2), (b"cmap", &cmap), (b"glyf", &glyf),
        (b"head", &head), (b"hhea", &hhea), (b"hmtx", &hmtx),
        (b"loca", &loca), (b"maxp", &maxp), (b"name", &name),
        (b"post", &post),
    ];

    #[derive(Clone, Copy)]
    struct TableEntry {
        tag: &'static [u8], offset: u32, len: u32, padded_len: u32,
    }
    let mut meta = alloc::vec::Vec::with_capacity(tables.len());
    let mut cursor = 12u32 + tables.len() as u32 * 16;
    for &(tag, data) in &tables {
        let len = data.len() as u32;
        let padded_len = ((len + 3) / 4) * 4;
        meta.push(TableEntry { tag, offset: cursor, len, padded_len });
        cursor += padded_len;
    }

    let mut font = alloc::vec::Vec::new();
    font.extend_from_slice(&u32::to_be_bytes(0x00010000));
    font.extend_from_slice(&u16::to_be_bytes(tables.len() as u16));
    font.extend_from_slice(&u16::to_be_bytes(0));
    font.extend_from_slice(&u16::to_be_bytes(0));
    font.extend_from_slice(&u16::to_be_bytes(0));
    for e in &meta {
        font.extend_from_slice(e.tag);
        font.extend_from_slice(&u32::to_be_bytes(0)); // checksum (not needed for tests)
        font.extend_from_slice(&u32::to_be_bytes(e.offset));
        font.extend_from_slice(&u32::to_be_bytes(e.len));
    }
    for e in &meta {
        let data = tables.iter().find(|t| t.0 == e.tag).unwrap().1;
        font.extend_from_slice(data);
        while font.len() < (e.offset + e.padded_len) as usize {
            font.push(0);
        }
    }
    font
}
