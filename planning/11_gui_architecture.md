# GUI Architecture for GergiOS

> **Part of**: GergiOS Modernization Roadmap
> **Related**: `planning/03_migration_roadmap.md`, `planning/09_c_language_modernization.md`, `TODO.md §3.2–3.4`
> **Status**: Planning phase
> **Language Strategy**: Rust + C FFI (core), C (drivers), Lua (scripting/GUI logic)

---

## 1. Current State Assessment

### 1.1 Existing Display Infrastructure

GergiOS (MINIX 3) already has some foundational graphics primitives:

| Component | Location | Status |
|-----------|----------|--------|
| **Framebuffer structures** | `minix/include/minix/fb.h` | `fb_fix_screeninfo`, `fb_var_screeninfo`, `fb_bitfield` — базовые Linux-совместимые структуры |
| **Video driver** | `minix/drivers/video/` | Общий видеодрайвер (Makefile + Makefile.inc) |
| **HID driver** | `minix/drivers/hid/` | HID-драйверы (клавиатура, мышь) |
| **TTY driver** | `minix/drivers/tty/` | Терминальный драйвер |
| **FBD control** | `minix/usr.sbin/fbdctl/` | Утилита для управления framebuffer device |
| **Input server** | `minix/servers/` (вероятно) | Сервер ввода |

**Чего НЕ хватает** для полноценной GUI:
- Нет Wayland compositor'а
- Нет аппаратного ускорения (DRI/DRM)
- Нет менеджера окон
- Нет GUI toolkit'а
- Нет шрифтового рендеринга
- Нет курсора мыши (кроме текстового)
- Нет multi-seat / multi-screen

### 1.2 Relevant IPC Types (from `minix/include/minix/ipc.h`)

Микроядро предоставляет IPC-сообщения для:
- `mess_input_tty_event` — события ввода (keyboard, mouse)
- `mess_linputdriver_input_event` — события от input driver'а
- `mess_lsys_krn_sys_devio` — I/O port доступ для VGA регистров
- `mess_lsys_krn_sys_setalarm` — таймеры для vsync

Эти типы уже определены в `minix-rs` (Phase 3) и готовы к использованию.

---

## 2. Architecture Overview

### 2.1 Слои графического стека

```
┌─────────────────────────────────────────────────────┐
│                   GUI Applications                    │
│  (Rust + Lua скрипты, GTK/Qt через C FFI или native) │
├─────────────────────────────────────────────────────┤
│             Wayland Protocol (Rust)                   │
│  (wayland-server-rs + кастомные MINIX-расширения)     │
├─────────────────────────────────────────────────────┤
│               Window Manager / Compositor             │
│  (Rust — композитинг, рендеринг, декор, анимации)    │
├─────────────────────────────────────────────────────┤
│        Graphics Abstraction Layer (Rust + C)          │
│  ┌──────────┐ ┌──────────┐ ┌───────────────────┐    │
│  │ DRM/KMS  │ │ Renderer │ │ Font Renderer     │    │
│  │ (C FFI)  │ │ (wgpu)   │ │ (ttf-parser+swf)  │    │
│  └──────────┘ └──────────┘ └───────────────────┘    │
├─────────────────────────────────────────────────────┤
│             Kernel / Driver Layer (C)                 │
│  ┌──────────┐ ┌──────────┐ ┌───────────────────┐    │
│  │Framebuffer│ │ HID/Input│ │ Timer / VSync     │    │
│  │  driver  │ │  driver  │ │    driver         │    │
│  └──────────┘ └──────────┘ └───────────────────┘    │
├─────────────────────────────────────────────────────┤
│              MINIX Microkernel (C)                    │
│        (IPC, syscalls, memory management)             │
└─────────────────────────────────────────────────────┘
```

### 2.2 Языковая стратегия

