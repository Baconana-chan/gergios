# GergiOS Project Website

> **Стек**: Rust + Lua + Wayland-стиль UI
> **Статус**: Планирование
> **Часть**: GergiOS Modernization Roadmap — Community & Ecosystem

---

## 1. Концепция

Сайт проекта GergiOS — это не просто статическая страница с документацией.
Это **интерактивная витрина** операционной системы, построенная на тех же
технологиях, что и сама ОС:

| Технология | Где в ОС | Где на сайте |
|-----------|----------|-------------|
| **Rust** | Compositor, drivers, userspace | Backend (Axum) + Frontend (WASM) |
| **Lua** | GUI scripting, compositor config | Content management, dynamic pages |
| **Wayland** | Протокол оконного менеджера | Композитинг в браузере через WASM |
| **MINIX IPC** | Межпроцессное взаимодействие | Архитектура бэкенда (actor model) |

**Девиз**: «Сайт работает так же, как работает GergiOS».

---

## 2. Архитектура

```
┌──────────────────────────────────────────────────────────┐
│                     Browser (WASM)                        │
│  ┌────────────────────────────────────────────────────┐   │
│  │   Dioxus/Leptos UI (Rust → WASM)                   │   │
│  │   ┌────────┐ ┌──────────┐ ┌───────────────────┐    │   │
│  │   │ Pages  │ │Widgets   │ │ Terminal Emulator │    │   │
│  │   └────────┘ └──────────┘ └───────────────────┘    │   │
│  └────────────────────────────────────────────────────┘   │
│                           ↕ WASM IPC (postMessage)         │
│  ┌────────────────────────────────────────────────────┐   │
│  │   Wayland Compositor Demo (WASM)                    │   │
│  │   — surface compositing                             │   │
│  │   — window manager demo                             │   │
│  │   — input simulation                                │   │
│  └────────────────────────────────────────────────────┘   │
│                           ↕ HTTP/SSE                       │
└──────────────────────────────────────────────────────────┘
                           ↕ HTTP/WebSocket
┌──────────────────────────────────────────────────────────┐
│              Rust Backend (Axum)                          │
│  ┌────────────┐ ┌──────────────┐ ┌──────────────────┐   │
│  │ API Routes │ │ Lua Engine   │ │ Build Monitor    │   │
│  │ (REST+SSE) │ │ (mlua)       │ │ (CI/CD pipeline) │   │
│  └────────────┘ └──────────────┘ └──────────────────┘   │
│  ┌──────────────────────────────────────────────────┐    │
│  │  Content Store (git-based, lunar-тексты)         │    │
│  └──────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────┘
```

### 2.1 Компоненты

#### Rust Backend (Axum)

```rust
// Псевдокод роутов
async fn api_documentation(path: Path<String>) -> impl IntoResponse {
    // Загружает .lua или .md файл из content store
    let page = lua_engine.render(&path).await?;
    Html(page)
}

async fn api_build_status() -> impl IntoResponse {
    // SSE stream: real-time CI/CD лог
    Sse::new(build_monitor.stream())
}

async fn api_package_search(query: Query<String>) -> impl IntoResponse {
    // Поиск по packages.install / pkgsrc
    Json(package_db.search(&query))
}

async fn ws_terminal() -> impl IntoResponse {
    // WebSocket терминал в браузере
    let ws = WebSocket::upgrade();
    // spawn: serial → JS term → WebSocket → kernel build log
}
```

#### Lua Engine (mlua)

```lua
-- Пример: страница документации на Lua
return Page {
    title = "Getting Started with GergiOS",
    layout = "docs",
    
    sections = {
        Section {
            header = "Building from Source",
            content = markdown([[
                1. Install prerequisites
                2. Clone the repository
                3. Run `./build.sh`
            ]]),
            code_block = "bash",
            interactive = true,  -- запускается в WASM terminal
        },
        
        Section {
            header = "Your First Rust Driver",
            content = code_preview("rust/drivers/e1000/src/lib.rs"),
        },
    },
    
    related_pages = {
        "Architecture Overview",
        "Porting Guide",
        "Contributing",
    },
}
```

#### Wayland Compositor Demo (WASM)

Компиляция `minix-wayland` + `minix-compositor` в WASM:

```rust
// Псевдокод: WASM entry
#[wasm_bindgen]
pub struct WasmCompositor {
    server: WaylandServer,
    canvas: Canvas,
}

#[wasm_bindgen]
impl WasmCompositor {
    pub fn new(width: u32, height: u32) -> Self {
        let comp = Rc::new(RefCell::new(Compositor::new(width, height)));
        let mut server = WaylandServer::new(comp.clone());
        setup_default_handlers(&mut server);
        setup_compositor_handlers(&mut server);
        // ...
        Self { server, canvas: ... }
    }
    
    pub fn on_click(&mut self, x: i32, y: i32) {
        // Симуляция ввода → dispatch Wayland события
        self.server.process_input_event(&InputEvent::MouseButton {
            button: MouseButton::Left,
            pressed: true,
            x, y,
            modifiers: Modifiers::new(),
        });
        // Composite → render to canvas
        let stats = self.server.compositor.borrow_mut().composite(None);
        // Blit output to HTML5 canvas
    }
}
```

---

## 3. UI/UX Design — «Терминал встречает Web»

### 3.1 Визуальный стиль

**Тема**: «Cyberpunk Terminal» — наследие MINIX 3 с современным Rust-акцентом.

```
┌─────────────────────────────────────────────────┐
│ ● ● ●  GergiOS v0.2.0 — [build 2026-07-13]     │
├─────────────────────────────────────────────────┤
│                                                 │
│  ┌─────────────────────────────────────────┐   │
│  │  $ _                                     │   │
│  │  Welcome to GergiOS                      │   │
│  │  Type 'help' for available commands      │   │
│  │  > _                                      │   │
│  └─────────────────────────────────────────┘   │
│                                                 │
│  ┌──────┐ ┌──────┐ ┌──────┐ ┌──────────────┐  │
│  │ Docs │ │ Build│ │ PKG  │ │ Interactive   │  │
│  │      │ │Status│ │Search│ │   Demo       │  │
│  └──────┘ └──────┘ └──────┘ └──────────────┘  │
│                                                 │
│  ═══════════════════════════════════════════════ │
│  Latest build: PASS ✓ | 1342 tests | 0.3s       │
│  Packages: 8472 | Rust crates: 156              │
│  Contributors: 12 | Commits: 3,847              │
│  ═══════════════════════════════════════════════ │
│                                                 │
└─────────────────────────────────────────────────┘
```

**Цветовая схема**:
- Фон: `#0a0e14` (тёмно-синий/чёрный, как в терминале)
- Текст: `#b3b1ad` (светло-серый)
- Акцент: `#ff8f40` (оранжевый — как в MINIX kernel log)
- Зелёный: `#41b883` (успешные сборки, онлайн)
- Красный: `#e3342f` (ошибки, critical)
- Синтаксис кода: Gruvbox Dark

**Типографика**:
- Моноширинный: `JetBrains Mono` / `Fira Code` (для кода и UI)
- Заголовки: `Plus Jakarta Sans` (для контраста)

### 3.2 Ключевые страницы

#### `/` — Terminal Dashboard

Приветственный экран в стиле `tail -f /var/log/messages`:

```lua
return TerminalPage {
    boot_animation = true,  -- псевдо-загрузка ядра
    
    lines = {
        "[    0.000] GergiOS v0.2.0 starting...",
        "[    0.001] Architecture: x86_64, ARM64",
        "[    0.002] Kernel: MINIX 3.4.0 + Rust modules",
        "[    0.010] CPU: 8 cores @ 2.4GHz",
        "[    0.050] Memory: 16 GB available",
        "[    1.200] Compositor: minix-compositor v0.1 (Wayland)"
        "[    1.300] Services: PM, VFS, RS, DS, MACD, AUDITD",
        "[    1.500] Network: lwIP + WireGuard + DTLS",
        "[    2.000] System ready. Type 'help' for commands.",
        "",   
        "    ╔══════════════════════════════════════════╗",
        "    ║        Welcome to GergiOS               ║",
        "    ║     A modern MINIX 3 distribution        ║",
        "    ╚══════════════════════════════════════════╝",
        "",
    },
    
    commands = {
        help   = { description = "Show available commands" },
        docs   = { description = "Documentation browser" },
        build  = { description = "Build status dashboard" },
        demo   = { description = "Interactive compositor demo" },
        pkg    = { description = "Package search" },
        about  = { description = "About GergiOS" },
    },
}
```

