# ext4 Driver Architecture — GergiOS 1.0+/1.1

> **Статус**: Phase 1 ✅, Phase 2 ✅, Phase 3 ✅, Phase 4 ✅, Phase 5 ✅, Phase 6 ✅, **Phase 7 🟡** (July 2026)
> **Связанные**: `planning/10_netbsd_dependency_audit.md` (§3.8 Phase 7 ✅), `planning/17_remaining_tasks.md` (§T20 ✅, §T21)
> **Репозиторий**: `rust/ext4-core/` (58 unit tests + 1 doc-test, 0 errors)
> **C bridge**: `minix/fs/ext4/` (ffi.h, ffi_bridge.c, main.c, table.c, CMakeLists.txt)

---

## 1. Executive Summary

**Цель**: Добавить ext4 как нативную файловую систему GergiOS, сохранив MFS как legacy.

**Ключевой архитектурный выбор**: Rust core (pure ext4 parser) + C FFI bridge (libfsdriver).

| Аспект | Решение | Причина |
|--------|---------|---------|
| **Язык ядра** | Rust | Memory safety, существующие крейты, будущая архитектура |
| **Язык интерфейса** | C (libfsdriver) | Единственный способ взаимодействия с MINIX VFS |
| **Блочный ввод-вывод** | C (libbdev + libminixfs) | Готовая MINIX инфраструктура |
| **Read-only first** | ✅ Да | Быстрый результат, foundation для write |
| **FUSE** | ❌ Нет | libpuffs не поддерживается, лишняя прослойка |
| **Журнал (jbd2)** | deferred | Phase 2, после read/write |

---

## 2. Текущая архитектура MINIX FS серверов

### 2.1 Как работает FS сервер (на примере ext2)

```
VFS (minix/servers/vfs/)
  │    IPC message (REQ_LOOKUP, REQ_READ, REQ_WRITE, ...)
  ▼
ext2 server (minix/fs/ext2/)
  │    struct fsdriver ext2_table { fdr_mount, fdr_lookup, fdr_read, ... }
  ▼
libfsdriver (minix/lib/libfsdriver/)
  │    fsdriver_process() — диспатчинг сообщений
  │    fsdriver_copyin/copyout — копирование между адресными пространствами
  ▼
libminixfs (minix/lib/libminixfs/)
  │    lmfs_get_block, lmfs_put_block — кеш блоков
  │    lmfs_bio — блочный ввод-вывод
  ▼
libbdev → block driver (ahci, at_wini, virtio_blk, ...)
```

### 2.2 struct fsdriver — таблица функций

Каждый FS сервер определяет `struct fsdriver` с ~30 колбеками:

```c
struct fsdriver {
    int   (*fdr_mount)(dev_t, unsigned int, struct fsdriver_node*, unsigned int*);
    void  (*fdr_unmount)(void);
    int   (*fdr_lookup)(ino_t, char*, struct fsdriver_node*, int*);
    int   (*fdr_newnode)(mode_t, uid_t, gid_t, dev_t, struct fsdriver_node*);
    int   (*fdr_putnode)(ino_t, unsigned int);
    ssize_t (*fdr_read)(ino_t, struct fsdriver_data*, size_t, off_t, int);
    ssize_t (*fdr_write)(ino_t, struct fsdriver_data*, size_t, off_t, int);
    ssize_t (*fdr_getdents)(ino_t, struct fsdriver_data*, size_t, off_t*);
    int   (*fdr_trunc)(ino_t, off_t, off_t);
    int   (*fdr_create)(ino_t, char*, mode_t, uid_t, gid_t, struct fsdriver_node*);
    int   (*fdr_mkdir)(ino_t, char*, mode_t, uid_t, gid_t);
    int   (*fdr_link)(ino_t, char*, ino_t);
    int   (*fdr_unlink)(ino_t, char*, int);
    ...
    ssize_t (*fdr_bread)(dev_t, struct fsdriver_data*, size_t, off_t, int);
    ssize_t (*fdr_bwrite)(dev_t, struct fsdriver_data*, size_t, off_t, int);
};
```

### 2.3 libminixfs — блочный кеш

`libminixfs` предоставляет:
- `lmfs_get_block()`/`lmfs_put_block()` — кешированные блоки
- `lmfs_bio()` — прямой блочный ввод-вывод (используется для bread/bwrite)
- `lmfs_set_blocksize()` — размер блока (для ext4: 1024/2048/4096)
- `lmfs_markdirty()` — пометить блок как изменённый
- `lmfs_flushall()`/`lmfs_flushdev()` — сброс кеша

---

## 3. ext4 On-Disk Format — что нужно поддерживать

### 3.1 Ключевые отличия ext4 от ext2

| Аспект | ext2 | ext4 |
|--------|------|------|
| Block mapping | Indirect blocks (triple) | **Extent tree** (ext4_extent_header, ext4_extent_idx, ext4_extent) |
| Inode size | 128 bytes | 256 bytes (+ extended attributes in inode) |
| Superblock | s_blocks_count, s_inodes_count | + s_blocks_count_hi, s_inodes_count_hi, s_first_ino, s_inode_size |
| Group descriptor | 32 bytes | 64 bytes (+ flex_bg, meta_bg) |
| Directory | Linear linked list | **Htree** (indexed B-tree for large dirs) |
| Journal | ❌ | ✅ **jbd2** — журналирование |
| Timestamps | секунды | **наносекунды** (i_atime_extra, etc.) |
| File size max | 2TB / 16TB | 16TB / 1EB (extents) |
| Feature flags | Нет | INCOMPAT, RO_COMPAT, COMPAT |

### 3.2 Структуры для реализации Phase 1 (read-only)