| Слой | Язык | Обоснование |
|------|------|-------------|
| **Kernel / Drivers** | C | Существующий код, низкоуровневый доступ к железу |
| **DRM/KMS bridge** | C → Rust FFI | `minix-rs` стиль: C-драйвер + Rust-safe обёртка |
| **Compositor / WM** | Rust | Memory safety в самом сложном коде |
| **Renderer (wgpu)** | Rust | wgpu — Rust-first, кросс-платформенный |
| **Wayland protocol** | Rust | `wayland-server-rs` — зрелая библиотека |
| **GUI скриптинг** | Lua | Встраиваемый, лёгкий, уже в проекте |
| **Приложения** | Rust / Lua | Rust для производительности, Lua для быстрых прототипов |

---

## 3. Компоненты

### 3.1 Framebuffer / DRM Driver (C)

**Текущее состояние**: Есть базовый framebuffer (`minix/drivers/video/`) с Linux-совместимыми структурами в `fb.h`.

**План**:
1. ✅ Аудит существующего video driver'а
2. Портировать `libdrm` (Linux DRM userspace библиотека) на MINIX
3. Реализовать KMS (Kernel Mode Setting) через MINIX IPC
4. Поддержка: VBE/UEFI GOP (x86_64), PL111/HDMI (ARM)

**Ключевые файлы**:
- `minix/drivers/video/fb.c` — framebuffer driver
- `minix/include/minix/fb.h` — framebuffer structures (уже существует!)
- `libdrm` — порт (новый компонент)

### 3.2 Input Driver (C)

**Текущее состояние**: Есть HID-драйвер (`minix/drivers/hid/`) и TTY-драйвер (`minix/drivers/tty/`).

**План**:
1. ✅ Аудит HID/TTY driver'ов
2. Добавить поддержку absolute positioning (тачскрин, планшеты)
3. Добавить multi-touch протокол
4. Обеспечить корректный `EVDEV`-совместимый интерфейс

**Rust-обёртка** (через `minix-rs`):
```rust
// Псевдокод: Rust API для input событий
pub struct InputEvent {
    pub event_type: EventType,  // KEY / ABS / REL
    pub code: u32,              // KEY_ENTER, ABS_X, REL_WHEEL
    pub value: i32,             // key press/repeat/release или координаты
}

pub fn read_input() -> Result<InputEvent, Error>;
```

### 3.3 Wayland Compositor (Rust)

**Сердце GUI**. Композитор на Rust, использующий:
- `wayland-server-rs` — реализация протокола Wayland
- `calloop` — event loop для MINIX IPC (вместо epoll/kqueue)
- `wgpu` — GPU-ускоренный рендеринг (SoftwareFallback: Rust pixmap)
- `smithay` — библиотека для написания Wayland compositor'ов (или прямая работа с `wayland-server-rs`)

**Архитектура compositor'а**:

```
┌────────────────────────────┐
│     Wayland Compositor     │
│  (Rust — gergios-comp)     │
│                            │
│  ┌──────────────────────┐  │
│  │  Event Loop (calloop) │  │ ← IPC receive (minix-rs)
│  ├──────────────────────┤  │
│  │  Surface Tree         │  │ ← wl_surface, wl_subsurface
│  ├──────────────────────┤  │
│  │  Renderer             │  │ ← wgpu → DRM → framebuffer
│  ├──────────────────────┤  │
│  │  Shell / WM logic     │  │ ← xdg_shell, layer_shell
│  ├──────────────────────┤  │
│  │  Input dispatcher     │  │ ← input → keyboard focus
│  └──────────────────────┘  │
└────────────────────────────┘
```

**Протоколы для реализации (MVP)**:
- `wl_compositor`, `wl_surface`, `wl_region`
- `wl_shell` или `xdg_shell` (стабильный)
- `wl_seat`, `wl_keyboard`, `wl_pointer`, `wl_touch`
- `wl_data_device` (copy-paste)
- `wl_output` (multi-monitor)

**Дополнительные протоколы (post-MVP)**:
- `layer-shell` (панели, уведомления)
- `xdg-decoration` (скины окон)
- `wlr-screencopy` (скриншоты, recording)
- `fractional-scale`, `ext-transient-seat`

### 3.4 Рендеринг

**Стратегия: wgpu (Rust) с software fallback**

wgpu — это Rust-реализация WebGPU API. Он поддерживает:
- Vulkan / Metal / DX12 / OpenGL ES (через native)
- **Software fallback** (через `wgpu_hal` с CPU-растеризатором)

