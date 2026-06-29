# ext4 Driver Architecture — GergiOS 1.0+/1.1

> **Статус**: Phase 1 ✅, Phase 2 ✅, Phase 3 ✅ (June 2026)
> **Связанные**: `planning/10_netbsd_dependency_audit.md` (§3.8 Phase 7 ✅), `planning/17_remaining_tasks.md` (§T20 ✅, §T21)
> **Репозиторий**: `rust/ext4-core/` (29 unit tests, 0 errors)
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
- [x] `cargo test` — 18 unit tests, all PASS
- [ ] `cargo bench` — производительность (отложено)
- [ ] Монтирование реального ext4 раздела в MINIX (ждёт интеграции)

**Зависимости**: Rust toolchain ✅, CMake Rust integration ✅

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
- [x] **Symlink support** (`inode.rs`): `set_symlink_target()` / `get_symlink_target()` — fast symlinks (target в i_block, до 60 байт)
- [x] **Truncate** (`extent.rs`): `extent_truncate()` — удаление/укорачивание extent-ов за new_size, free_blocks_cb, set_file_size
- [x] **Timestamps** (`inode.rs`): `update_timestamps_ns()` — kernel-compatible nanosecond precision (extra[2:31] = ns, extra[0:1] = extra seconds)
- [x] **Reserved inodes**: `new_reserved_inode()` — создание зарезервированных inode (root, etc.)

**Колбеки fsdriver**:
- [ ] Все write-колбеки `fdr_create`, `fdr_mkdir`, `fdr_write`, `fdr_link`, etc. — deferred до Phase 4 (C bridge integration)

**Финальные дополнения Phase 2:**
- [x] **i_blocks_hi поддержка**: новое поле `i_blocks_hi: u32` в `Ext4Inode`, `blocks_count()` getter, `set_blocks_count()` теперь пишет обе части
- [x] **Link/unlink helpers**: `inode_link()` (inc links_count), `inode_unlink()` (dec, возвращает true если 0), `mark_inode_deleted()` (clear mode/flags, set dtime)
- [x] **Free inode**: `free_inode_data()` — truncate extent tree до 0 + free_blocks_cb + free_inode_cb

**Оставшаяся deferred функциональность:**
- [ ] Htree update (directory index tree write)
- [ ] fsdriver write-колбеки fdr_create, fdr_mkdir, fdr_write, fdr_link и т.д.

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

**Не реализовано (deferred):**
- [ ] Checksum validation (V2/V3 checksums in commit/descriptor blocks)
- [ ] Asynchronous journal commit
- [ ] Journal checkpointing (trim committed transactions)

---

### Phase 4: Advanced Features ⬜ 4-8 weeks (1.1+)

**Цель**: Паритет с Linux ext4 для типовых сценариев.

- [ ] Delayed allocation (allocate on flush)
- [ ] Online defragmentation
- [ ] Extended attributes (user, system, security)
- [ ] ACLs (POSIX)
- [ ] Quota support
- [ ] Project quota (for containers)
- [ ] fsck integration
- [ ] resize (online + offline)

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
| `dir.rs` | ~250 | ✅ (+ linear + htree detection + insert/remove/init dir block) |
| `block.rs` | ~100 | ✅ |
| `alloc.rs` (Phase 2) | ~220 | ✅ (block allocator, bitmaps) |
| `ialloc.rs` (Phase 2) | ~160 | ✅ (inode allocator, bitmaps) |
| `ffi.rs` | ~310 | ✅ (+ InvalidJournal, JournalCorrupt errors) |
| `journal.rs` (Phase 3) | ~460 | ✅ (jbd2 parsing + recovery, 8 tests) |
| `lib.rs` | ~35 | ✅ (+ journal module) |
| Тесты | ~750 (29 unit tests) | ✅ |
| **C bridge** | | |
| `main.c` | ~50 | ✅ |
| `table.c` | ~30 | ✅ |
| `ffi_bridge.c` | ~80 | ✅ |
| `ffi.h` | ~40 | ✅ |
| `CMakeLists.txt` | ~20 | ✅ |
| **Итого Phase 1** | **~1,500 LOC** | **✅** |
| **Итого Phase 2** | **~1,000 LOC** | **✅** |
| **Итого Phase 3 (journal)** | ~460 LOC | ✅ |
| **Итого Phase 4 (advanced)** | ~1,500 LOC | ⬜ |
| **Всего** | **~5,000 LOC / ~8,000** | **~62%** |

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

## 10. Связанные документы

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