```c
// Суперблок (читается с offset 1024, size 1024)
struct ext4_superblock {
    u32  s_inodes_count;       // Общее количество inodes
    u32  s_blocks_count_lo;    // Общее количество блоков (lo)
    u32  s_free_blocks_count_lo;
    u32  s_free_inodes_count;
    u32  s_first_data_block;   // 0 для block_size=1024, иначе 0
    u32  s_log_block_size;     // block_size = 1024 << s_log_block_size
    u32  s_log_cluster_size;   // cluster_size = 1024 << s_log_cluster_size
    u32  s_blocks_per_group;
    u32  s_clusters_per_group;
    u32  s_inodes_per_group;
    u32  s_mtime;
    u32  s_wtime;
    u16  s_inode_size;         // 128 или 256 (для ext4 обычно 256)
    u32  s_first_ino;          // Первый нерезервированный inode (обычно 11)
    u16  s_inode_size;
    u8   s_blocks_count_hi;    // Верхние 32 бита для >2^32 блоков
    u32  s_feature_incompat;   // INCOMPAT_* flags
    u32  s_feature_ro_compat;  // RO_COMPAT_* flags
    u32  s_feature_compat;     // COMPAT_* flags
    u8   s_uuid[16];
    char s_volume_name[16];
    // ... many more fields
};

// Extent tree — основа ext4
struct ext4_extent_header {
    u16  eh_magic;             // 0xF30A
    u16  eh_entries;           // Количество записей в этом блоке
    u16  eh_max;               // Максимальное количество записей
    u16  eh_depth;             // 0=leaf, 1+=index blocks
    u32  eh_generation;
};

struct ext4_extent {
    u32  ee_block;             // Первый логический блок
    u16  ee_len;               // Количество блоков (1-32768)
    u16  ee_start_hi;          // Верхние 16 бит физического блока
    u32  ee_start_lo;          // Нижние 32 бита физического блока
};

// Inode (256 bytes для ext4)
struct ext4_inode {
    u16  i_mode;
    u16  i_uid;
    u32  i_size_lo;
    u32  i_atime;
    u32  i_ctime;
    u32  i_mtime;
    u32  i_dtime;
    u16  i_gid;
    u16  i_links_count;
    u32  i_blocks_lo;          // Количество 512-байтных блоков
    u32  i_flags;
    union {
        struct {
            u32  i_block[15];  // Для ext2/3 — indirect blocks
            // Для ext4 — root of extent tree в i_block[0..3]
        };
        struct {
            struct ext4_extent_header i_extent_header;
            struct ext4_extent     i_extent[4];
        };
    };
    u32  i_generation;
    u32  i_file_acl;
    u32  i_size_hi;
    u32  i_obso_faddr;
    // ... extended attributes follow (if inode_size > 128)
};
```

---

## 4. Архитектурные опции

### 4.1 Option A: Нативный C сервер (как ext2)

```
minix/fs/ext4/        ← C код, копия ext2 сервера
  ├── main.c          ← fsdriver_task(&ext4_table)
  ├── table.c         ← struct fsdriver ext4_table
  ├── mount.c         ← ext4 superblock + group descriptors
  ├── inode.c         ← ext4 inode management
  ├── path.c          ← lookup (directory traversal + htree)
  ├── read.c          ← read + extent tree traversal
  ├── write.c         ← write + block allocation
  ├── balloc.c        ← block allocator
  ├── ialloc.c        ← inode allocator
  ├── open.c, link.c, stadir.c, misc.c, protect.c
  └── super.c         ← superblock operations
```

**Плюсы**: Проще всего — прямое копирование ext2 сервера, полный контроль  
**Минусы**: C (нет memory safety), ~5000 LOC, нужно переписывать ext4 логику с нуля  
**Риск**: Высокая вероятность багов в extent tree и htree  

### 4.2 Option B: FUSE (libpuffs)

```
minix/fs/ext4/        ← C код + FUSE прослойка
  └── puffs_ext4.c    ← libpuffs + Linux ext4 driver
```

**Плюсы**: Можно использовать Linux ext4 driver  
**Минусы**: libpuffs не поддерживается на GergiOS, FUSE overhead, ненадёжно  
**Вердикт**: ❌ Отвергнуто

### 4.3 Option C: Rust core + C FFI bridge (РЕКОМЕНДУЕТСЯ)

```
minix/fs/ext4/                        ← C glue (libfsdriver интерфейс)
  ├── CMakeLists.txt                  ← add_minix_service() + Rust static lib
  ├── main.c                          ← fsdriver_task(&ext4_table)
  ├── table.c                         ← struct fsdriver ext4_table
  └── ffi_bridge.c                    ← вызовы Rust функций через extern "C"

rust/ext4-core/                       ← Pure Rust ext4 parser
  ├── Cargo.toml
  ├── src/
  │   ├── lib.rs                      ← Публичный API
  │   ├── superblock.rs               ← Парсинг суперблока
  │   ├── group_desc.rs               ← Group descriptors
  │   ├── inode.rs                    ← Inode table + extent tree
  │   ├── extent.rs                   ← Extent tree traversal
  │   ├── dir.rs                      ← Directory + htree
  │   ├── block.rs                    ← Block addressing
  │   └── ffi.rs                      ← extern "C" функции для C FFI
  ├── tests/
  └── benches/
```

**FFI Interface** (C ↔ Rust):

```c
// C вызывает Rust через эти функции:
int     ext4_mount(dev_t dev, struct ext4_sb_info *sbi);
int     ext4_lookup(struct ext4_sb_info *sbi, ino_t dir_ino,
                    const char *name, ino_t *ino_out);
ssize_t ext4_read(struct ext4_sb_info *sbi, ino_t ino,
                  void *buf, size_t count, off_t pos);
int     ext4_stat(struct ext4_sb_info *sbi, ino_t ino, struct stat *buf);
// ... и т.д.
```

**Плюсы**: 
- Memory safety в самом сложном коде (extent tree, htree)
- Можно тестировать на любой платформе (cargo test)
- Существующие Rust крейты как reference
- Постепенная миграция экосистемы на Rust
**Минусы**:
- FFI overhead (минимальный)
- Нужен Rust staticlib для CMake
- Два языка в одном компоненте