На MINIX:
1. **Без GPU**: CPU-рендеринг через software растеризатор (wgpu в режиме `BACKEND_EMULATED`)
2. **С GPU**: Vulkan через `lunarg`-драйверы (VirGL для виртуализации)
3. **С framebuffer только**: прямой 2D-рендеринг в pixmap (через Rust pixmap crate)

```rust
// Пример: software-рендеринг в framebuffer
struct SoftwareRenderer {
    fb: &mut [u8],          // mmap'ленный framebuffer
    width: u32,
    height: u32,
    stride: u32,
}

impl SoftwareRenderer {
    fn clear(&mut self, color: [u8; 4]) { /* ... */ }
    fn blend_surface(&mut self, surface: &Surface, x: i32, y: i32) {
        // Альфа-блендинг wl_surface в framebuffer
    }
    fn present(&mut self) {
        // vsync через MINIX syscall SYS_SETALARM
    }
}
```

### 3.5 Font Renderer

**Rust решения** (без C-FFI):
- `ttf-parser` — парсинг TTF/OTF (pure Rust, no_std, используется в Firefox)
- `rustybuzz` — шейпинг (harfbuzz на Rust)
- `swash` — advance font rasterization (subpixel, hinting)
- `ab_glyph` — простая rasterization для UI текста

**Стэк**:
```
TTF/OTF → ttf-parser → rustybuzz (шейпинг) → swash/ab_glyph (raster) → pixmap
```

### 3.6 Cursor / Pointer

