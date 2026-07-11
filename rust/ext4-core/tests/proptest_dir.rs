//! Property-based tests for ext4 directory operations.
//!
//! Uses the proptest API directly (TestRunner::run()) instead of the
//! proptest! macro, because the macro in v1.11.0 doesn't generate
//! #[test] functions in integration tests.

use proptest::prelude::*;
use proptest::test_runner::{TestRunner, Config};
use ext4_core::*;

#[test]
fn dirent_size_is_aligned() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(0u8..=255u8),
        |name_len| {
            let size = dir::dirent_size(name_len);
            prop_assert_eq!(size % 4, 0);
            Ok(())
        },
    ).unwrap();
}

#[test]
fn short_names_have_minimal_size() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(1u8..=4u8),
        |name_len| {
            let size = dir::dirent_size(name_len);
            let expected = ((8 + name_len as usize) + 3) & !3;
            prop_assert_eq!(size, expected);
            Ok(())
        },
    ).unwrap();
}

#[test]
fn insert_then_lookup_finds_entry() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(1u32..100000u32, "[a-z]{1,8}", prop_oneof![
            Just(EXT4_FT_REG_FILE),
            Just(EXT4_FT_DIR),
            Just(EXT4_FT_SYMLINK),
        ]),
        |(inode, name, file_type)| {
            let block_size = 1024usize;
            let mut block = vec![0u8; block_size];
            dir::init_dir_block(&mut block, 2, 2, 0);
            let inserted = dir::insert_into_block(&mut block, inode, &name, file_type);
            prop_assert!(inserted);
            if let Some(entry) = dir::lookup_linear(&block, &name) {
                prop_assert_eq!(entry.inode, inode);
                prop_assert_eq!(entry.file_type, file_type);
            } else {
                panic!("lookup_linear should find '{}'", name);
            }
            Ok(())
        },
    ).unwrap();
}

#[test]
fn remove_then_lookup_returns_none() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(1u32..100000u32, "[a-z]{1,8}"),
        |(inode, name)| {
            let block_size = 1024usize;
            let mut block = vec![0u8; block_size];
            dir::init_dir_block(&mut block, 2, 2, 0);
            prop_assert!(dir::insert_into_block(&mut block, inode, &name, EXT4_FT_REG_FILE));
            let removed = dir::remove_from_block(&mut block, &name);
            prop_assert!(removed);
            let found = dir::lookup_linear(&block, &name);
            prop_assert!(found.is_none() || found.unwrap().inode == 0);
            Ok(())
        },
    ).unwrap();
}

#[test]
fn init_dir_block_has_dot_and_dotdot() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(1u32..100000u32, 1u32..100000u32),
        |(dir_ino, parent_ino)| {
            let block_size = 1024usize;
            let mut block = vec![0u8; block_size];
            dir::init_dir_block(&mut block, dir_ino, parent_ino, 0);
            let dot = dir::lookup_linear(&block, ".");
            prop_assert!(dot.is_some());
            prop_assert_eq!(dot.unwrap().inode, dir_ino);
            let dotdot = dir::lookup_linear(&block, "..");
            prop_assert!(dotdot.is_some());
            prop_assert_eq!(dotdot.unwrap().inode, parent_ino);
            Ok(())
        },
    ).unwrap();
}

#[test]
fn dirent_iter_traverses_all_entries() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &proptest::collection::vec(
            (1u32..1000u32, "[a-z]{1,4}", prop_oneof![
                Just(EXT4_FT_REG_FILE),
                Just(EXT4_FT_DIR),
            ]),
            1..=5,
        ),
        |entries| {
            let block_size = 1024usize;
            let mut block = vec![0u8; block_size];
            dir::init_dir_block(&mut block, 2, 2, 0);
            for (ino, ref name, ft) in &entries {
                let inserted = dir::insert_into_block(&mut block, *ino, name, *ft);
                prop_assume!(inserted);
            }
            let count = dir::DirEntryIter::new(&block).count();
            prop_assert_eq!(count, 2 + entries.len());
            Ok(())
        },
    ).unwrap();
}

#[test]
fn valid_dirent_name_checks() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(".*",),
        |(name,)| {
            let valid = dir::is_valid_dirent_name(&name);
            if name.is_empty() || name == "." || name == ".." || name.len() > 255 || name.contains('\0') {
                prop_assert!(!valid);
            } else {
                prop_assert!(valid);
            }
            Ok(())
        },
    ).unwrap();
}