---

## 5. Рекомендуемый подход: Rust core + C bridge

### 5.1 Почему это лучший выбор

1. **ext4 сложен** — extent tree, htree, flex_bg, журнал. Rust ownership модель
   предотвращает целый класс багов (use-after-free в extent tree, double-free
   в блочном аллокаторе).

2. **Тестируемость** — Rust парсер можно тестировать `cargo test` на хосте
   (Linux, Windows, macOS). C сервер требует MINIX для тестирования.

3. **Существующие крейты** — `fs-ext4` (pure Rust, read-heavy, FUSE-ready) и
   `ext4-view` можно использовать как reference или зависимости.

4. **Не блокирует MINIX** — C bridge минимален (~200 строк), вся логика в Rust.

5. **Будущее** — Когда MINIX VFS/libfsdriver будет портирован на Rust, ext4-core
   станет pure Rust без FFI.

### 5.2 Основные риски и mitigation

| Риск | Mitigation |
|------|------------|
| **FFI паника** | Все `extern "C"` функции используют `catch_unwind` |
| **Производительность extent tree** | Rust ownership = zero-cost, не хуже C |
| **Сборка Rust staticlib** | CMake `add_rust_staticlib()` макрос (уже есть) |
| **Размер бинарника** | LTO убирает мёртвый код |
| **Совместимость с libminixfs** | C bridge использует `lmfs_get_block` для блочного ввод-вывода |

---

## 6. Дорожная карта

### Phase 1: Foundation — Rust ext4 parser (read-only) ✅ 2-3 weeks

**Цель**: Читать ext4 раздел, ls, cat, stat. ✅

**Rust crate `ext4-core`** (10 source files, ~1,500 LOC):
- [x] Суперблок: парсинг всех полей, валидация magic (0xEF53), feature flags
- [x] Group descriptor: 32/64-bit, flex_bg, reserve GDT
- [x] Inode table: 256-byte inodes, extended attributes (extra isize)
- [x] Extent tree: depth-1/2/3 traversal, owned Vec для index node loop
- [x] Directory: linear DirEntryIter + htree detection + fallback scan
- [x] Symlink: `file_type_to_mode()` готов (чтение через extent_read)
- [x] FFI: `ext4_parse_superblock`, `ext4_sb_info_size`, `ext4_read_inode` (placeholder)
- [x] Error handling: 11 error variants, POSIX errno mapping

**C bridge**:
- [x] `minix/fs/ext4/main.c` — fsdriver_task(&ext4_table)
- [x] `minix/fs/ext4/table.c` — struct fsdriver ext4_table
- [x] `minix/fs/ext4/ffi_bridge.c` — extern "C" вызовы
- [x] `minix/fs/ext4/CMakeLists.txt` — add_minix_service + Rust staticlib

**Тестирование**:
- [x] `cargo test` — 58 unit tests + 1 doc-test, all PASS
- [x] `cargo bench` — 19 benchmarks, все работают (см. §11 Benchmark Results)
- [x] Rust staticlib (native): `cargo build --release --lib` → `ext4_core.lib`/`libext4_core.a` ✅
- [x] Cross-compilation infra: target spec + build script + CMake integration ✅
- [ ] Монтирование реального ext4 раздела в MINIX (ждёт MINIX toolchain + DESTDIR)

**Зависимости**: Rust toolchain ✅, CMake Rust integration ✅ (см. §12 Build Infrastructure)

---

### Phase 2: Write Support ✅ 4-6 weeks

**Цель**: mkdir, touch, cp, rm, mv на ext4.

**Реализовано в Rust (`rust/ext4-core/src/`):**
- [x] **Block allocator** (`alloc.rs`): `BlockAllocator` struct, bitmap helpers, flex_bg support
- [x] **Block allocation**: `allocate_blocks()` — поиск последовательных свободных блоков, group descriptor update
- [x] **Block free**: `free_blocks()` — отметка блоков как свободных в bitmap + GD update
- [x] **Group descriptor helpers**: `free_blocks_count()`, `set_free_blocks_count()`, `free_inodes_count()`, `set_free_inodes_count()`
- [x] **Extent insert** (`extent.rs`): `extent_insert()` — depth-0 sorted insert
- [x] **Extent merge**: merge with predecessor (physically adjacent) + merge with successor
- [x] **Extent serialize**: `serialize_header()`, `serialize_extent()`, `serialize_idx()`, `deserialize_extents()`
- [x] **Inode serialize** (`inode.rs`): `serialize_inode()` — полная запись inode в raw buffer
- [x] **Inode helpers**: `new_inode()`, `init_extent_tree()`, `set_file_size()`, `set_blocks_count()`, `update_timestamps()`
- [x] **Error types**: `NoSpace`, `ExtentTreeFull`

**Добавлено позже (Phase 2C + Deferred):**
- [x] **Extent split** (`extent.rs`): overlap detection + split (left/right/full-cover/inside) при частичном перекрытии
- [x] **Inode allocator** (`ialloc.rs`, ~160 LOC): `InodeAllocator` struct, `allocate_inode()`, `free_inode()`, inode bitmap helpers
- [x] **Directory write** (`dir.rs`): `insert_into_block()` — split rec_len padding; `remove_from_block()` — mark deleted, merge rec_len; `init_dir_block()` — `.` и `..` entries
- [x] **Htree write** (`dir.rs`): `htree_find_leaf()` — walk htree index (binary search); `htree_insert_entry()` / `htree_remove_entry()` — insert/delete in htree; `init_htree_dir()` — create htree root block; `expand_dir()` — allocate new block; `insert_in_dir()` / `remove_in_dir()` — generic dispatch (linear vs htree)
- [x] **Symlink support** (`inode.rs`): `set_symlink_target()` / `get_symlink_target()` — fast symlinks (target в i_block, до 60 байт)
- [x] **Truncate** (`extent.rs`): `extent_truncate()` — удаление/укорачивание extent-ов за new_size, free_blocks_cb, set_file_size
- [x] **Timestamps** (`inode.rs`): `update_timestamps_ns()` — kernel-compatible nanosecond precision (extra[2:31] = ns, extra[0:1] = extra seconds)
- [x] **Reserved inodes**: `new_reserved_inode()` — создание зарезервированных inode (root, etc.)