- Software cursor (рендеринг курсора в framebuffer compositor'ом)
- Hardware cursor (если KMS/DRM поддерживает)
- XCursor темы (`xcursor` формат или `cursor-rs` crate)

### 3.7 GUI Toolkit

**Варианты для приложений**:

| Вариант | Описание | Плюсы | Минусы |
|---------|----------|-------|--------|
| **GTK** (C FFI) | Портировать GTK на MINIX | Зрелый, много виджетов | Тяжёлый, C dependency |
| **Qt** (C++ FFI) | Портировать Qt | Мощный, QML | C++ ABI проблемы на MINIX |
| **Druid / Xilem** (Rust) | Native Rust GUI | Лёгкий, без C | Молодой, мало виджетов |
| **Slint** (Rust) | Декларативный UI | .slint файлы, lvgl-style | Мало известен |
| **egui** (Rust) | Immediate mode | Быстрый прототипинг | Не подходит для сложных окон |
| **iced** (Rust) | Elixir-inspired | Elm архитектура | В разработке |

**Рекомендация**: **Slint** для MVP (декларативный UI, встроенный рендеринг, Rust-first) + **egui** для инструментов (debug панели, мониторинг).

### 3.8 Lua Скриптинг

Lua интегрируется через:
- `mlua` или `rlua` — Rust крейты для встраивания Lua
- Lua скрипты для: конфигурация compositor'а, layout окон, простая анимация
- Пример: конфиг compositor'а на Lua (как AwesomeWM делает)

```lua
-- example compositor config
panel {
    position = "top",
    height = 32,
    widgets = {
        clock(format = "%H:%M"),
        battery(),
        wifi(),
    }
}

keybind({"Super", "Return"}, function()
    launch("terminal")
end)

keybind({"Super", "d"}, function()
    launch_app_launcher()
end)
```

---

## 4. Фазы реализации

### Phase 1: Foundation (C + Rust FFI) — 3-4 месяца

- [ ] **1.1** Портировать `libdrm` userspace на MINIX
- [ ] **1.2** Создать Rust-safe DRM bindings (`minix-drm-sys` + `minix-drm`)
- [ ] **1.3** Реализовать KMS (Kernel Mode Setting) на framebuffer driver
- [ ] **1.4** Создать Rust-safe Input bindings (`minix-input`) на основе `minix-rs`
- [ ] **1.5** Получить рабочий framebuffer mmap из Rust
- [ ] **1.6** MVP: программа на Rust, которая рисует пиксели на экране через `minix-rs`

### Phase 2: Software Renderer + Fonts — 2-3 месяца

- [ ] **2.1** Software rasterizer для 2D (Rust pixmap ops: fill, blend, blit)
- [ ] **2.2** Font rendering стэк: `ttf-parser` + `rustybuzz` + `swash`
- [ ] **2.3** Вывод текста на экран (UTF-8, left-to-right, bi-directional)
- [ ] **2.4** Базовая композиция (overlay, alpha-blending)
- [ ] **2.5** Курсор мыши (software cursor)
- [ ] **2.6** Демо: Rust программа с текстом и простой анимацией

### Phase 3: Wayland Compositor MVP — 4-6 месяцев

- [ ] **3.1** Event loop: `calloop` с MINIX IPC бэкендом
- [ ] **3.2** Wayland protocol: `wayland-server-rs` на MINIX
- [ ] **3.3** `wl_compositor` + `wl_surface` + `subcompositor`
- [ ] **3.4** `xdg_shell` (стабильный) — окна с заголовками
- [ ] **3.5** `wl_seat` + input (keyboard focus, pointer, touch)
- [ ] **3.6** `wl_data_device` (copy-paste между окнами)
- [ ] **3.7** Shell с запуском клиентских приложений
- [ ] **3.8** Демо: терминал под compositor'ом

### Phase 4: Window Manager — 2-3 месяца

- [ ] **4.1** Tiling window manager (как i3/sway — естественно для микроядра)
- [ ] **4.2** Плавающие окна (dragging, resize, snapping)
- [ ] **4.3** Window decorations (Rust software render)
- [ ] **4.4** Workspaces / Tags
- [ ] **4.5** Keyboard shortcuts (configurable через Lua)
- [ ] **4.6** Panel / Status bar (Lua-скриптуемый)

### Phase 5: GUI Toolkit — 3-4 месяца

- [ ] **5.1** Выбор toolkit: **Slint** или **iced** как recommended
- [ ] **5.2** Адаптация toolkit для Wayland на MINIX
- [ ] **5.3** Demo приложения: file manager, text editor, calculator
- [ ] **5.4** Lua GUI bindings (скриптовые UI на Lua)
- [ ] **5.5** Темы, шрифты, локализация

### Phase 6: Hardware Acceleration — 3-6 месяцев

- [ ] **6.1** Vulkan software fallback (проект Mesa Lavapipe)
- [ ] **6.2** VirGL для виртуальных GPU (QEMU, VirtualBox)
- [ ] **6.3** wgpu hardware backend (если есть GPU драйверы)
- [ ] **6.4** GPU-accelerated compositing
- [ ] **6.5** Smooth animations (60fps vsync)

---

## 5. Ключевые решения

### 5.1 Почему Rust для compositor'а, а не C++ или Qt?

| Критерий | Rust | C++ | Qt QML |
|----------|------|-----|--------|
| Memory safety | ✅ | ❌ UAF, use-after-free | 🟡 GC-like |
| C FFI совместимость | ✅ `extern "C"` | 🟡 ABI проблемы | 🟡 |
| Размер бинарника | 🟡 1-5 MB | 🟡 5-10 MB | 🔴 30+ MB |
| Wayland биндинги | ✅ `wayland-server-rs` | ✅ `wayland-server` | 🟡 косвенно |
| GPU acceleration | ✅ wgpu | ✅ Vulkan | 🔴 Qt specific |
| MINIX IPC интеграция | ✅ `minix-rs` (готово) | ❌ нет | ❌ нет |
| Cross-compilation | ✅ rustc | 🟡 сложно | 🔴 |

**Rust — единственный язык, который даёт memory safety + прямой доступ к MINIX IPC + GPU acceleration + кросс-компиляцию.**

### 5.2 Software Rendering Strategy

До появления аппаратного ускорения:

```
Rust software renderer
  → pixmap [u8; width * height * 4]
    → software compositing (alpha blending, transforms)
      → memcpy в framebuffer mmap
        → SYS_SETALARM для vsync
```

Производительность (оценка):
- 800×600 @ 30fps: ~14 Mpx/s — достижимо на software
- 1280×720 @ 24fps: ~22 Mpx/s — достижимо с оптимизациями
- 1920×1080 @ 60fps: ~124 Mpx/s — нужен GPU

Для сравнения: netbook EeePC 701 (Celeron M 900MHz) мог выводить 1024×600 @ 30fps через pure software.

---

## 6. Структура репозитория (предлагаемая)

```
rust/
├── compositor/              # Wayland compositor
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs          # Entry point
│       ├── backend/
│       │   ├── drm.rs       # DRM/KMS бэкенд (C FFI)
│       │   ├── fb.rs        # Framebuffer fallback
│       │   └── input.rs     # Input device handling
│       ├── render/
│       │   ├── software.rs  # Software 2D renderer
│       │   ├── font.rs      # Font rendering
│       │   └── cursor.rs    # Cursor compositing
│       ├── shell/
│       │   ├── layout.rs    # Tiling layout
│       │   ├── floating.rs  # Floating windows
│       │   └── panel.rs     # Status bar
│       └── lua_config.rs    # Lua scripting integration
├── toolkit/                 # GUI toolkit (Slint/iced)
│   ├── Cargo.toml
│   └── src/
├── gergios-apps/            # First-party GUI apps
│   ├── terminal/            # Rust terminal emulator
│   ├── file-manager/
│   ├── calculator/
│   └── settings/
├── minix-rs/                # MINIX FFI bindings (готово)
└── minix-drm/               # DRM bindings (Phase 1)
    ├── Cargo.toml
    ├── minix-drm-sys/       # C FFI bindings
    └── src/
```

---

## 7. Зависимости (Crates)

### Phase 1-2 (Foundation):
```toml
minix-rs = { path = "../minix-rs" }
# DRM bindings
minix-drm = { path = "../minix-drm" }
# Software rendering
pixmap = "0.1"     # Simple 2D pixmap operations
# Font rendering
ttf-parser = "0.25"
rustybuzz = "0.18"
swash = "0.2"
```

### Phase 3-4 (Wayland):
```toml
wayland-server = "0.31"      # Wayland protocol implementation
calloop = "0.13"             # Event loop (MINIX IPC backend)
temp-allocate = "1"          # DMA-buf allocation
```

### Phase 5-6 (Toolkit + GPU):
```toml
slint = { version = "1", features = ["wayland"] }
# ИЛИ
iced = { version = "0.13", features = ["wayland"] }
# GPU
wgpu = "23"
```

---

## 8. Риски

| Риск | Impact | Mitigation |
|------|--------|------------|
| Wayland compositor на Rust никто не делал для MINIX | Высокий | Начать с software рендеринга, Wayland layer by layer |
| `calloop` без epoll на MINIX | Средний | Написать MINIX backend через `_ipc_sendrec` + SYS_SETALARM |
| Нет Vulkan драйверов для железа MINIX | Средний | Software fallback через wgpu CPU backend |
| Framebuffer без KMS/DRM | Низкий | Использовать существующий fbdev через `minix/drivers/video` |
| Производительность software рендеринга | Средний | Оптимизация через SIMD (`core::simd`), async compute |

---

## 9. Success Criteria

1. **Phase 1**: Rust программа рисует пиксели на framebuffer через `minix-rs` FFI
2. **Phase 2**: Текст и простые UI элементы выводятся на экран
3. **Phase 3**: Wayland-совместимое приложение работает под compositor'ом
4. **Phase 4**: Tiling WM с Lua-конфигурацией, переключение окон
5. **Phase 5**: GUI приложения на Rust (терминал, файловый менеджер)
6. **Phase 6**: GPU-ускоренный композитинг, 60fps

---

## 10. Related Documents

- `planning/09_c_language_modernization.md` — C→Rust migration, Phase 3 (minix-rs)
- `planning/04_target_architecture_support.md` — x86_64 and ARM targets
- `TODO.md` §3.2–3.4 — Display, Input, Window Management
- `minix/include/minix/fb.h` — Existing framebuffer structures
- `minix/drivers/video/` — Existing video driver
- `minix/drivers/hid/` — Existing HID driver
