//! Property-based tests for ext4 extent tree operations.
//!
//! Uses the proptest API directly (TestRunner::run()) instead of the
//! proptest! macro, because the macro in v1.11.0 doesn't generate
//! #[test] functions in integration tests.

use proptest::prelude::*;
use proptest::test_runner::{TestRunner, Config};
use ext4_core::*;

// Note: the ext4-core crate names this module "superblock" (no underscore)
use ext4_core::superblock;
use ext4_core::inode;
use ext4_core::extent;

fn test_sb_bytes() -> Vec<u8> {
    let mut data = vec![0u8; 1024];
    data[56..58].copy_from_slice(&EXT4_SUPER_MAGIC.to_le_bytes());
    data[24..28].copy_from_slice(&2u32.to_le_bytes());
    data[96..100].copy_from_slice(&(EXT4_FEATURE_INCOMPAT_FILETYPE |
        EXT4_FEATURE_INCOMPAT_EXTENTS | EXT4_FEATURE_INCOMPAT_FLEX_BG).to_le_bytes());
    data[100..104].copy_from_slice(&EXT4_FEATURE_RO_COMPAT_SPARSE_SUPER.to_le_bytes());
    data[76..80].copy_from_slice(&1u32.to_le_bytes());
    data[88..90].copy_from_slice(&256u16.to_le_bytes());
    data[92] = 5;
    data
}

fn make_empty_inode_bytes() -> Vec<u8> {
    let mut data = vec![0u8; 256];
    data[32..36].copy_from_slice(&EXT4_EXTENTS_FL.to_le_bytes());
    data[40..42].copy_from_slice(&EXT4_EXTENT_MAGIC.to_le_bytes());
    data[42..44].copy_from_slice(&0u16.to_le_bytes());
    data[44..46].copy_from_slice(&4u16.to_le_bytes());
    data[46..48].copy_from_slice(&0u16.to_le_bytes());
    data
}

#[test]
fn insert_then_lookup_returns_inserted() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(0u32..10000u32, 0u64..(1u64 << 40), 1u16..=100u16),
        |(logical_block, physical_block, block_count)| {
            let sb_data = test_sb_bytes();
            let sb = superblock::parse_superblock(&sb_data).unwrap();
            let raw = inode::parse_inode(&make_empty_inode_bytes(), &sb).unwrap();
            let mut inode = raw;
            let lb = logical_block;

            let result = extent::extent_insert(&sb, &mut inode, lb, physical_block, block_count, &mut |_| Ok(()));
            prop_assert!(result.is_ok());

            let lookup = extent::extent_lookup(&sb, &inode, lb as u64, |_, _| Err(Ext4Error::IoError));
            prop_assert!(lookup.is_ok());
            prop_assert_eq!(lookup.unwrap(), Some(physical_block));
            Ok(())
        },
    ).unwrap();
}

#[test]
fn empty_inode_lookup_returns_none() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(0u64..10000u64),
        |logical_block| {
            let sb_data = test_sb_bytes();
            let sb = superblock::parse_superblock(&sb_data).unwrap();
            let inode = inode::parse_inode(&make_empty_inode_bytes(), &sb).unwrap();
            let result = extent::extent_lookup(&sb, &inode, logical_block, |_, _| Err(Ext4Error::IoError));
            prop_assert!(result.is_ok());
            prop_assert_eq!(result.unwrap(), None);
            Ok(())
        },
    ).unwrap();
}

#[test]
fn extent_header_serialization_roundtrip() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(0u16..=4u16, 0u16..=2u16),
        |(entries, depth)| {
            let header = Ext4ExtentHeader {
                eh_magic: EXT4_EXTENT_MAGIC,
                eh_entries: entries,
                eh_max: 4,
                eh_depth: depth,
                eh_generation: 0,
            };
            let mut buf = [0u8; 12];
            extent::serialize_header(&mut buf, &header);
            let read_magic = u16::from_le_bytes([buf[0], buf[1]]);
            let read_entries = u16::from_le_bytes([buf[2], buf[3]]);
            let read_depth = u16::from_le_bytes([buf[6], buf[7]]);
            prop_assert_eq!(read_magic, EXT4_EXTENT_MAGIC);
            prop_assert_eq!(read_entries, entries);
            prop_assert_eq!(read_depth, depth);
            Ok(())
        },
    ).unwrap();
}

#[test]
fn extent_entry_serialization_roundtrip() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(any::<u32>(), 1u16..=32768u16, any::<u32>()),
        |(ee_block, ee_len, ee_start_lo)| {
            let extent = Ext4Extent {
                ee_block, ee_len, ee_start_hi: 0, ee_start_lo,
            };
            let mut buf = [0u8; 60];
            let header = Ext4ExtentHeader { eh_magic: EXT4_EXTENT_MAGIC, eh_entries: 1,
                eh_max: 4, eh_depth: 0, eh_generation: 0 };
            extent::serialize_header(&mut buf, &header);
            extent::serialize_extent(&mut buf, 12, &extent);
            let extents = extent::deserialize_extents(&buf, &header).unwrap();
            prop_assert_eq!(extents.len(), 1);
            prop_assert_eq!(extents[0].ee_block, ee_block);
            prop_assert_eq!(extents[0].ee_len & 0x7FFF, ee_len & 0x7FFF);
            prop_assert_eq!(extents[0].ee_start_lo, ee_start_lo);
            Ok(())
        },
    ).unwrap();
}

#[test]
fn extent_start_block_is_correct() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(any::<u32>(), any::<u16>()),
        |(ee_start_lo, ee_start_hi)| {
            let extent = Ext4Extent { ee_block: 0, ee_len: 1, ee_start_hi, ee_start_lo };
            prop_assert_eq!(extent.start_block(),
                (ee_start_lo as u64) | ((ee_start_hi as u64) << 32));
            Ok(())
        },
    ).unwrap();
}

#[test]
fn uninit_extent_detection() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(any::<u32>(), any::<u32>()),
        |(ee_block, ee_start_lo)| {
            let uninit = Ext4Extent { ee_block, ee_len: 0x8001, ee_start_hi: 0, ee_start_lo };
            prop_assert!(uninit.is_uninit());
            let init = Ext4Extent { ee_block, ee_len: 1, ee_start_hi: 0, ee_start_lo };
            prop_assert!(!init.is_uninit());
            Ok(())
        },
    ).unwrap();
}
