use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ext4_core::*;

/// Create a raw journal superblock buffer
fn make_journal_sb_buf() -> Vec<u8> {
    let mut data = vec![0u8; 1024];
    data[0..4].copy_from_slice(&0xC03B3998u32.to_be_bytes());
    data[4..8].copy_from_slice(&4u32.to_be_bytes()); // SUPERBLOCK_V2
    data[8..12].copy_from_slice(&1u32.to_be_bytes());
    data[12..16].copy_from_slice(&(4096u32).to_be_bytes()); // s_blocksize
    data[16..20].copy_from_slice(&(1024u32).to_be_bytes()); // s_maxlen
    data[20..24].copy_from_slice(&(1u32).to_be_bytes());    // s_first
    data[24..28].copy_from_slice(&(1u32).to_be_bytes());    // s_sequence
    data[28..32].copy_from_slice(&(0u32).to_be_bytes());    // s_start
    data
}

fn bench_parse_journal_superblock(c: &mut Criterion) {
    let data = make_journal_sb_buf();
    c.bench_function("parse_journal_superblock", |b| {
        b.iter(|| {
            let result = parse_journal_superblock(black_box(&data));
            black_box(result.unwrap());
        })
    });
}

fn bench_scan_journal_block_descriptor(c: &mut Criterion) {
    let mut data = vec![0u8; 1024];
    data[0..4].copy_from_slice(&0xC03B3998u32.to_be_bytes());
    data[4..8].copy_from_slice(&1u32.to_be_bytes()); // DESCRIPTOR_BLOCK
    data[8..12].copy_from_slice(&1u32.to_be_bytes());
    data[12..16].copy_from_slice(&(1000u32).to_be_bytes());
    data[16..18].copy_from_slice(&(8u16).to_be_bytes()); // LAST_TAG | SAME_UUID
    c.bench_function("scan_descriptor_block", |b| {
        b.iter(|| {
            let result = scan_journal_block(black_box(&data), black_box(1), black_box(false), black_box(None));
            black_box(result);
        })
    });
}

fn bench_crc32c(c: &mut Criterion) {
    let data = [0u8; 4096];
    c.bench_function("crc32c_4k", |b| {
        b.iter(|| {
            let crc = crc32c_le(0xFFFFFFFF, black_box(&data));
            black_box(crc);
        })
    });
}

fn bench_crc32c_small(c: &mut Criterion) {
    let data = b"hello world, this is a test string for crc32c calculation";
    c.bench_function("crc32c_small", |b| {
        b.iter(|| {
            let crc = crc32c_le(0xFFFFFFFF, black_box(data));
            black_box(crc);
        })
    });
}

fn bench_journal_info_string(c: &mut Criterion) {
    let data = make_journal_sb_buf();
    let sb = parse_journal_superblock(&data).unwrap();
    c.bench_function("journal_info_string", |b| {
        b.iter(|| {
            let s = black_box(&sb).info_string();
            black_box(s);
        })
    });
}

criterion_group!(journal,
    bench_parse_journal_superblock,
    bench_scan_journal_block_descriptor,
    bench_crc32c,
    bench_crc32c_small,
    bench_journal_info_string,
);
criterion_main!(journal);