#### `/docs` — Documentation Browser

Браузер документации в стиле `man`:

```
┌─────────────────────────────────────────────────┐
│ ● ● ●  GergiOS Documentation                     │
├─────────────────────────────────────────────────┤
│                                                   │
│  Search: [_wayland compositor_______] [🔍]       │
│                                                   │
│  ┌─ Categories ───────┐ ┌─ Content ───────────┐  │
│  │ 📘 Getting Started │ │                     │  │
│  │ 📗 Architecture    │ │ # Wayland Compositor│  │
│  │ 📕 Kernel          │ │                     │  │
│  │ 📙 Drivers         │ │ The GergiOS Wayland │  │
│  │ 📐 API Reference   │ │ compositor is built │  │
│  │ 📊 Performance     │ │ on minix-compositor │  │
│  │ 🔒 Security        │ │ ...                 │  │
│  │ 📦 Packages        │ │                     │  │
│  └────────────────────┘ │ [run example ▸]     │  │
│                         └──────────────────────┘  │
└─────────────────────────────────────────────────┘
```

Фичи:
- **Live примеры**: каждый code block можно запустить в WASM
- **man-стиль**: клавиши `j/k` для навигации, `/` для поиска
- **Lua-контент**: каждая страница — Lua-скрипт с разметкой

#### `/build` — Build Dashboard

Real-time CI/CD монитор:

```lua
return BuildDashboard {
    branches = {
        main = {
            status = "passing",
            last_commit = "feat(net): add WireGuard support",
            tests = { total = 1342, passed = 1342, failed = 0 },
            duration = "0.3s",
        },
        develop = {
            status = "building",
            current_step = "Compiling wolfSSL with DTLS...",
            progress = 78,  -- percent
        },
    },
    
    recent_builds = {
        { id = "#847", status = "pass", time = "2m ago", branch = "main" },
        { id = "#846", status = "pass", time = "15m ago", branch = "main" },
        { id = "#845", status = "fail", time = "1h ago", branch = "feature/bt" },
    },
}
```

#### `/demo` — Interactive Compositor Demo

** killer feature ** — полноценный Wayland compositor в браузере!

- Компиляция `minix-wayland` + `minix-compositor` в WASM
- HTML5 Canvas как framebuffer
- Drag-n-drop окон, ресайз, z-order
- Live editing Lua-конфига compositor'а

```rust
// WASM bindings (псевдокод)
#[wasm_bindgen]
pub fn create_demo() -> JsValue {
    // Инициализация compositor'а c Wayland сервером
    // ...
    // Возвращает JS объект с методами:
    JsValue::from(WasmDemo {
        // on_mouse_move(x, y),
        // on_click(button, x, y),
        // on_key(keycode, pressed),
        // composite() → ImageData,
        // set_lua_config(code: &str),
    })
}
```

**Что можно делать в демо**:
1. Создавать окна (клик → wl_surface → commit)
2. Перетаскивать окна (wl_pointer.motion)
3. Редактировать Lua-конфиг и видеть изменения live
4. Смотреть композитинг в real-time (alpha blending, z-order)
5. Загружать свои TTF-шрифты (через ttf-parser → WASM)

#### `/pkg` — Package Browser

```
┌─────────────────────────────────────────────────┐
│ ● ● ●  Package Browser                           │
├─────────────────────────────────────────────────┤
│                                                   │
│  Search: [e1000______________________] [🔍]       │
│                                                   │
│  Results: 3 packages                              │
│                                                   │
│  ┌────────────────────────────────────────────┐  │
│  │ e1000 — Intel PRO/1000 Network Driver      │  │
│  │ Rust ⋆ Network ⋆ Driver                    │  │
│  │ Version: 0.1.0 | Deps: minix-rs, alloc     │  │
│  │ [View Source ▸] [Dependencies ▸]           │  │
│  └────────────────────────────────────────────┘  │
│  ┌────────────────────────────────────────────┐  │
│  │ e1000 — FreeBSD e1000 driver port          │  │
│  │ C ⋆ Driver ⋆ Network                       │  │
│  │ Version: 1.0.0 | Deps: pci, ifnet          │  │
│  └────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────┘
```

#### `/security` — Security Dashboard

Live мониторинг security:

- MAC policy browser (интерактивный просмотр правил)
- Audit log viewer (в стиле `journalctl -f`)
- Capability matrix (граф привилегий)
- CVE tracker для зависимостей

---

## 4. Техническая реализация

### 4.1 Rust Backend

| Компонент | Крейт | Назначение |
|-----------|-------|------------|
| HTTP Server | `axum` | REST API + SSE + WebSocket |
| Templating | `minijinja` / `tera` | HTML-шаблоны |
| Lua Engine | `mlua` | Выполнение .lua-страниц |
| Content | `git2` | Git-based CMS |
| Markdown | `comrak` | MD → HTML |
| Build Monitor | `tokio` + `async-process` | CI/CD pipeline |
| Search | `tantivy` / `meilisearch` | Полнотекстовый поиск |

### 4.2 WASM Frontend

| Компонент | Крейт | Назначение |
|-----------|-------|------------|
| UI Framework | `dioxus` / `leptos` | Реактивный UI |
| Terminal | `xterm.js` (JS FFI) | Эмуляция терминала |
| Compositor | `minix-compositor` (WASM) | Демо композитора |
| Editor | `monaco` / `codemirror` (JS FFI) | Редактор кода |
| HTTP Client | `reqwest` (WASM) | API-запросы |
| Router | `dioxus-router` | Клиентская маршрутизация |

### 4.3 Lua Content

```lua
-- docs/getting-started.lua — пример файла контента

return DocPage {
    title = "Getting Started",
    layout = "guide",
    tags = { "beginner", "build", "quickstart" },
    
    -- Метаданные для SEO
    meta = {
        description = "How to build and run GergiOS",
        keywords = "gergios, minix, build, rust",
        published = "2026-07-01",
        author = "Gergios Team",
    },
    
    -- Содержимое страницы (секции)
    content = {
        intro = [[
            GergiOS is a modern MINIX 3 distribution with Rust userspace.
            This guide will help you build it from source.
        ]],
        
        prerequisites = {
            title = "Prerequisites",
            items = {
                "A Unix-like system (Linux, macOS, WSL2)",
                "Clang 16+, cmake 3.25+, ninja",
                "Rust 1.80+ (rustup recommended)",
                "QEMU 8+ (for testing)",
            },
        },
        
        building = {
            title = "Building",
            code = [[
                git clone https://github.com/gergios/gergios.git
                cd gergios
                ./build.sh -j$(nproc)
                # Build output: release/gergios.iso
            ]],
            
            -- Интерактивный блок: можно запустить в WASM
            interactive = true,
        },
        
        running = {
            title = "Running in QEMU",
            code = [[
                ./scripts/qemu-aarch64.sh  # ARM64
                ./scripts/qemu-x86_64.sh   # x86_64
            ]],
            
            -- Кнопка запуска в браузере (через WASM + V86)
            run_in_browser = true,
        },
    },
    
    -- Связанные страницы
    related = {
        "Architecture Overview",
        "Porting to New Hardware",
        "Contributing Guide",
    },
}
```

### 4.4 Структура репозитория сайта

```
site/
├── Cargo.toml               # Workspace: backend + frontend
├── src/
│   ├── main.rs              # Axum server entry
│   ├── routes/
│   │   ├── mod.rs
│   │   ├── docs.rs          # Documentation routes
│   │   ├── build.rs         # Build status routes
│   │   ├── demo.rs          # Compositor demo route
│   │   └── api.rs           # REST API routes
│   ├── lua/
│   │   ├── mod.rs           # Lua engine wrapper
│   │   ├── page.rs          # Page rendering
│   │   └── widgets.rs       # Reusable Lua widgets
│   ├── content/
│   │   ├── mod.rs           # Content store (git-based)
│   │   └── search.rs        # Full-text search
│   └── monitor/
│       ├── mod.rs           # Build pipeline monitor
│       └── ci.rs            # CI/CD integration
├── wasm/
│   ├── Cargo.toml           # WASM crete
│   └── src/
│       ├── lib.rs           # WASM entry
│       ├── compositor.rs    # minix-compositor → WASM
│       ├── terminal.rs      # Terminal emulator
│       └── editor.rs        # Code editor (Monaco FFI)
├── content/                 # Lua content files
│   ├── index.lua
│   ├── docs/
│   │   ├── getting-started.lua
│   │   ├── architecture.lua
│   │   └── ...
│   ├── blog/
│   │   ├── 2026-07-01-wireguard.lua
│   │   └── ...
│   └── security/
│       └── mac-policy.lua
├── static/                  # Static assets
│   ├── fonts/
│   ├── css/
│   └── js/
└── templates/               # HTML templates (for SSR)
    ├── base.html
    ├── page.html
    └── terminal.html
```

