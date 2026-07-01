use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ext4_core::*;

fn make_sb() -> Ext4Superblock {
    let mut data = vec![0u8; 1024];
    data[56..58].copy_from_slice(&EXT4_SUPER_MAGIC.to_le_bytes());
    data[24..28].copy_from_slice(&(2u32).to_le_bytes());
    data[32..36].copy_from_slice(&(32768u32).to_le_bytes());
    data[40..44].copy_from_slice(&(8192u32).to_le_bytes());
    data[88..90].copy_from_slice(&(256u16).to_le_bytes());
    data[76..80].copy_from_slice(&(1u32).to_le_bytes());
    data[84..88].copy_from_slice(&(11u32).to_le_bytes());
    data[96..100].copy_from_slice(&(EXT4_FEATURE_INCOMPAT_FILETYPE | EXT4_FEATURE_INCOMPAT_EXTENTS | EXT4_FEATURE_INCOMPAT_FLEX_BG).to_le_bytes());
    data[100..104].copy_from_slice(&EXT4_FEATURE_RO_COMPAT_SPARSE_SUPER.to_le_bytes());
    parse_superblock(&data).unwrap()
}

/// Build a raw directory block with `count` entries
fn build_dir_block(count: usize) -> Vec<u8> {
    let block_size = 4096;
    let mut block = vec![0u8; block_size];
    let mut offset = 0usize;
    for i in 0..count {
        let name = format!("file{:04}.txt", i);
        let name_len = name.len() as u8;
        let entry_size = 8 + name_len as usize;
        let rec_len = if i == count - 1 {
            block_size - offset
        } else {
            ((entry_size + 3) / 4) * 4
        };
        block[offset..offset+4].copy_from_slice(&(1000 + i as u32).to_le_bytes());
        block[offset+4..offset+6].copy_from_slice(&(rec_len as u16).to_le_bytes());
        block[offset+6] = name_len;
        block[offset+7] = 1; // file_type = REG_FILE
        block[offset+8..offset+8+name_len as usize].copy_from_slice(name.as_bytes());
        offset += rec_len;
        if offset >= block_size - 8 { break; }
    }
    block
}

fn bench_lookup_linear_small(c: &mut Criterion) {
    let block = build_dir_block(16);
    c.bench_function("lookup_linear_16_entries", |b| {
        b.iter(|| {
            let iter = DirEntryIter::new(black_box(&block));
            let mut found = false;
            for entry in iter {
                if entry.name.starts_with(b"file015") {
                    found = true;
                    break;
                }
            }
            black_box(found);
        })
    });
}

fn bench_lookup_linear_large(c: &mut Criterion) {
    let block = build_dir_block(200);
    c.bench_function("lookup_linear_200_entries", |b| {
        b.iter(|| {
            let iter = DirEntryIter::new(black_box(&block));
            let mut found = false;
            for entry in iter {
                if entry.name.starts_with(b"file199") {
                    found = true;
                    break;
                }
            }
            black_box(found);
        })
    });
}

fn bench_file_type_to_mode(c: &mut Criterion) {
    c.bench_function("file_type_to_mode", |b| {
        b.iter(|| {
            for ft in 0..8 {
                let mode = file_type_to_mode(black_box(ft));
                black_box(mode);
            }
        })
    });
}

fn bench_insert_into_block(c: &mut Criterion) {
    let mut block = build_dir_block(16);
    c.bench_function("insert_into_block", |b| {
        b.iter(|| {
            let mut buf = block.clone();
            let ok = insert_into_block(black_box(&mut buf), black_box(9999), black_box("newfile.txt"), black_box(1));
            black_box(ok);
        })
    });
}

fn bench_remove_from_block(c: &mut Criterion) {
    let block = build_dir_block(16);
    c.bench_function("remove_from_block", |b| {
        b.iter(|| {
            let mut buf = block.clone();
            let ok = remove_from_block(black_box(&mut buf), black_box("file000.txt"));
            black_box(ok);
        })
    });
}

criterion_group!(dir,
    bench_lookup_linear_small,
    bench_lookup_linear_large,
    bench_file_type_to_mode,
    bench_insert_into_block,
    bench_remove_from_block,
);
criterion_main!(dir);