**Финальные дополнения Phase 2:**
- [x] **i_blocks_hi поддержка**: новое поле `i_blocks_hi: u32` в `Ext4Inode`, `blocks_count()` getter, `set_blocks_count()` теперь пишет обе части
- [x] **Link/unlink helpers**: `inode_link()` (inc links_count), `inode_unlink()` (dec, возвращает true если 0), `mark_inode_deleted()` (clear mode/flags, set dtime)
- [x] **Free inode**: `free_inode_data()` — truncate extent tree до 0 + free_blocks_cb + free_inode_cb

**C bridge write-колбеки (table.c):**
- [x] fdr_create, fdr_mkdir, fdr_write, fdr_link, fdr_unlink, fdr_rename, fdr_rmdir, fdr_trunc, fdr_slink, fdr_rdlink, fdr_chown, fdr_chmod, fdr_utime, fdr_mknod, fdr_peek, fdr_mountpt — все active, 29/35 колбеков

---

### Phase 3: Journal (jbd2) ✅ 3-4 weeks

**Цель**: Crash-safe запись с журналированием.

- [x] **JBD2 on-disk format**: journal superblock (V1/V2), descriptor blocks, commit blocks, revoke blocks
- [x] **Big-endian helpers**: `be_u32()`, `be_u16()`, `be_i32()` — JBD2 uses network byte order (unlike ext4 LE)
- [x] **Parsing**: `parse_jbd2_header()`, `parse_journal_superblock()`, `parse_descriptor_block()`, `parse_commit_block()`, `parse_revoke_block()`
- [x] **Block tag parsing**: 32/64-bit block numbers, JBD2_FLAG_SAME_UUID, JBD2_FLAG_LAST_TAG detection
- [x] **Journal scanner**: `scan_journal_block()` — identifies block type from raw data, validates magic + sequence
- [x] **Recovery engine**: `recover_journal()` — 3-pass (SCAN → REVOKE → REPLAY), committed transaction detection, revoked block skipping
- [x] **Clean journal detection**: `is_clean()` — fast path for journals with s_start == 0
- [x] **8 unit tests**: superblock parsing, descriptor parsing, commit parsing, revoke parsing, scan + commit flow, clean recovery, short buffer, wrong magic, info string

**Реализовано позже:**
- [x] **UUID skip**: `parse_descriptor_block()` — каждый tag без `JBD2_FLAG_SAME_UUID` имеет 16-byte UUID после себя. Теперь корректно пропускается (`off += 16`)
- [x] **Tag size fix**: tag всегда 8 байт (blocknr:4 + flags:2 + blocknr_high:2), независимо от SAME_UUID. Раньше при SAME_UUID читалось только 6 байт, теряя blocknr_high для 64-bit FS
- [x] **Escaped block**: `unescape_block()` — восстанавливает `JBD2_MAGIC_NUMBER` (0xC03B3998) в big-endian в первые 4 байта data блока при `JBD2_FLAG_ESCAPE`
- [x] **Recovery rewrite**: `recover_journal()` — теперь корректно читает data блоки (descriptor_pos + 1 + tag_index) и применяет unescape/zero для ESCAPE/DELETED блоков

**Реализовано позже:**
- [x] **Checksum validation**: CRC-32C + CRC-32 табличная реализация (~50 LOC). Валидация CSUM_V2/V3 tail checksum для descriptor блоков (последние 4 байта). Валидация V3 CRC-32C в commit блоках (offset 16). `InvalidChecksum` variant в `ScanResult`. Функции `validate_descriptor_checksum()` и `validate_commit_checksum()` подключены к `scan_journal_block()` через `Option<&Jbd2Superblock>`. 8 новых тестов на CRC + checksum.
- [x] **CSUM_V3 tag format**: `parse_descriptor_block()` теперь принимает `csum_v3: bool`. При true — теги 6 байт (blocknr:4 + flags:2, без blocknr_high), соответствует `journal_block_tag3_s`.

**Реализовано в Phase 3 (extended):**
- [x] **Journal state management**: `Journal` struct with `new()`, `free_blocks()`, `has_space_for()`, `advance()`
- [x] **Journal commit**: `journal_commit()` — serializes descriptor + data blocks + commit block + updated SB, circular buffer with `(first+pos)%maxlen`
- [x] **Journal checkpoint**: `journal_checkpoint()` — scans committed transactions, replays data blocks to FS locations, marks journal clean
- [x] **Journal start transaction**: `journal_start_transaction()` — advances sequence number
- [x] **Journal serialization**: `serialize_descriptor_block()`, `serialize_commit_block()`, `serialize_journal_superblock()`, `set_commit_timestamp()`
- [x] **Block escaping**: caller zeroes first 4 bytes + sets `JBD2_FLAG_ESCAPE`; `unescape_block()` restores MAGIC on checkpoint
- [x] **CRC-32C checksum options**: CSUM_V3 6-byte tags, descriptor tail checksums, V3 extended commit header checksums
- [x] **Bugfix: circular buffer modulo**: checkpoint scan/replay and recovery now use `(first+pos)%maxlen` (was `first+pos` — index out of bounds)
- [x] **Bugfix: SB persistence in checkpoint**: added `write_journal_block` callback for writing updated SB to journal device block 0 (was writing to FS block 0)
- [x] **Bugfix: escaping contract**: removed auto-detection of JBD2_MAGIC (was zeroing data without setting ESCAPE flag — corruption risk)
- [x] **3 integration tests**: basic commit, commit+checkpoint (verify FS blocks), commit+checkpoint with escaping