---

## 5. Интерактивные фичи (WOW-фактор)

### 5.1 Wayland Compositor в браузере

Это **главная фича**. Пользователь видит и может взаимодействовать
с настоящим Wayland compositor'ом, работающим в его браузере через WASM.

```rust
// how it works
// 1. minix-compositor компилируется в .wasm
// 2. HTML5 Canvas выступает как framebuffer
// 3. События мыши/клавиатуры маппятся в InputEvent
// 4. Каждый кадр: composite() → ImageData → putImageData()

#[wasm_bindgen]
impl WasmCompositor {
    pub fn composite_frame(&mut self) -> Vec<u8> {
        // 1. Process pending Wayland requests
        self.server.tick();
        
        // 2. Composite all surfaces
        self.compositor.borrow_mut().composite(None);
        
        // 3. Return RGBA pixel data for the canvas
        self.compositor.borrow().output.data.clone()
    }
}
```

**Что можно делать**:
- Создавать/удалять окна через API
- Перетаскивать окна мышью
- Редактировать конфиг на Lua и видеть изменения live
- Загружать свои шрифты
- Open Source: можно форкнуть и встроить в свой сайт

### 5.2 Live Build Log

```
Build #847 | main | feat(net): add WireGuard support
────────────────────────────────────────────────────
[00:00] Cloning repository... ✓
[00:02] Configuring CMake... ✓
[00:05] Building kernel... ✓
[00:15] Building wolfSSL... ✓
[00:20] Building Rust userspace... 
  → minix-wayland [OK]
  → minix-compositor [OK]  
  → e1000 [OK]
  → virtio-net [BUILDING...]  ◉
[00:25] Running tests...
  → Kernel tests: 456/456 ✓
  → Rust tests: 1342/1342 ✓
  → Network tests: 89/89 ✓
[00:30] Build complete ✓
────────────────────────────────────────────────────
Duration: 30.2s | Size: 42MB | 3 warnings
```

SSE-stream реального CI/CD пайплайна.

### 5.3 Lua REPL в браузере

```lua
-- Интерактивная Lua-консоль на сайте
> for i = 1, 10 do
*   print("Hello from GergiOS #" .. i)
* end
Hello from GergiOS #1
Hello from GergiOS #2
...
```

Через WASM + `mlua` — полноценный Lua интерпретатор в браузере.

### 5.4 Interactive Capability Matrix

Визуализация Mandatory Access Control:

```
┌──────────────┬─────┬─────┬─────┬─────┬─────┐
│              │ VFS │ PM  │ RS  │ NET │ DRV │
├──────────────┼─────┼─────┼─────┼─────┼─────┤
│ IPC_SEND     │  ✓  │  ✓  │  ✓  │  ✓  │  ✓  │
│ FILE_ACCESS  │  ✓  │  ✗  │  ✗  │  ✗  │  ✗  │
│ RAWIO        │  ✗  │  ✗  │  ✗  │  ✓  │  ✓  │
│ PROC_KILL    │  ✗  │  ✓  │  ✓  │  ✗  │  ✗  │
│ PRIVCTL_SET  │  ✗  │  ✗  │  ✓  │  ✗  │  ✗  │
└──────────────┴─────┴─────┴─────┴─────┴─────┘
[Edit in Lua ▸]  [Export as policy ▸]
```

Клик на ячейку → показывает соответствующее правило из macd.conf.

### 5.5 Code Explorer

```
rust/minix-wayland/src/
├── server.rs ───────────────────────────────────────
│ fn setup_seat_handlers() {                         │
│     dispatcher.on_seat(|conn, msg| {              │
│         match msg.opcode {                        │
│             GET_POINTER => create_pointer(conn),   │
│             GET_KEYBOARD => create_keyboard(conn), │
│         }                                         │
│     });                                           │
│ }                                                  │
│                                                    │
│ [Open in GitHub ▸] [Run as Demo ▸] [Blame ▸]      │
└────────────────────────────────────────────────────
```

