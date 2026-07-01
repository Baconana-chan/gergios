use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ext4_core::*;

// ─── Xattr benchmarks ──────────────────────────────────────────

fn build_xattr_block(count: usize) -> Vec<u8> {
    let block_size = 4096;
    let mut buf = vec![0u8; block_size];

    // xattr header at offset 0
    buf[0..4].copy_from_slice(&EXT4_XATTR_MAGIC.to_le_bytes());
    buf[4..8].copy_from_slice(&0u32.to_le_bytes());
    buf[8..12].copy_from_slice(&(block_size as u32).to_le_bytes());
    buf[12..16].copy_from_slice(&0u32.to_le_bytes());

    let mut entry_off = 16usize;
    let mut value_off = block_size;

    for i in 0..count {
        let name = format!("user.attr{:03}", i);
        let name_bytes = name.as_bytes();
        let name_len = name_bytes.len() as u8;
        let value = format!("value_{}", i);
        let value_bytes = value.as_bytes();
        let value_size = value_bytes.len() as u32;

        value_off -= value_size as usize;
        value_off &= !3;

        buf[value_off..value_off + value_size as usize].copy_from_slice(value_bytes);

        let entry_size = ((name_len as usize + 3) / 4) * 4 + 12;
        buf[entry_off] = 1;              // e_name_index (user)
        buf[entry_off + 1] = name_len;   // e_name_len
        buf[entry_off + 2..entry_off + 4].copy_from_slice(&(0u16).to_le_bytes());
        buf[entry_off + 4..entry_off + 8].copy_from_slice(&value_size.to_le_bytes());
        buf[entry_off + 8..entry_off + 12].copy_from_slice(&(value_off as u32).to_le_bytes());
        buf[entry_off + 12..entry_off + 12 + name_len as usize].copy_from_slice(name_bytes);
        entry_off += entry_size;
    }

    // Last entry marker
    buf[entry_off] = 0;
    buf[entry_off + 1] = 0;
    buf[entry_off + 2..entry_off + 4].copy_from_slice(&(0u16).to_le_bytes());
    buf[entry_off + 4..entry_off + 8].copy_from_slice(&(0u32).to_le_bytes());
    buf[entry_off + 8..entry_off + 12].copy_from_slice(&(0u32).to_le_bytes());

    buf
}

fn bench_parse_xattrs(c: &mut Criterion) {
    let data = build_xattr_block(64);
    c.bench_function("parse_xattrs_64", |b| {
        b.iter(|| {
            let xattrs = parse_xattrs(black_box(&data), true);
            black_box(xattrs.unwrap());
        })
    });
}

fn bench_match_xattr_name(c: &mut Criterion) {
    c.bench_function("match_xattr_name", |b| {
        b.iter(|| {
            let result = match_xattr_name(black_box("user.my_custom_attr"));
            black_box(result);
        })
    });
}

fn bench_find_xattr(c: &mut Criterion) {
    let data = build_xattr_block(64);
    let xattrs = parse_xattrs(&data, true).unwrap();
    c.bench_function("find_xattr", |b| {
        b.iter(|| {
            let found = find_xattr(black_box(&xattrs), "user.attr031");
            black_box(found);
        })
    });
}

// ─── ACL benchmarks ────────────────────────────────────────────

fn build_acl_data(count: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(8 + count * 8);
    data.extend_from_slice(&EXT4_ACL_VERSION.to_le_bytes());
    for i in 0..count {
        let tag = if i == 0 { ACL_USER_OBJ }
                  else if i == 1 { ACL_GROUP_OBJ }
                  else { ACL_USER };
        data.extend_from_slice(&(tag as u16).to_le_bytes());
        data.extend_from_slice(&0x1FCu16.to_le_bytes());
        data.extend_from_slice(&(i as u32).to_le_bytes());
    }
    data
}

fn bench_parse_acl(c: &mut Criterion) {
    let data = build_acl_data(8);
    c.bench_function("parse_acl_8", |b| {
        b.iter(|| {
            let acl = parse_acl(black_box(&data));
            black_box(acl.unwrap());
        })
    });
}

fn bench_serialize_acl(c: &mut Criterion) {
    let data = build_acl_data(4);
    let acl = parse_acl(&data).unwrap();
    let mut buf = [0u8; 256];
    c.bench_function("serialize_acl", |b| {
        b.iter(|| {
            let out = serialize_acl(black_box(&acl), black_box(&mut buf));
            black_box(out.unwrap());
        })
    });
}

// ─── Quota benchmarks ───────────────────────────────────────────

fn build_dqblk_v2() -> Vec<u8> {
    let mut data = vec![0u8; 72];
    data[0..4].copy_from_slice(&(1000u32).to_le_bytes());
    data[8..16].copy_from_slice(&(5000u64).to_le_bytes());
    data[16..24].copy_from_slice(&(200u64).to_le_bytes());
    data[24..32].copy_from_slice(&(10000u64).to_le_bytes());
    data[32..40].copy_from_slice(&(20000u64).to_le_bytes());
    data[40..48].copy_from_slice(&(500u64).to_le_bytes());
    data[48..56].copy_from_slice(&(1000u64).to_le_bytes());
    data
}

fn bench_parse_dqblk_v2(c: &mut Criterion) {
    let data = build_dqblk_v2();
    c.bench_function("parse_dqblk_v2", |b| {
        b.iter(|| {
            let dqblk = parse_dqblk_v2(black_box(&data));
            black_box(dqblk.unwrap());
        })
    });
}

fn bench_serialize_dqblk_v2(c: &mut Criterion) {
    let data = build_dqblk_v2();
    let dqblk = parse_dqblk_v2(&data).unwrap();
    let mut buf = [0u8; 72];
    c.bench_function("serialize_dqblk_v2", |b| {
        b.iter(|| {
            serialize_dqblk_v2(black_box(&mut buf), black_box(&dqblk));
            black_box(&buf);
        })
    });
}

criterion_group!(xattr_acl_quota,
    bench_parse_xattrs,
    bench_match_xattr_name,
    bench_find_xattr,
    bench_parse_acl,
    bench_serialize_acl,
    bench_parse_dqblk_v2,
    bench_serialize_dqblk_v2,
);
criterion_main!(xattr_acl_quota);