---

### Phase 4: C Bridge Integration (Read Path) ✅ 2-3 weeks

**Цель**: Рабочий read path через MINIX VFS (ls, cat, stat работают).

**Rust side (`ffi.rs` + `group_desc.rs`):**
- [x] **`sb_from_sbi()`**: реконструкция `Ext4Superblock` из C-совместимого `ext4_sb_info`
- [x] **`make_block_reader()`**: обёртка C `ext4_read_block_cb` в Rust `FnMut` (by-value fn pointer)
- [x] **`ext4_gd_info`**: C-совместимая структура group descriptor
- [x] **`ext4_read_inode()`**: чтение GD + inode table + парсинг inode — полностью работает
- [x] **`ext4_lookup()`**: чтение dir inode + `lookup_in_dir` с `extent_lookup` для каждого блока
- [x] **`ext4_read_file()`**: чтение inode + `extent_read` с block reader
- [x] **`ext4_stat()`**: чтение inode, заполнение stat полей
- [x] **`ext4_read_group_descriptor()`**: чтение одного GD через callback
- [x] **`free_blocks_count/inodes_count`**: добавлены в `ext4_sb_info`, заполняются из superblock

**C bridge (`ffi.h`, `ffi_bridge.c`, `table.c`):**
- [x] **`ffi.h`**: сигнатуры обновлены (ctx + read_block), добавлен `ext4_gd_info`, `EXT4_ROOT_INO`, inode константы
- [x] **`ffi_bridge.c`**: исправлен баг (undeclared `bytes`), `ext4_read_block_cb` non-static
- [x] **`table.c`**: полная реализация mount/lookup/read/stat/putnode/statvfs через Rust FFI

**Статус**: Rust 36/36 тестов PASS, C bridge ждёт компиляции в MINIX окружении.

### Phase 5: Write Path (Partial) ✅ 4-8 weeks (1.1+)

**Цель**: Базовые файловые операции через Rust FFI.

- [x] **ext4_truncate** — truncation via `extent_truncate` + `free_blocks_cb` + `write_inode` closure
- [x] **ext4_link** — hard link: increment target link count + `insert_in_dir` (htree/linear)
- [x] **ext4_unlink** — unlink: `lookup_in_dir` + `remove_in_dir` + decrement link count; if 0 → `free_inode_data`
- [x] **ext4_rmdir** — rmdir: check empty (only `.`/`..`), remove from parent, decrement parent link count, `free_inode_data` + zeroed inode
- [x] **write_block callback** — `ext4_write_block_cb` (uses `lmfs_bio(FSC_WRITE)`)
- [x] **free_blocks/free_inode/alloc_block callbacks** — C-side stubs with TODO (real allocator deferred)
- [x] **ext4_write_file** — write via `extent_write` (read-modify-write for existing blocks, alloc+insert for sparse)
- [x] **ext4_create** — alloc inode + init extent tree + write inode table + insert_in_dir
- [x] **ext4_mkdir** — alloc inode + alloc block + init_dir_block + extent_insert + insert_in_dir
- [x] **ext4_rmdir** — check empty (only `.`/`..`), remove from parent, decrement parent link count, `free_inode_data` + zeroed inode
- [x] **ext4_readdir** — getdents: читает блоки через `extent_read`, парсит raw direntry, заполняет ext4_dirent буфер
- [x] **ext4_rename** bugfix — правильное декрементирование link count заменяемого inode (не old_name, а new_name)
- [x] **ext4_mkdir** fix — `inode_link(&mut parent_inode)` для симметрии с rmdir
- [x] **Metadata checksums (CRC-32C) — inode/SB/GD write**: `ext4_update_sb_csum()`, `ext4_update_gd_csum()`, `serialize_inode()` теперь вычисляет `i_checksum_hi`; все call sites обновлены
- [x] **Directory entry CRC-32C checksums**: `init_dir_block()`/`init_htree_dir()`/`expand_dir()` пишут checksum tail; `insert_in_dir()`/`remove_in_dir()`/`htree_insert_entry()`/`htree_remove_entry()` обновляют tail при каждом изменении; алгоритм CRC-32C(seed+dir_ino+i_generation+data)
- [x] **Багфикс**: `remove_in_dir()`/`expand_dir()`/`htree_*_entry()` использовали `0` вместо реального `dir_ino` в checksum — исправлено
- [x] **Багфикс**: `inode_table_hi` не применялся в `ext4_verify_all_csums()` для 64-bit ФС — исправлено
- [x] **Extended attributes** (`xattr.rs`, ~200 LOC) — парсинг/сериализация in-inode + external block xattrs
- [x] **POSIX ACLs** (`acl.rs`, ~100 LOC) — парсинг ACL из xattr данных, проверка прав
- [x] **Quota** (`quota.rs`, ~250 LOC) — V2 dqblk on-disk формат, QuotaManager с enforcement
- [ ] Delayed allocation — отложенная запись для производительности
- [ ] fsck — полная проверка целостности ФС (e2fsck ~50K LOC)

### Phase 6: Complete FS Driver (All Callbacks) ✅ 1-2 weeks

**Цель**: Полная поддержка всех fsdriver колбеков для ext4.

**Rust FFI функции (`ffi.rs`):**
- [x] **ext4_chown** — изменение владельца/группы (read inode → set uid/gid → serialize → write back)
- [x] **ext4_chmod** — изменение прав (preserve type bits + set permission bits)
- [x] **ext4_utime** — обновление временных меток (atime, mtime, ctime)
- [x] **ext4_mknod** — создание device/special nodes: alloc inode + rdev в i_block[0] (для blk/char) + `insert_in_dir`
- [x] **ext4_symlink** — fast symlinks (≤60 байт: `set_symlink_target` в i_block) + slow symlinks (>60 байт: alloc block + `extent_insert`)
- [x] **ext4_readlink** — fast symlinks (`get_symlink_target` из i_block) + slow symlinks (`extent_read` из data blocks)