Встроенный просмотрщик исходников с подсветкой синтаксиса,
ссылками на GitHub и кнопкой «Run as Demo» (открывает демо
compositor'а на этом коде).

---

## 6. Фазы реализации

### Phase 1: Foundation (2-3 недели)
- [x] Rust backend с Axum (роутинг, шаблоны)
- [x] Lua engine (mlua) для контента
- [x] Базовая тема «Terminal Dashboard»
- [ ] Страница `/docs` с man-style навигацией
- [ ] Страница `/` с приветственным терминалом

### Phase 2: Content (2-3 недели)
- [ ] Миграция документации из .md в .lua-файлы
- [ ] Полнотекстовый поиск (tantivy)
- [ ] Git-based CMS (редактирование через git push)
- [ ] Blog engine (Lua-посты с датами)

### Phase 3: Interactive (3-4 недели)
- [ ] WASM сборка minix-compositor
- [ ] Canvas-backed compositor demo
- [ ] Drag-n-drop окон в браузере
- [ ] Lua REPL (mlua → WASM)
- [ ] Live build log (SSE)

### Phase 4: Advanced (2-3 недели)
- [ ] Capability matrix visualizer
- [ ] Code Explorer (GitHub integration)
- [ ] Package browser
- [ ] Security dashboard
- [ ] Audit log viewer

### Phase 5: Polish (1-2 недели)
- [ ] Responsive design (мобильная версия)
- [ ] PWA (offline support)
- [ ] i18n (русский + английский)
- [ ] Dark/light темы
- [ ] Performance оптимизация WASM

---

## 7. Стек зависимостей

### Backend (Cargo.toml)

```toml
[package]
name = "gergios-site"
version = "0.1.0"
edition = "2021"

[dependencies]
# HTTP server
axum = "0.8"
tokio = { version = "1", features = ["full"] }
tower-http = { version = "0.6", features = ["fs", "cors"] }
serde = { version = "1", features = ["derive"] }

# Templating
minijinja = "2"
comrak = "0.28"  # Markdown

# Lua scripting
mlua = { version = "0.10", features = ["vendored"] }

# Search
tantivy = "0.22"

# Git
git2 = "0.19"

# Monitoring
sysinfo = "0.33"

# WASM
wasm-bindgen = "0.2"

[target.'cfg(target_arch = "wasm32")'.dependencies]
minix-compositor = { path = "../rust/minix-compositor" }
minix-wayland = { path = "../rust/minix-wayland" }
wasm-bindgen = "0.2"
web-sys = "0.3"
js-sys = "0.3"
```

### Frontend (WASM Cargo.toml)

```toml
[package]
name = "gergios-site-wasm"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
wasm-bindgen = "0.2"
# Compositor (same code as the OS!)
minix-compositor = { path = "../../rust/minix-compositor" }
minix-wayland = { path = "../../rust/minix-wayland" }
# Canvas rendering
web-sys = { version = "0.3", features = ["CanvasRenderingContext2d", "ImageData"] }
js-sys = "0.3"
```

---

## 8. Что делает это крутым

| Фича | Почему круто |
|------|-------------|
| **Compositor в WASM** | Единственный сайт, где работает настоящий Wayland compositor |
| **Lua-контент** | Не Markdown/HAML/PUG, а тот же Lua, что и в самой ОС |
| **Live Demo** | Не скриншоты, а живое демо с drag-n-drop |
| **Build Monitor** | SSE-stream настоящего CI/CD пайплайна |
| **Терминал** | Приветственная страница — терминал (как в ОС) |
| **Rust-only** | Ни одного .js файла (кроме WASM glue) |
| **Open book** | Весь код открыт, можно запустить у себя |
| **Единый стек** | Один код для ОС и для сайта |

---

## 9. Related Documents

- `planning/11_gui_architecture.md` — GUI Architecture (compositor, Wayland)
- `planning/27_rust_bluetooth_stack.md` — Rust Bluetooth stack (ещё один WASM-компонент?)
- `rust/minix-compositor/` — Software compositor (основа для WASM демо)
- `rust/minix-wayland/` — Wayland protocol (основа для WASM демо)
- `rust/` — Все Rust крейты (демонстрация на сайте)
