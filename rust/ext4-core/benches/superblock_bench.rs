use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ext4_core::{
    parse_superblock,
    EXT4_SUPER_MAGIC, EXT4_FEATURE_INCOMPAT_FILETYPE, EXT4_FEATURE_INCOMPAT_EXTENTS,
    EXT4_FEATURE_INCOMPAT_FLEX_BG, EXT4_FEATURE_RO_COMPAT_SPARSE_SUPER,
    EXT4_FEATURE_RO_COMPAT_LARGE_FILE, EXT4_FEATURE_RO_COMPAT_GDT_CSUM,
    EXT4_FEATURE_RO_COMPAT_DIR_NLINK, EXT4_FEATURE_RO_COMPAT_EXTRA_ISIZE,
};

fn create_test_superblock() -> Vec<u8> {
    let mut sb = vec![0u8; 1024];
    sb[0..4].copy_from_slice(&(1024u32).to_le_bytes());       // s_inodes_count
    sb[4..8].copy_from_slice(&(819200u32).to_le_bytes());     // s_blocks_count_lo
    sb[12..16].copy_from_slice(&(789200u32).to_le_bytes());   // s_free_blocks_count_lo
    sb[16..20].copy_from_slice(&(824u32).to_le_bytes());      // s_free_inodes_count
    sb[20..24].copy_from_slice(&(0u32).to_le_bytes());        // s_first_data_block
    sb[24..28].copy_from_slice(&(2u32).to_le_bytes());        // s_log_block_size
    sb[28..32].copy_from_slice(&(2u32).to_le_bytes());        // s_log_cluster_size
    sb[32..36].copy_from_slice(&(32768u32).to_le_bytes());    // s_blocks_per_group
    sb[36..40].copy_from_slice(&(32768u32).to_le_bytes());    // s_clusters_per_group
    sb[40..44].copy_from_slice(&(8192u32).to_le_bytes());     // s_inodes_per_group
    sb[56..58].copy_from_slice(&EXT4_SUPER_MAGIC.to_le_bytes()); // magic
    sb[58..60].copy_from_slice(&(1u16).to_le_bytes());        // s_state
    sb[76..80].copy_from_slice(&(1u32).to_le_bytes());        // s_rev_level
    sb[84..88].copy_from_slice(&(11u32).to_le_bytes());       // s_first_ino
    sb[88..90].copy_from_slice(&(256u16).to_le_bytes());      // s_inode_size
    let incompat = EXT4_FEATURE_INCOMPAT_FILETYPE | EXT4_FEATURE_INCOMPAT_EXTENTS | EXT4_FEATURE_INCOMPAT_FLEX_BG;
    sb[96..100].copy_from_slice(&incompat.to_le_bytes());
    let ro_compat = EXT4_FEATURE_RO_COMPAT_SPARSE_SUPER | EXT4_FEATURE_RO_COMPAT_LARGE_FILE
        | EXT4_FEATURE_RO_COMPAT_GDT_CSUM | EXT4_FEATURE_RO_COMPAT_DIR_NLINK | EXT4_FEATURE_RO_COMPAT_EXTRA_ISIZE;
    sb[100..104].copy_from_slice(&ro_compat.to_le_bytes());
    sb[286..288].copy_from_slice(&(64u16).to_le_bytes());     // s_desc_size
    sb
}

fn bench_parse_superblock(c: &mut Criterion) {
    let sb_data = create_test_superblock();
    c.bench_function("parse_superblock", |b| {
        b.iter(|| {
            let result = parse_superblock(black_box(&sb_data));
            black_box(result.unwrap());
        })
    });
}

criterion_group!(superblock, bench_parse_superblock);
criterion_main!(superblock);
