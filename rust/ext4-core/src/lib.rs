//! # ext4-core — Pure Rust ext4 filesystem parser
//!
//! Read-only ext4 on-disk format parser supporting:
//! - Superblock validation (magic, feature flags)
//! - Group descriptor table (32-bit and 64-bit)
//! - Inode parsing (128/256 byte, extents, inline data)
//! - Extent tree traversal (depth 0-3)
//! - Directory entry reading (linear + htree fallback)
//!
//! ## no_std compatibility
//!
//! This crate is `no_std` with `alloc` for cross-compilation.
//! When targeting a MINIX kernel driver (staticlib), the C host
//! provides malloc/free. For native builds, `std` is available
//! through the default feature set.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use ext4_core::{Ext4Superblock, parse_superblock};
//!
//! // Read the superblock from a raw device (offset 1024)
//! let mut block = vec![0u8; 4096];
//! // ... fill block from device at offset 1024 ...
//! let sb = parse_superblock(&block).unwrap();
//! let block_size = sb.block_size();
//! ```

#![cfg_attr(target_os = "minix", no_std)]

// Global allocator (wraps C malloc/free) and panic handler.
// Required when building with -Zbuild-std=core,alloc for a custom target.
// For native builds, this never conflicts because the library is no_std.
#[cfg(all(not(test), not(feature = "std"), target_os = "minix"))]
mod allocator;

extern crate alloc;

// Re-export common alloc types for convenience
pub use alloc::vec::Vec;
pub use alloc::string::String;
pub use alloc::string::ToString;
pub use alloc::format;
pub use alloc::boxed::Box;

mod types;
pub mod superblock;
pub mod group_desc;
pub mod inode;
pub mod extent;
pub mod dir;
pub mod block;
#[allow(non_camel_case_types)]
pub mod ffi;
pub mod block_alloc;
pub mod ialloc;
pub mod journal;
pub mod xattr;
pub mod acl;
pub mod quota;
// mkfs uses std::os::unix file I/O; only available with the "std" feature on unix.
#[cfg(all(feature = "std", unix))]
pub mod mkfs;

pub use types::*;
pub use superblock::{parse_superblock, serialize_superblock};
pub use group_desc::{parse_group_descriptors, serialize_group_descriptors, serialize_group_desc};
pub use inode::{parse_inode, new_inode, serialize_inode, init_extent_tree,
    set_file_size, set_blocks_count, update_timestamps, update_timestamps_ns,
    set_symlink_target, get_symlink_target, inode_to_group, inode_to_group_index,
    inode_link, inode_unlink, mark_inode_deleted, free_inode_data};
pub use extent::{extent_lookup, extent_read, extent_insert, extent_truncate, extent_write,
    serialize_header, serialize_extent, deserialize_extents};
pub use dir::{DirEntryIter, lookup_linear, lookup_in_dir, file_type_to_mode,
    insert_into_block, remove_from_block, init_dir_block,
    htree_hash, htree_find_leaf, htree_insert_entry, htree_remove_entry,
    init_htree_dir, expand_dir, insert_in_dir, remove_in_dir,
    parse_dx_root_info};
pub use block::{read_inode, has_superblock_backup, block_to_byte, gdt_blocks_count};
pub use journal::{Journal, JournalTransaction, JournalBlock,
    Jbd2Superblock, Jbd2Header, Jbd2BlockTag, Jbd2DescriptorBlock,
    Jbd2CommitBlock, Jbd2RevokeBlock, ScanResult,
    journal_new, journal_commit, journal_checkpoint, journal_start_transaction,
    serialize_descriptor_block, serialize_commit_block, serialize_journal_superblock,
    set_commit_timestamp, crc32c_le, crc32c, crc32, crc32c_seeded,
    parse_jbd2_header, parse_journal_superblock, parse_descriptor_block,
    parse_commit_block, parse_revoke_block, scan_journal_block,
    recover_journal, RecoveryConfig, unescape_block, read_journal_superblock};
pub use xattr::{Xattr, Ext4XattrEntry, Ext4XattrHeader, Ext4XattrIbodyHeader,
    match_xattr_name, xattr_prefix, find_xattr,
    parse_xattrs, inode_xattr_space,
    EXT4_XATTR_MAGIC};
pub use acl::{Ext4Acl, Ext4AclEntry,
    EXT4_ACL_VERSION, ACL_USER_OBJ, ACL_USER, ACL_GROUP_OBJ,
    parse_acl, serialize_acl, acl_permissions};
pub use quota::{Ext4DqblkV2, parse_dqblk_v2, serialize_dqblk_v2};