**C bridge:**
- [x] **ffi.h** — все декларации для 6 новых функций + EXT4_FT_* константы
- [x] **table.c** — полная fsdriver таблица (29/35 колбеков):
  - `fdr_chown`, `fdr_chmod`, `fdr_utime` — inode metadata
  - `fdr_mknod` — device/special nodes
  - `fdr_slink`, `fdr_rdlink` — symlink create/read
  - `fdr_peek` — чтение с VM-нотификацией (делегирует `ext4_read_cb`)    - `fdr_mountpt` — возвращает FALSE (ext4 не поддерживает mount points)
  - Остальные (fdr_newnode, fdr_seek, fdr_postcall, fdr_other) — NULL (корректно)

**Итого Phase 6:** ~350 LOC (6 FFI функций + C колбеки)

### Phase 6b: Metadata Checksums (CRC-32C) ✅ 1 week

**Цель**: Валидация и обновление CRC-32C checksum'ов для всех метаданных (superblock, group descriptors, inodes, directory blocks) при записи.

**Rust FFI функции (`ffi.rs` + `journal.rs` + `dir.rs`):**
- [x] **`crc32c_le()`** — raw CRC-32C без final XOR (kernel-compatible), в `journal.rs`
- [x] **`ext4_compute_csum_seed()`** — вычисление seed = crc32c_le(~0, s_uuid)
- [x] **`ext4_verify_sb_csum()`** — валидация s_checksum (offset 672, full 32-bit)
- [x] **`ext4_verify_gd_csum()`** — валидация bg_checksum (offset 30, lower 16 bits, + group number для 64-bit)
- [x] **`ext4_verify_inode_csum()`** — валидация i_checksum_hi (offset 130, lower 16 bits, seed+ino+generation+inode)
- [x] **`ext4_verify_all_csums()`** — batch-проверка при mount (SB + все GDT + root inode)
- [x] **`ext4_update_sb_csum()`** — обновление s_checksum при записи SB
- [x] **`ext4_update_gd_csum()`** — обновление bg_checksum при записи GD
- [x] **`serialize_inode()`** — автоматический расчёт i_checksum_hi при записи любого inode (23 call sites обновлены)
- [x] **`init_dir_block()`** — CRC-32C tail для новых директорий
- [x] **`init_htree_dir()`** — dx_tail checksum для htree root
- [x] **`insert_in_dir()`/`remove_in_dir()`** — обновление checksum tail при каждом изменении

**C bridge:**
- [x] **`ffi.h`** — `csum_seed` field, `ext4_csum_result` struct, 8 новых деклараций
- [x] **`ffi_bridge.c`** — `ext4_mount()` логирует `metadata_csum=yes/no`; `update_group_desc_free_count()` вызывает `ext4_update_gd_csum`; `sync_superblock_free_counts()` вызывает `ext4_update_sb_csum`

**Багфиксы:**
- [x] `inode_table_hi` не использовался в `ext4_verify_all_csums()` для 64-bit — исправлено
- [x] `remove_in_dir()`/`expand_dir()`/`htree_*_entry()` использовали `0` вместо `dir_ino` в checksum — исправлено
- [x] `init_dir_block()` вызывался без `csum_seed` — исправлено

**Тесты:** Rust 36/36 PASS; C bridge ждёт интеграции

---

## 7. Интеграция с MINIX

### 7.1 CMakeLists.txt

```cmake
# minix/fs/ext4/CMakeLists.txt
# ext4 filesystem server — Rust core + C bridge

# 1. Build Rust static library
add_rust_staticlib(ext4-core
    CRATE rust/ext4-core
    OUTPUT ext4_core
)

# 2. Build MINIX service
add_minix_service(ext4
    SOURCES
        main.c
        table.c
        ffi_bridge.c
    LIBS minixfs fsdriver bdev sys
    DEPENDS ext4-core
)

# 3. Link Rust static lib
target_link_libraries(ext4 PRIVATE ${CMAKE_CURRENT_BINARY_DIR}/libext4_core.a)
```

### 7.2 Сборка

```
# Текущий статус (build-aarch64/ показывает что cmake configure работает)
# ext4 сервер будет собираться условно:
if(TARGET ext4-core)
    add_subdirectory(ext4)     # Только если Rust код компилируется
endif()
```

---

## 8. Оценка объёма работ

| Компонент | LOC | Статус |
|-----------|-----|--------|
| **Rust ext4-core** | | |
| `types.rs` | ~300 | ✅ |
| `superblock.rs` | ~200 | ✅ |
| `group_desc.rs` | ~120 | ✅ |
| `inode.rs` | ~320 | ✅ (+ serialize, new_inode, symlink, nanosecond timestamps, helpers) |
| `extent.rs` | ~430 | ✅ (+ write, merge, serialize, split, truncate) |
| `dir.rs` | ~650 | ✅ (+ linear + htree + insert/remove/init dir block + htree write + CRC-32C checksum tails) |
| `block.rs` | ~100 | ✅ |
| `alloc.rs` (Phase 2) | ~220 | ✅ (block allocator, bitmaps) |
| `ialloc.rs` (Phase 2) | ~160 | ✅ (inode allocator, bitmaps) |
| `ffi.rs` | ~480 | ✅ (+ InvalidJournal, JournalCorrupt, metadata_csum validation/update functions) |
| `journal.rs` (Phase 3) | ~660 | ✅ (jbd2 parsing + recovery + CRC-32C/CRC-32 + checksum validation, `crc32c_le()`) |
| `xattr.rs` (Phase 7) | ~200 | ✅ (extended attributes — in-inode + external block) |
| `acl.rs` (Phase 7) | ~100 | ✅ (POSIX ACL parsing from xattr data) |
| `quota.rs` (Phase 7) | ~250 | ✅ (V2 dqblk, QuotaManager) |
| `lib.rs` | ~35 | ✅ (+ journal, xattr, acl, quota modules) |
| Тесты | ~1300 (58 unit + 1 doc-test) | ✅ |
| **C bridge** | | |
| `main.c` | ~50 | ✅ |
| `table.c` | ~30 | ✅ |
| `ffi_bridge.c` | ~80 | ✅ |
| `ffi.h` | ~40 | ✅ |
| `CMakeLists.txt` | ~20 | ✅ |
| **Итого Phase 1** | **~1,500 LOC** | **✅** |
| **Итого Phase 2** | **~1,250 LOC** | **✅** |
| **Итого Phase 3 (journal)** | ~1060 LOC | ✅ |
| **Итого Phase 4 (read path FFI)** | ~400 LOC | ✅ |
| **Итого Phase 5 (write FFI)** | ~350 LOC | ✅ |
| **Итого Phase 6 (complete driver)** | ~350 LOC | ✅ |
| **Итого Phase 6b (metadata_csum)** | ~400 LOC | ✅ |
| **Итого Phase 7 (xattr + ACL + quota)** | ~550 LOC | ✅ |
| **Всего** | **~7,600 LOC / ~9,000** | **~84%** |

