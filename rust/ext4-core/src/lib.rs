//! # ext4-core — Pure Rust ext4 filesystem parser
//!
//! Read-only ext4 on-disk format parser supporting:
//! - Superblock validation (magic, feature flags)
//! - Group descriptor table (32-bit and 64-bit)
//! - Inode parsing (128/256 byte, extents, inline data)
//! - Extent tree traversal (depth 0-3)
//! - Directory entry reading (linear + htree fallback)
//!
//! ## Usage
//!
//! ```rust,no_run
//! use ext4_core::{Ext4Superblock, parse_superblock};
//!
//! // Read the superblock from a raw device (offset 1024)
//! let mut block = vec![0u8; 4096];
//! // ... fill block from device at offset 1024 ...
//! let sb = parse_superblock(&block).unwrap();
//! println!("Block size: {}", sb.block_size());
//! ```

mod types;
pub mod superblock;
pub mod group_desc;
pub mod inode;
pub mod extent;
pub mod dir;
pub mod block;
pub mod ffi;
pub mod alloc;
pub mod ialloc;
pub mod journal;

pub use types::*;
pub use superblock::parse_superblock;
pub use group_desc::parse_group_descriptors;
pub use inode::{parse_inode, new_inode, serialize_inode, init_extent_tree,
    set_file_size, set_blocks_count, update_timestamps, update_timestamps_ns,
    set_symlink_target, get_symlink_target, inode_to_group, inode_to_group_index,
    inode_link, inode_unlink, mark_inode_deleted, free_inode_data};
pub use extent::{extent_lookup, extent_read, extent_insert, extent_truncate,
    serialize_header, serialize_extent, deserialize_extents};
pub use dir::{DirEntryIter, lookup_linear, lookup_in_dir, file_type_to_mode,
    insert_into_block, remove_from_block, init_dir_block};
pub use block::{read_inode, has_superblock_backup, block_to_byte, gdt_blocks_count};
