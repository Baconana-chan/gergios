use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ext4_core::*;

fn make_sb() -> Ext4Superblock {
    let mut data = vec![0u8; 1024];
    data[56..58].copy_from_slice(&EXT4_SUPER_MAGIC.to_le_bytes());
    data[24..28].copy_from_slice(&(2u32).to_le_bytes());   // s_log_block_size = 4096
    data[32..36].copy_from_slice(&(32768u32).to_le_bytes()); // s_blocks_per_group
    data[40..44].copy_from_slice(&(8192u32).to_le_bytes()); // s_inodes_per_group
    data[96..100].copy_from_slice(&(EXT4_FEATURE_INCOMPAT_FILETYPE | EXT4_FEATURE_INCOMPAT_EXTENTS | EXT4_FEATURE_INCOMPAT_FLEX_BG).to_le_bytes());
    data[100..104].copy_from_slice(&EXT4_FEATURE_RO_COMPAT_SPARSE_SUPER.to_le_bytes());
    data[76..80].copy_from_slice(&(1u32).to_le_bytes());   // s_rev_level
    data[88..90].copy_from_slice(&(256u16).to_le_bytes()); // s_inode_size
    data[84..88].copy_from_slice(&(11u32).to_le_bytes());  // s_first_ino
    parse_superblock(&data).unwrap()
}

/// Create a mock inode with an extent tree containing extents
fn make_extent_inode(count: usize) -> Ext4Inode {
    let mut inode = new_inode(EXT4_S_IFREG | 0o644, 0, 0);
    init_extent_tree(&mut inode);
    
    // Build extent data manually in i_block
    let header = Ext4ExtentHeader {
        eh_magic: EXT4_EXTENT_MAGIC,
        eh_entries: count.min(4) as u16,
        eh_max: 4,
        eh_depth: 0,
        eh_generation: 0,
    };
    let mut i_block = [0u8; 60];
    serialize_header(&mut i_block, &header);
    for i in 0..count.min(4) {
        let extent = Ext4Extent {
            ee_block: (i * 1024) as u32,
            ee_len: 128,
            ee_start_hi: 0,
            ee_start_lo: (1000 + i as u32 * 128),
        };
        serialize_extent(&mut i_block, 12 + i * 12, &extent);
    }
    inode.i_block = i_block;
    inode
}

fn bench_extent_header_parse(c: &mut Criterion) {
    let inode = make_extent_inode(4);
    c.bench_function("extent_header_parse", |b| {
        b.iter(|| {
            let hdr = black_box(&inode).extent_header();
            black_box(hdr.unwrap());
        })
    });
}

fn bench_extent_lookup(c: &mut Criterion) {
    let sb = make_sb();
    let inode = make_extent_inode(4);
    let mut read_called = false;
    c.bench_function("extent_lookup_inline", |b| {
        b.iter(|| {
            let result = extent_lookup(black_box(&sb), black_box(&inode), black_box(500), |_block, _buf| {
                read_called = true;
                Err(Ext4Error::IoError)
            });
            black_box(result);
        })
    });
}

fn bench_serialize_extent_header(c: &mut Criterion) {
    let header = Ext4ExtentHeader {
        eh_magic: EXT4_EXTENT_MAGIC,
        eh_entries: 4,
        eh_max: 4,
        eh_depth: 0,
        eh_generation: 0,
    };
    let mut buf = [0u8; 12];
    c.bench_function("serialize_extent_header", |b| {
        b.iter(|| {
            serialize_header(black_box(&mut buf), black_box(&header));
            black_box(buf);
        })
    });
}

fn bench_serialize_extent(c: &mut Criterion) {
    let extent = Ext4Extent {
        ee_block: 0,
        ee_len: 128,
        ee_start_hi: 0,
        ee_start_lo: 256,
    };
    let mut buf = [0u8; 12];
    c.bench_function("serialize_single_extent", |b| {
        b.iter(|| {
            serialize_extent(black_box(&mut buf), black_box(0), black_box(&extent));
            black_box(buf);
        })
    });
}

criterion_group!(extent,
    bench_extent_header_parse,
    bench_extent_lookup,
    bench_serialize_extent_header,
    bench_serialize_extent,
);
criterion_main!(extent);