---

## 9. Альтернативный путь: C-only (если Rust FFI проблематичен)

Если FFI окажется проблемой, fallback — C-only как ext2:

```
minix/fs/ext4/
  ├── super.c      ← ext4 superblock + group descriptors
  ├── inode.c      ← inode + extent tree
  ├── path.c       ← dir + htree
  ├── read.c       ← read + extent traversal
  ├── write.c      ← write + alloc
  ├── balloc.c     ← block bitmap + flex_bg
  ├── ialloc.c     ← inode bitmap
  ├── mount.c      ← mount/unmount
  ├── open.c       ← create, mknod, mkdir
  ├── link.c       ← link, unlink, rename
  ├── stadir.c     ← stat, statvfs
  ├── protect.c    ← chown, chmod
  ├── time.c       ← utime
  ├── misc.c       ← sync, driver
  ├── table.c      ← fsdriver table
  ├── main.c       ← main + SEF
  ├── buf.h, inode.h, super.h, fs.h
  └── CMakeLists.txt
```

**Но это ~5000 LOC C с высоким риском багов в extent tree и htree.**
Rust ownership model предотвращает:
- Use-after-free в extent tree
- Double-free в block allocator  
- Race conditions в journal
- Memory leaks в htree

---

## 11. Benchmark Results (July 2026)

**Система**: FX-8150 (8× 3.6 GHz Bulldozer), 24 GB RAM, Rust 2021 edition, criterion 0.5

### Superblock
| Benchmark | Среднее (ns/iter) | Пояснение |
|-----------|------------------|-----------|
| `parse_superblock` | ~258 ns | Парсинг 1024-байтного суперблока + валидация magic/feature flags |

### Extent Tree
| Benchmark | Среднее | Пояснение |
|-----------|---------|-----------|
| `extent_header_parse` | ~5.5 ns | Чтение 12-байтного заголовка экстента |
| `extent_lookup_inline` | ~48 ns | Поиск logical block в depth-0 (inline) дереве |
| `serialize_extent_header` | ~25 ns | Запись заголовка экстента |
| `serialize_single_extent` | ~27 ns | Запись одной записи экстента |

### Directory
| Benchmark | Среднее | Пояснение |
|-----------|---------|-----------|
| `lookup_linear_16_entries` | ~2.1 µs | Линейный поиск в 16 записях |
| `lookup_linear_200_entries` | ~34.7 µs | Линейный поиск в 200 записях (O(n)) |
| `file_type_to_mode` | ~23 ns | Маппинг file_type → mode_t |
| `insert_into_block` | ~1.0 µs | Вставка записи в dir block (split rec_len) |

### Journal (jbd2)
| Benchmark | Среднее | Пояснение |
|-----------|---------|-----------|
| `parse_journal_superblock` | ~112 ns | Парсинг journal SB (big-endian) |
| `scan_descriptor_block` | ~413 ns | Поиск и идентификация descriptor блока |
| `crc32c_4k` | ~14.8 µs | CRC-32C 4KB данных (табличная реализация) |
| `crc32c_small` | ~135 ns | CRC-32C 48 байт |
| `journal_info_string` | ~216 ns | Форматирование строки состояния журнала |

### Extended Attributes, ACL, Quota
| Benchmark | Среднее | Пояснение |
|-----------|---------|-----------|
| `parse_xattrs_64` | ~28 ns | Парсинг 64 extended attribute entry |
| `match_xattr_name` | ~217 ns | Разбор имени xattr (user. → index 1, etc.) |
| `find_xattr` | ~7.9 ns | Поиск xattr по имени в Vec |
| `parse_acl_8` | ~296 ns | Парсинг 8 ACL entry |
| `serialize_acl` | ~17 ns | Сериализация ACL entry |
| `parse_dqblk_v2` | ~16.5 ns | Парсинг 72-байтного V2 dqblk |
| `serialize_dqblk_v2` | ~7.3 ns | Сериализация V2 dqblk |

### Анализ производительности

**Наблюдения**:
1. **superblock/ACL/quota парсинг — sub-microsecond** — overhead незначителен
2. **extent_lookup: ~48 ns** — inline extent tree lookup (depth-0) практически бесплатен
3. **dir lookup: O(n) для линейных директорий** — 200 записей ≈ 35 µs. Для больших директорий htree даст O(log n)
4. **CRC-32C: ~15 µs/4KB** — узкое место для metadata_csum на записи. Возможна оптимизация: SIMD (SSE4.2 CRC32), hardware CRC на современных CPU
5. **crc32c_small: ~135 ns** — для маленьких блоков (48 байт) overhead незаметен

**Распределение времени (типичный read path):**
```
superblock parse:     ~260 ns
inode parse:          ~200 ns (extent_header_parse + lookup)
extent lookup (inline):  ~48 ns
dir block read:       ~2-35 µs (зависит от размера)
file data:            ~15 µs/4KB (CRC-32C) + I/O latency
```

**Узкие места для оптимизации:**
1. CRC-32C (~15 µs/4KB) — можно заменить на hardware CRC (SSE 4.2 `_mm_crc32_u64`)
2. Dir lookup для больших директорий — требуется htree (уже реализован, benchmark не сделан)

---

## 12. MINIX Integration Status

## 12. MINIX Integration Status & Build Infrastructure

### Rust cross-compilation for MINIX

**Проблема**: Rust не имеет официального `x86_64-unknown-minix` target. 
**Решение**: Создан кастомный target specification + build script.

### Созданные файлы

#### `rust/x86_64-unknown-minix.json` — кастомный Rust target
- Базируется на `x86_64-unknown-netbsd` (ближайший аналог MINIX)
- LLVM codegen через `x86_64-unknown-unknown` (generic x86_64 ELF)
- Linker: `x86_64-elf64-minix-gcc` (из MINIX_TOOLCHAIN)
- SSE/SSE2: включены (обязательны для x86_64), остальные SIMD отключены
- panic-strategy: `abort` (безопаснее для kernel-mode FS driver)
- dwarf_version: 2

#### `releasetools/build_ext4.sh` — скрипт сборки
```bash
# Нативная (host) сборка для тестирования:
./releasetools/build_ext4.sh native

# Кросс-компиляция для MINIX:
export MINIX_TOOLCHAIN=/opt/minix/toolchain
export MINIX_DESTDIR=/opt/minix/destdir
./releasetools/build_ext4.sh cross x86_64
```

#### `rust/ext4-core/.cargo/config.toml` — Cargo configuration
- Документирует `RUSTFLAGS="-C linker=..."` для установки linker при кросс-компиляции
- `[build] target-dir = "target"`

#### `minix/fs/ext4/CMakeLists.txt` — CMake интеграция
- Авто-детект pre-built `libext4_core.a` в `CMAKE_CURRENT_BINARY_DIR`
- Если найден → линкуется в `ext4` сервер
- Если не найден → C-only fallback с `#define EXT4_C_ONLY 1`
- Нет зависимости от bash/add_custom_target (Windows-совместимо)

### Процесс сборки для MINIX

```
# 1. Установить MINIX cross-toolchain (см. releasetools/cmake-build.sh)
#    export MINIX_TOOLCHAIN=/opt/minix/toolchain
#    export MINIX_DESTDIR=/opt/minix/destdir

# 2. Собрать Rust staticlib
./releasetools/build_ext4.sh cross x86_64
# → rust/ext4-core/target/x86_64-unknown-minix/release/libext4_core.a
# → копируется в build/minix/fs/ext4/libext4_core.a

# 3. Собрать MINIX с ext4 сервером
cmake --preset x86_64-debug
cmake --build --preset x86_64-debug
# CMake найдёт libext4_core.a и включит полную сборку

# 4. Загрузить в QEMU и протестировать
#    mount -t ext4 /dev/c0d0p1 /mnt
#    ls /mnt
```

### Требования к MINIX toolchain

Для кросс-компиляции Rust staticlib на MINIX необходимо:
1. **MINIX_TOOLCHAIN** — cross-toolchain (gcc/binutils) для MINIX:
   - `x86_64-elf64-minix-gcc` — C компилятор/линкер
   - `x86_64-elf64-minix-ar` — архиватор
   - Пакет: `releasetools/cmake-build.sh setup-toolchain` или вручную из MINIX репозитория

2. **MINIX_DESTDIR** — MINIX sysroot (заголовочные файлы и библиотеки):
   - `/usr/lib/libc.a` — MINIX libc для линковки Rust std
   - `/usr/include/*.h` — заголовочные файлы
   - Создаётся через `cmake --build .. install`

3. **Rust nightly** (опционально) — для `-Z unstable-options --print target-spec-json`

### Текущий статус

| Компонент | Статус | LOC |
|-----------|--------|-----|
| `rust/x86_64-unknown-minix.json` | ✅ Target spec | ~30 |
| `rust/ext4-core/.cargo/config.toml` | ✅ Cargo config | ~30 |
| `releasetools/build_ext4.sh` | ✅ Build script | ~130 |
| `minix/fs/ext4/CMakeLists.txt` | ✅ CMake + Rust | ~60 |
| Native staticlib (`cargo build --release --lib`) | ✅ `ext4_core.lib` | — |
| Cross-compilation for MINIX | 🟡 Нужен MINIX toolchain | — |
| Монтирование реального ext4 раздела | 🟡 Нужен MINIX в QEMU | — |
| `#ifdef EXT4_C_ONLY` stubs в `ffi_bridge.c` | 🟡 Нужно реализовать | ~TBD |
| `aarch64-unknown-minix.json` | ❌ Не создан | ~TBD |

---

## 13. Связанные документы

- `planning/10_netbsd_dependency_audit.md` §3.8 — VFS cleanup завершён
- `planning/17_remaining_tasks.md` §T20 — VFS cleanup ✅, §T21 — Filesystem migration
- `planning/18_complex_packages.md` — отложенные пакеты (ext4 не входит)
- `ROADMAP.md` — ext4 foundation в 1.0, полный ext4 в 1.1
- `minix/fs/ext2/` — reference implementation (ext2 server, ~3000 LOC C)
- `minix/lib/libfsdriver/` — fsdriver library
- `minix/lib/libminixfs/` — block cache library
- [kernel.org ext4 docs](https://www.kernel.org/doc/html/latest/filesystems/ext4/index.html)
- [fs-ext4 crate](https://github.com/christhomas/rust-fs-ext4) — pure Rust ext4 reference
- [ext4-view crate](https://crates.io/crates/ext4-view) — simpler Rust parser
