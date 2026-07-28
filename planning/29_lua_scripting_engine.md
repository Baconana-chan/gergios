# Lua Scripting Engine — GergiOS Desktop Customization

> **Part of**: GergiOS Modernization Roadmap
> **Related**: `planning/11_gui_architecture.md`, `planning/28_gui_system_programs.md`, `planning/03_migration_roadmap.md`
> **Status**: Planning
> **Language**: Rust (host) + Lua 5.4 (embedded via `mlua`)

---

## 0. Философия: «Каждый пиксель — твой»

> **«GergiOS должен быть настолько кастомизируемым, чтобы любой пользователь мог изменить любой пиксель на экране, написав три строки на Lua.»**

### Почему Lua, а не JavaScript/QML/Python?

| Критерий | Lua | JavaScript/QML | Python | Свой скриптовый язык |
|----------|-----|---------------|--------|---------------------|
| **Встраиваемость** | ✅ Идеальная (C API, `mlua` для Rust) | ❌ V8/SpiderMonkey — >50MB | ❌ CPython embedding — >10MB | ❌ Надо писать с нуля |
| **Размер** | ~200KB | ~50MB (V8) | ~10MB | — |
| **Время старта** | ~5ms | ~200ms | ~300ms | — |
| **Понятность** | ✅ Естественный английский синтаксис | 🟡 QML свой, JS — ок | 🟡 Indentation-sensitive | ❌ |
| **Безопасность** | ✅ Песочница (`mlua` sandbox) | 🟡 Ограниченная | 🟡 os/syscall доступ | 🟡 |
| **Сообщество** | ✅ AwesomeWM, Neovim, World of Warcraft | ✅ KDE Plasma (QML+JS) | ✅ Много | ❌ |
| **Уже в проекте** | ✅ `cargo.lock` + `planning/11` | ❌ | ❌ | ❌ |

### Цитаты из сообщества

> «Lua — это не язык для программистов. Это язык для **пользователей**, которые хотят изменить поведение программы, не став программистами.» — Roberto Ierusalimschy (создатель Lua)

> «AwesomeWM доказывает: когда конфиг — это полноценный язык программирования, пользователи создают вещи, которые разработчики никогда бы не предусмотрели.» — Сообщество AwesomeWM

### Принципы дизайна Lua API для GergiOS

1. **Zero-to-customization в 3 строки**: Чтобы изменить цвет панели, нужно написать 3 строки, а не гуглить 20 минут.
2. **Читаемость > Производительность**: Lua-код читается как английский текст.
3. **Fail gracefully**: Любая ошибка в Lua-скрипте показывает понятное сообщение и не валит compositor.
4. **Песочница по умолчанию**: Lua-скрипты не имеют доступа к файловой системе (кроме своего `~/.config/`).
5. **Live reload**: Изменил `panel.lua` → compositor перезагружает скрипт без перезапуска.

---

## 1. Архитектура Lua VM в Compositor'е

### 1.1 Схема

```
┌──────────────────────────────────────────────────────────────┐
│                   GergiOS Compositor (Rust)                    │
│                                                               │
│  ┌────────────────────────────────────────────────────────┐   │
│  │                  Lua VM (mlua 0.10+)                    │   │
│  │                                                         │   │
│  │  ┌─────────────┐  ┌──────────────┐  ┌──────────────┐   │   │
│  │  │  gergios.*   │  │  panel.*     │  │  wm.*         │   │   │
│  │  │  Core API    │  │  Panel API   │  │  WM API       │   │   │
│  │  └─────────────┘  └──────────────┘  └──────────────┘   │   │
│  │                                                         │   │
│  │  ┌─────────────────────────────────────────────────┐    │   │
│  │  │           Загруженные скрипты                     │    │   │
│  │  │  /etc/gergios/compositor.lua  (system-wide)      │    │   │
│  │  │  ~/.config/gergios/panel.lua   (user)            │    │   │
│  │  │  ~/.config/gergios/keybinds.lua (user)            │    │   │
│  │  │  /usr/share/gergios/gadgets/*.lua (gadgets)      │    │   │
│  │  └─────────────────────────────────────────────────┘    │   │
│  └────────────────────────────────────────────────────────┘   │
│                                                               │
│  ┌─────────────┐  ┌──────────────┐  ┌───────────────────┐     │
│  │  Core Loop   │  │  Renderer    │  │  Input Handler    │     │
│  │  (calloop)   │  │  (software)  │  │  (wl_seat)        │     │
│  └─────────────┘  └──────────────┘  └───────────────────┘     │
│                                                               │
│  Lua VM callback ◄──── Keyboard / Mouse events ─────────────  │
│  Lua VM callback ◄──── Frame tick (для анимаций) ───────────  │
│  Lua VM callback ◄──── Configuration reload ────────────────  │
└──────────────────────────────────────────────────────────────┘
```

### 1.2 `mlua` Rust интеграция

```rust
// Псевдокод: архитектура Lua VM в compositor'е

use mlua::{Lua, Function, Table, Value, Result as LuaResult};

pub struct LuaEngine {
    lua: Lua,
    /// Текущий путь к конфигурации (для live reload)
    config_path: PathBuf,
    /// Хранилище callback'ов: event_name → Vec<LuaFunction>
    hooks: HashMap<String, Vec<mlua::RegistryKey>>,
}

impl LuaEngine {
    pub fn new() -> Self {
        let lua = Lua::new();

        // 1. Песочница: убрать опасные функции
        lua.globals().raw_remove("os")?;
        lua.globals().raw_remove("io")?;
        lua.globals().raw_remove("loadfile")?;
        lua.globals().raw_remove("dofile")?;
        // Оставить: math, string, table, tonumber, tostring, type, pairs, ipairs, select, unpack, error

        // 2. Зарегистрировать GergiOS API
        register_core_api(&lua)?;    // gergios.*
        register_panel_api(&lua)?;   // panel.*
        register_wm_api(&lua)?;      // wm.*
        register_gadget_api(&lua)?;  // gadget.*

        Self { lua, config_path: PathBuf::from("/etc/gergios/"), hooks: HashMap::new() }
    }

    /// Выполнить скрипт
    pub fn run(&self, path: &str) -> Result<(), LuaError> {
        let chunk = self.lua.load(std::fs::read_to_string(path)?);
        chunk.exec()?;
        Ok(())
    }

    /// Триггернуть событие (из Rust → Lua)
    pub fn trigger(&self, event: &str, ...) {
        if let Some(callbacks) = self.hooks.get(event) {
            for key in callbacks {
                let func: Function = self.lua.registry_value(key)?;
                func.call::<()>(())?;
            }
        }
    }
}
```

### 1.3 Песочница (Sandbox)

**Что ЗАБЛОКИРОВАНО** в Lua VM:
- ⛔ `os.execute()` — запуск shell-команд
- ⛔ `os.exit()` — завершение compositor'а
- ⛔ `io.*` — файловый ввод/вывод (кроме `gergios.read_config()`)
- ⛔ `loadfile()`, `dofile()`, `require()` — загрузка произвольных файлов
- ⛔ `debug.*` — мета-программирование (кроме `debug.traceback`)
- ⛔ `package.*` — загрузка C-библиотек

**Что РАЗРЕШЕНО**:
- ✅ `math.*`, `string.*`, `table.*` — полный доступ
- ✅ `tonumber()`, `tostring()`, `type()`, `pairs()`, `ipairs()`, `select()`, `unpack()`, `error()`
- ✅ `gergios.*` — GergiOS Core API
- ✅ `panel.*` — Panel API
- ✅ `wm.*` — Window Manager API
- ✅ `gadget.*` — Gadget (widget) API
- ✅ `color()`, `font()`, `theme()` — вспомогательные функции

**Безопасность через `mlua`**:
- `Lua::new()` создаёт изолированное состояние — никакой доступ к глобальным переменным других VM
- `RegistryKey` для callback'ов — Rust владеет ключами, Lua не может их подменить
- `sandbox()` функция при старте очищает глобальную таблицу от опасных функций
- Resource limit: `lua.set_memory_limit(10 * 1024 * 1024)` — 10MB максимум на скрипты
- Instruction limit: `lua.set_instruction_limit(10_000_000)` — защита от бесконечных циклов

---

## 2. Сравнение с KDE Plasma

| KDE Plasma (QML+JS) | GergiOS (Lua) | Комментарий |
|---------------------|---------------|-------------|
| **QML** — декларативный UI | **Lua tables** — декларация UI через таблицы | В Lua та же роль: `Panel { height = 32 }` |
| **JavaScript** — логика | **Lua** — логика | Lua проще, нет async/await/promises |
| **KConfig** — иерархия конфигов | **Lua script as config** — конфиг = код | Как AwesomeWM: весь конфиг — это Lua |
| **KPlugin** — .desktop метаданные | **JSON metadata** + .lua файл | Проще: `clock.lua` + `clock.json` |
| **DBus** — IPC для виджетов | **MINIX IPC** — через Rust FFI | Прозрачно для Lua-скриптов |
| **QtQuick** — рендеринг UI | **Compositor renderer** — через Rust | Lua говорит «что», Rust делает «как» |
| **Plasmoids** — виджеты | **Gadgets** — виджеты | Аналогичная концепция |
| **KPackage** — установка виджетов | `gergios install <gadget>` | CLI + возможный GUI store |
| **System Settings** — GUI настройки | `gergios-settings` + Lua API | Настройки — это Lua-скрипты |

### 2.1 KConfig → GergiOS Config

KConfig features:
- Cascading: `/etc/kde/` → `~/.config/`
- Key groups: `[General]`, `[Appearance]`
- Kiosk mode: lock certain keys

GergiOS equivalent:
```lua
-- ~/.config/gergios/compositor.lua
-- Cascading: system values from /etc/gergios/compositor.lua
-- are loaded first, then user values override.

-- Kiosk mode: /etc/gergios/kiosk.lua
-- gergios.lock("panel.position")  -- user cannot change this

theme {
    name = "dark",
    font_size = 12,
    accent_color = color("#3B82F6"),
}

Panel {
    position = "top",     -- can be locked by kiosk
    height = 32,
}

-- Если kiosk блокирует panel.position, то
-- panel.position будет иметь системное значение,
-- а пользовательское будет проигнорировано.
```

### 2.2 KPlugin → Gergios Gadgets

```json
// /usr/share/gergios/gadgets/clock/clock.json
{
    "name": "Clock",
    "version": "1.0",
    "description": "Simple digital clock widget",
    "entry": "clock.lua",
    "author": "GergiOS Team",
    "license": "BSD-2",
    "api_version": 1,
    "category": "utilities",
    "permissions": ["gergios.timer"],  // что нужно виджету
    "sizes": ["small", "medium", "large"]
}
```

```lua
-- /usr/share/gergios/gadgets/clock/clock.lua
-- Digital clock widget — ~15 строк

local clock = gadget.new("clock")

function clock:init()
    self.text = font.new("mono", 14)
end

function clock:render(ctx)
    local time = os.date("%H:%M:%S")  -- os.date разрешён (только чтение)
    local w = self.text:width(time)

    -- Центрируем текст в области виджета
    local x = (ctx.width - w) / 2
    local y = (ctx.height - 14) / 2

    ctx:draw_text(self.text, time, x, y, color("#FFFFFF"))
end

-- Обновление каждую секунду (через gergios.timer)
gergios.every("1s", function()
    clock:redraw()
end)

-- При клике — показать календарь
gergios.on("click", function()
    gergios.notify(os.date("%A, %B %d %Y"))
end)
```

---

## 3. API Design: Rust → Lua Surface

### 3.1 Core API (`gergios.*`)

```lua
-- Информация о системе
gergios.version()                        → "0.5.0"
gergios.hostname()                       → "gergios-pc"
gergios.screen_width()                   → 1920
gergios.screen_height()                  → 1080
gergios.uptime()                         → 3600 (секунды)
gergios.battery_level()                  → 85 (проценты, или nil)

-- Уведомления
gergios.notify(text)                     — показать уведомление
gergios.notify(text, urgency)            — urgency: "low", "normal", "critical"

-- Система
gergios.launch("terminal")               — запустить программу
gergios.launch("gergios-calc")           — запустить калькулятор
gergios.execute("command")               — выполнить shell-команду (❗ песочница: только разрешённые)

-- Таймеры
gergios.every("1s", callback)            — периодический вызов
gergios.after("5s", callback)            — отложенный вызов
gergios.cancel(timer_id)                 — отменить таймер

-- Цвета
color("#FF0000")                         → Color { r=255, g=0, b=0, a=255 }
color(255, 0, 0)                         → Color { r=255, g=0, b=0, a=255 }
color("red")                             → Color — именованные цвета (CSS named colors)
color:alpha(128)                         → Color с прозрачностью
color:lighten(0.2)                       → осветлить на 20%
color:darken(0.2)                        → затемнить на 20%

-- Темы
gergios.theme_get("colors.background")    → Color
gergios.theme_get("fonts.ui")            → Font
gergios.theme_set("colors.background", color("#111111"))

-- Конфиг (песочница: только /etc/gergios/ и ~/.config/gergios/)
gergios.read_config("panel.lua")         → таблица с конфигом
gergios.write_config("panel.lua", tbl)   — записать конфиг

-- Хуки (события)
gergios.on("compositor:start", fn)       — при старте композитора
gergios.on("compositor:frame", fn)       — каждый кадр
gergios.on("keybind:press", fn)          — нажатие клавиши
gergios.on("mouse:move", fn)             — движение мыши
gergios.on("mouse:click", fn)            — клик мыши
gergios.on("window:open", fn)            — открытие окна
gergios.on("window:close", fn)           — закрытие окна
gergios.on("window:focus", fn)           — фокус окна
```

### 3.2 Panel API (`panel.*`)

```lua
-- Конфигурация панели
Panel {
    position = "top",                    -- "top", "bottom", "left", "right"
    height = 32,                         -- или width для left/right
    background = color("#1a1a2e"),
    opacity = 0.95,
    margin = 4,
    border_radius = 8,
}

-- Виджеты на панели
panel:add_widget("clock", { format = "%H:%M" })
panel:add_widget("battery", { })
panel:add_widget("network", { interface = "wlan0" })
panel:add_widget("tray", { })            -- системный трей
panel:add_widget("launcher", { })        -- меню приложений

-- Кастомный виджет
panel:add_widget(gadget.load("cpu_monitor"), {
    position = "right",
    width = 100,
})
```

### 3.3 Window Manager API (`wm.*`)

```lua
-- Layout
wm.layout("tile")                         -- "tile", "float", "monocle", "grid"
wm.master_count(2)                        -- количество мастер-окон
wm.master_factor(0.6)                     -- пропорция мастер-области

-- Keybindings (синтаксис как в AwesomeWM)
keybind({ "Super", "Return" }, function() gergios.launch("terminal") end)
keybind({ "Super", "d" },     function() gergios.launch("app-launcher") end)
keybind({ "Super", "q" },     function() wm.focused:close() end)
keybind({ "Super", "Tab" },   function() wm.next_window() end)
keybind({ "Super", "h" },     function() wm.focus_direction("left") end)
keybind({ "Super", "l" },     function() wm.focus_direction("right") end)
keybind({ "Super", "j" },     function() wm.focus_direction("down") end)
keybind({ "Super", "k" },     function() wm.focus_direction("up") end)

-- Floating windows
keybind({ "Super", "Shift", "space" }, function()
    wm.focused:toggle_floating()
end)

-- Workspaces
keybind({ "Super", "1" }, function() wm.goto_tag(1) end)
keybind({ "Super", "2" }, function() wm.goto_tag(2) end)
keybind({ "Super", "Shift", "1" }, function() wm.focused:move_to_tag(1) end)

-- Mouse bindings
mousebind({ "Super", "Button1" }, function(ctx)
    wm.focused:mouse_move(ctx)
end)
mousebind({ "Super", "Button3" }, function(ctx)
    wm.focused:mouse_resize(ctx)
end)

-- Rules (автоматические правила для окон)
wm.rule({
    match = { class = "Firefox" },
    tag = 2,
    floating = false,
})

wm.rule({
    match = { title = "Save As" },
    floating = true,
    width = 600,
    height = 400,
})

-- Tags (workspaces)
tag { name = "1: dev",     layout = "tile" }
tag { name = "2: web",     layout = "tile" }
tag { name = "3: chat",    layout = "tile" }
tag { name = "4: media",   layout = "float" }
tag { name = "5: system",  layout = "monocle" }
```

### 3.4 Rendering API (`render.*`)

```lua
-- Прямой доступ к рендерингу
render.set_pixel(x, y, color)
render.fill_rect(x, y, w, h, color)
render.fill_rounded_rect(x, y, w, h, radius, color)
render.fill_circle(x, y, radius, color)
render.fill_triangle(x1, y1, x2, y2, x3, y3, color)
render.draw_line(x1, y1, x2, y2, color)
render.draw_text(font, text, x, y, color)
render.draw_text_rect(font, text, rect, alignment, color)
render.text_width(font, text)                → number
render.text_height(font, text, max_width)    → number

-- Градиенты
render.gradient_linear(x, y, w, h, stops)   -- stops: {{offset, color}, ...}
render.gradient_radial(cx, cy, radius, stops)

-- Композитинг
render.with_alpha(alpha, function()
    render.fill_rect(0, 0, 100, 100, color("red"))
end)  -- все операции внутри блока с прозрачностью

render.with_clip(x, y, w, h, function()
    -- всё, что внутри — clipped
end)

render.capture(x, y, w, h)                 → Image
render.image(img, x, y)                     — отрисовать Image
render.image_scaled(img, x, y, w, h)        — масштабированное изображение

-- Анимации
render.animate {
    duration = "500ms",
    easing = "ease-out",                    -- "linear", "ease-in", "ease-out", "bounce"
    from = { x = 0, y = 0 },
    to = { x = 100, y = 100 },
    on_update = function(state)
        render.fill_rect(state.x, state.y, 50, 50, color("blue"))
    end,
}
```

### 3.5 Font API (`font.*`)

```lua
font.new("sans", 14)                       → Font
font.new("mono", 12)                       → Font
font.new("/path/to/custom.ttf", 16)        → Font (custom font)
font.default()                             → Font (system default)
font.default_mono()                        → Font (monospace default)

font:width("Hello")                        → number (pixels)
font:height()                              → number (line height)
font:ascent()                              → number
font:descent()                             → number
font:set_size(16)                          — изменить размер
```

### 3.6 Theme API (`theme.*`)

```lua
-- ~/.config/gergios/theme.lua
-- Полная тема оформления — это Lua-скрипт

theme {
    name = "GergiOS Dark",

    -- Colors
    colors = {
        background  = color("#0f0f1a"),
        surface     = color("#1a1a2e"),
        surface2    = color("#16213e"),
        primary     = color("#3B82F6"),
        secondary   = color("#8B5CF6"),
        accent      = color("#06D6A0"),
        text        = color("#E2E8F0"),
        text_muted  = color("#94A3B8"),
        border      = color("#2D3748"),
        error       = color("#EF4444"),
        warning     = color("#F59E0B"),
        success     = color("#10B981"),
    },

    -- Typography
    fonts = {
        ui = font.new("sans", 13),
        mono = font.new("mono", 12),
        heading = font.new("sans", 18),
        small = font.new("sans", 11),
        clock = font.new("mono", 14),
    },

    -- Spacing
    spacing = {
        xs = 4,
        sm = 8,
        md = 12,
        lg = 16,
        xl = 24,
    },

    -- Panel
    panel = {
        height = 32,
        background = nil,  -- uses colors.surface
        opacity = 0.92,
        border_radius = 8,
        margin = 4,
        shadow = true,
        shadow_color = color("#000000", 0.3),
    },

    -- Window decorations
    window = {
        titlebar_height = 28,
        titlebar_background = color("#1a1a2e"),
        border_width = 1,
        border_color = color("#2D3748"),
        border_radius = 6,
        shadow = true,
        shadow_size = 8,
        shadow_color = color("#000000", 0.4),
        button_colors = {
            close  = color("#EF4444"),
            minimize = color("#F59E0B"),
            maximize = color("#10B981"),
        },
    },

    -- Animations
    animations = {
        enabled = true,
        duration_open = "200ms",
        duration_close = "150ms",
        duration_hover = "100ms",
        easing = "ease-out",
    },
}
```

---

## 4. Фазы внедрения

### Phase A: Lua VM in Compositor (P2 — ~800 LOC)

**Цель**: Встроить Lua VM в compositor, загружать конфиг на Lua, base API.

| Задача | LOC | Статус |
|--------|-----|--------|
| A1. `mlua` dependency в Cargo.toml compositor'а | ~5 | 🔴 |
| A2. `LuaEngine` struct — создание, инициализация, песочница | ~150 | 🔴 |
| A3. Регистрация `gergios.*` Core API (info, notify, launch) | ~200 | 🔴 |
| A4. Загрузка `/etc/gergios/compositor.lua` и `~/.config/gergios/` | ~100 | 🔴 |
| A5. Загрузка `/etc/gergios/keybinds.lua` | ~100 | 🔴 |
| A6. Live reload (SIGHUP или `gergios reload`) | ~80 | 🔴 |
| A7. Error handling + debug output (что пошло не так в Lua) | ~100 | 🔴 |
| A8. Unit tests: Lua API, песочница, live reload | ~200 | 🔴 |

**Результат**:
```lua
-- ~/.config/gergios/compositor.lua
panel_height = 32
keybind({"Super", "Return"}, function()
    gergios.launch("terminal")
end)
```

### Phase B: Panel + Widget System (P2 — ~1200 LOC)

**Цель**: Панель с Lua-скриптуемыми виджетами.

| Задача | LOC | Статус |
|--------|-----|--------|
| B1. Panel API (`panel.*`) — создание, конфигурация | ~150 | 🔴 |
| B2. Widget registry — загрузка .lua + .json гаджетов из `gadgets/` | ~200 | 🔴 |
| B3. Gadget lifecycle (`init`, `render`, `click`, `resize`) | ~200 | 🔴 |
| B4. `render.*` API — доступ к рендерингу из Lua | ~300 | 🔴 |
| B5. `font.*` API — шрифты | ~100 | 🔴 |
| B6. Встроенные виджеты: clock, battery, network, tray, launcher | ~300 | 🔴 |
| B7. Unit tests: widget lifecycle, render API, timer | ~200 | 🔴 |

**Результат**:
```lua
Panel {
    position = "top",
    widgets = {
        clock { format = "%H:%M" },
        battery { },
    }
}
```

### Phase C: Window Manager Integration (P3 — ~1500 LOC)

**Цель**: Полный WM API + keybinds + rules + tags.

| Задача | LOC | Статус |
|--------|-----|--------|
| C1. `wm.*` API — layout, focus, tags | ~300 | 🔴 |
| C2. `keybind()` + `mousebind()` — система биндингов | ~200 | 🔴 |
| C3. `wm.rule{}` — автоматические правила для окон | ~150 | 🔴 |
| C4. `tag{}` — workspace management | ~100 | 🔴 |
| C5. `gergios.on()` — система событий (window:open, focus, click) | ~200 | 🔴 |
| C6. Signal-слот система для Lua callback'ов | ~150 | 🔴 |
| C7. `gergios.every()` / `gergios.after()` — таймеры | ~100 | 🔴 |
| C8. Unit tests: WM API, keybinds, rules, events | ~300 | 🔴 |

**Результат**: Полный AwesomeWM-стиль конфиг.
```lua
-- Итоговый конфиг: GergiOS в стиле AwesomeWM
-- Пользователь может полностью переопределить поведение WM

theme { ... }

Panel { ... }

tag { name = "dev",  layout = "tile" }
tag { name = "web",  layout = "tile" }
tag { name = "chat", layout = "float" }
tag { name = "media", layout = "monocle" }

keybind({ "Super", "Return" }, function() gergios.launch("terminal") end)
keybind({ "Super", "d" }, function() gergios.launch("app-launcher") end)
keybind({ "Super", "q" }, function() wm.focused:close() end)
keybind({ "Super", "Tab" }, function() wm.next_window() end)

mousebind({ "Super", "Button1" }, function(ctx) wm.focused:mouse_move(ctx) end)

wm.rule({
    match = { class = "Firefox" },
    tag = 2,
    floating = false,
})

gergios.on("window:open", function(win)
    if win.title:match("Save As") then
        win.floating = true
        win:resize(600, 400)
    end
end)
```

### Phase D: Theme System + Gadgets (P3 — ~1000 LOC)

**Цель**: Полная темизация и магазин гаджетов.

| Задача | LOC | Статус |
|--------|-----|--------|
| D1. `theme.*` API — colors, fonts, spacing, cascading | ~200 | 🔴 |
| D2. Загрузка `theme.lua` из system + user + per-app | ~150 | 🔴 |
| D3. `color()` API — CSS named colors, HEX, RGBA, lighten/darken | ~150 | 🔴 |
| D4. Material Design Icons интеграция | ~100 | 🔴 |
| D5. Cursor themes (через Lua-конфиг) | ~50 | 🔴 |
| D6. `gergios write_config` — сохранение изменений | ~100 | 🔴 |
| D7. Demo: 10 готовых гаджетов (clock, cpu, ram, network, weather, music, calendar, battery, uptime, launcher) | ~300 | 🔴 |
| D8. Integration tests: theme cascading, gadgets, write_config | ~200 | 🔴 |

### Phase E: Developer Experience + Ecosystem (P4 — ~1500 LOC)

**Цель**: Инструменты для разработки Lua-скриптов и сообщество.

| Задача | LOC | Статус |
|--------|-----|--------|
| E1. Lua Language Server definitions (`.lua` type annotations) | ~500 | 🔴 |
| E2. `gergios-gadget` CLI — init, test, package gadget | ~300 | 🔴 |
| E3. `gergios install <gadget>` — установка из репозитория | ~200 | 🔴 |
| E4. `gergios list` — список установленных гаджетов | ~100 | 🔴 |
| E5. `gergios edit` — открыть конфиг в редакторе | ~50 | 🔴 |
| E6. GUI Gadget Manager (часть gergios-settings) | ~300 | 🔴 |
| E7. Документация: API Reference + Examples + Tutorial | ~500 | 📝 |
| E8. Unit tests: CLI, LSP definitions, install | ~400 | 🔴 |

---

## 5. Примеры: от простого к сложному

### Пример 1: «Я хочу изменить цвет фона» (3 строки)

```lua
-- ~/.config/gergios/theme.lua
theme {
    colors = {
        background = color("#0a0a1a"),  -- мой любимый тёмно-синий
    }
}
```

### Пример 2: «Я хочу, чтобы панель была снизу» (5 строк)

```lua
-- ~/.config/gergios/panel.lua
Panel {
    position = "bottom",
    height = 36,
    background = color("#1a1a2e"),
    opacity = 0.9,
}
```

### Пример 3: «Я хочу горячую клавишу для скриншота» (6 строк)

```lua
-- ~/.config/gergios/keybinds.lua
keybind({ "Super", "Shift", "s" }, function()
    local path = "/tmp/screenshot.png"
    gergios.execute("gergios-screenshot " .. path)
    gergios.notify("Screenshot saved to " .. path)
end)
```

### Пример 4: «Я хочу виджет погоды» (~30 строк)

```lua
-- ~/.config/gergios/gadgets/weather.lua
local weather = gadget.new("weather")

local api_key = "..."  -- в реальности: gergios.read_config("secrets.lua")

weather.city = "Moscow"

function weather:init()
    self.font = font.new("sans", 12)
    self.refresh()
end

function weather:refresh()
    gergios.http_get("https://api.weather.com/v1/" .. self.city, function(data)
        self.temp = data.current.temp
        self.icon = data.current.icon
        self:redraw()
    end)
end

function weather:render(ctx)
    -- Иконка
    render.image(self.icon, 4, 2, 16, 16)
    -- Температура
    render.draw_text(self.font, self.temp .. "°C", 24, 4, theme.colors.text)
end

-- Обновление каждые 30 минут
gergios.every("30min", function()
    weather:refresh()
end)
```

### Пример 5: «Я хочу анимированную заставку» (~40 строк)

```lua
-- ~/.config/gergios/screensaver.lua
gergios.on("idle:5min", function()
    local particles = {}
    for i = 1, 50 do
        particles[i] = {
            x = math.random(0, gergios.screen_width()),
            y = math.random(0, gergios.screen_height()),
            vx = math.random(-2, 2),
            vy = math.random(-2, 2),
            size = math.random(2, 6),
            color = color(math.random(100, 255), math.random(100, 255), math.random(100, 255)),
        }
    end

    gergios.on("compositor:frame", function()
        render.with_alpha(0.05, function()
            render.fill_rect(0, 0, gergios.screen_width(), gergios.screen_height(), color("#000000"))
        end)

        for _, p in ipairs(particles) do
            p.x = p.x + p.vx
            p.y = p.y + p.vy
            if p.x < 0 or p.x > gergios.screen_width() then p.vx = -p.vx end
            if p.y < 0 or p.y > gergios.screen_height() then p.vy = -p.vy end
            render.fill_circle(p.x, p.y, p.size, p.color)
        end
    end)
end)
```

---

## 6. Инструментарий для пользователей

### 6.1 `gergios` CLI

```
gergios                      # Открыть GUI: Settings → Gadgets
gergios reload               # Перезагрузить конфиги
gergios edit                 # Открыть ~/.config/gergios/ в редакторе
gergios edit compositor      # Открыть compositor.lua
gergios edit keybinds        # Открыть keybinds.lua
gergios edit theme           # Открыть theme.lua
gergios list                 # Список установленных гаджетов
gergios install clock        # Установить clock gadget
gergios remove clock         # Удалить clock gadget
gergios init                 # Создать ~/.config/gergios/ с дефолтами
gergios doctor               # Проверить конфиги на ошибки
gergios logs                 # Показать ошибки Lua-скриптов
gergios template widget      # Создать шаблон виджета
gergios format config.lua    # Форматировать Lua-файл
```

### 6.2 GUI Settings → Gadgets

```
┌─────────────────────────────────────────────────┐
│  Settings  │  Appearance  │  Gadgets  │  About   │
├─────────────────────────────────────────────────┤
│                                                 │
│  ┌─ Installed Gadgets ─────────────────────┐   │
│  │ ☑ Clock              ⚙️  ✕              │   │
│  │ ☑ Battery            ⚙️  ✕              │   │
│  │ ☐ Weather            ⚙️  ✕              │   │
│  │ ☐ CPU Monitor        ⚙️  ✕              │   │
│  └──────────────────────────────────────────┘   │
│                                                 │
│  [Browse Gadgets...]   [Open Config Folder]     │
│                                                 │
│  ┌─ Preview ─────────────────────────────────┐ │
│  │                                            │ │
│  │         ☀️  25°C  🌤️  Moscow              │ │
│  │                                            │ │
│  └────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────┘
```

### 6.3 Live Preview (WYSIWYG)

**Идея**: Редактор `gergios edit compositor.lua` открывает файл в текстовом редакторе.
Compositor отслеживает изменения через `inotify` (MINIX аналог) и перезагружает конфиг:

```
$ gergios edit compositor
→ Открывает compositor.lua в редакторе
→ При каждом сохранении: compositor перезагружает скрипт
→ Изменения видны сразу (изменил фон → фон сменился)
→ Если в скрипте ошибка: compositor показывает уведомление
  "Error in compositor.lua line 42: expected 'end'"
```

### 6.4 `gergios doctor` — диагностика конфигов

```
$ gergios doctor
Checking configuration...
  ✅ /etc/gergios/compositor.lua — OK
  ✅ ~/.config/gergios/compositor.lua — OK (overrides 3 keys)
  ❌ ~/.config/gergios/panel.lua — ERROR line 15: unexpected symbol near '}'
  ✅ /usr/share/gergios/gadgets/clock/ — OK
  ❌ /usr/share/gergios/gadgets/weather/ — WARNING: missing API key

⛔ 2 issues found. Fix them with `gergios edit panel`
```

---

## 7. Безопасность

### 7.1 Модель угроз

| Угроза | Риск | Mitigation |
|--------|------|------------|
| Lua-скрипт читает `/etc/shadow` | Высокий | Песочница: `io.*`, `os.execute()`, `loadfile()` заблокированы |
| Lua-скрипт делает fork bomb | Средний | `instruction_limit = 10_000_000` |
| Lua-скрипт потребляет всю память | Средний | `memory_limit = 10MB` |
| Lua-скрипт вешает compositor бесконечным циклом | Средний | Instruction limit + watchdog timer |
| Зловредный гаджет из интернета | Высокий | Подпись гаджетов (начиная с Phase E) |
| Lua-скрипт читает чужие конфиги | Низкий | Песочница: только `/etc/gergios/` и `~/.config/gergios/` |

### 7.2 Права доступа гаджетов

Каждый гаджет декларирует нужные права в `metadata.json`:

```json
{
    "name": "weather",
    "permissions": [
        "gergios.http_get",      // доступ к HTTP
        "gergios.timer",         // доступ к таймерам
        "gergios.geo_location"   // доступ к геолокации (требует подтверждения)
    ]
}
```

При установке пользователь видит:

```
$ gergios install weather
weather@1.0 requests:
  ✅ gergios.http_get — make HTTP requests
  ✅ gergios.timer — schedule periodic tasks
  ❓ gergios.geo_location — access your location
     [Allow] [Deny] [Ask Later]
```

### 7.3 Resource Limits

```rust
// В Rust коде:
lua.set_memory_limit(10 * 1024 * 1024);       // 10 MB
lua.set_instruction_limit(10_000_000);         // 10M instructions
lua.set_hook(EventHook::new()
    .every_n_instructions(1_000_000, || {
        // Проверка: не завис ли скрипт?
    })
);
```

---

## 8. Инструменты разработчика (DX)

### 8.1 Lua Language Server Definitions

Для автодополнения в VS Code / Neovim:

```lua
-- .lua-ls/definitions/gergios.lua
-- Эти определения используются lua-language-server для автодополнения

---@class GergiosColor
---@field r integer
---@field g integer
---@field b integer
---@field a integer

---@param red integer|string
---@param green? integer
---@param blue? integer
---@param alpha? integer
---@return GergiosColor
function color(red, green, blue, alpha) end

---@class PanelConfig
---@field position "top"|"bottom"|"left"|"right"
---@field height integer
---@field background GergiosColor
---@field opacity number

---@param config PanelConfig
function Panel(config) end

---@class KeybindSpec
---@field mod string
---@field key string
---@field action string
```

### 8.2 Документация

```lua
-- https://gergios.dev/docs/lua/compositor
-- Каждая функция документирована с примерами:

--- Запустить программу.
--- @param program string Имя программы (ищется в PATH) или полный путь
--- @param args? table Аргументы командной строки (опционально)
--- @return boolean true если запуск успешен
--- @example
--- gergios.launch("terminal")
--- gergios.launch("gergios-calc", { "--mode=scientific" })
function gergios.launch(program, args) end
```

### 8.3 `gergios-gadget` CLI

```bash
# Создать новый гаджет
$ gergios-gadget init mywidget
Created mywidget/
├── mywidget.lua
├── mywidget.json
└── preview.png

# Запустить в тестовом режиме
$ gergios-gadget run mywidget
→ Открывает окно с preview гаджета

# Упаковать для публикации
$ gergios-gadget package mywidget
Created mywidget.gadget
```

---

## 9. Оценка времени и зависимостей

| Фаза | Описание | LOC | Зависит от | Время |
|------|----------|-----|------------|-------|
| **A** | Lua VM in Compositor | ~800 | Compositor (Phase 1-3 ✅) | 2-3 недели |
| **B** | Panel + Widgets | ~1200 | Фаза A | 3-4 недели |
| **C** | WM Integration | ~1500 | Фаза A, Window Manager | 4-6 недель |
| **D** | Theme System | ~1000 | Фаза B | 2-3 недели |
| **E** | Developer Tools | ~1500 | Фаза B, C, D | 4-6 недель |
| **Итого** | | **~6000 LOC** | | **15-22 недели** |

### Критический путь

```
Compositor Phase 1-3 (✅)
    └──→ Фаза A (Lua VM) ──→ Фаза B (Panel + Widgets)
                                       │
                                       └──→ Фаза D (Theme System)
                                             
Window Manager (Phase 4 🔴)
    └──→ Фаза C (WM Integration) ──→ Фаза E (Developer Tools)
```

Фазы A и C могут идти **параллельно** (разные модули).

---

## 10. Success Criteria

### Phase A
- [ ] Compositor загружает `/etc/gergios/compositor.lua` и выполняет его
- [ ] Песочница блокирует `os.execute()` и `io.open()`
- [ ] `gergios.launch("terminal")` запускает терминал
- [ ] `gergios.reload` перезагружает конфиги без перезапуска compositor'а
- [ ] Ошибки в Lua-скрипте показывают понятное сообщение (file + line)

### Phase B
- [ ] Панель создаётся через `Panel { position = "top" }`
- [ ] Виджеты загружаются из `/usr/share/gergios/gadgets/*.lua`
- [ ] `render.fill_rect()` рисует прямоугольник из Lua
- [ ] Clock widget показывает текущее время, обновляется каждую секунду

### Phase C
- [ ] `keybind({ "Super", "Return" }, fn)` работает
- [ ] `wm.layout("tile")` переключает layout
- [ ] `wm.rule({ match = { class = "Firefox" }, tag = 2 })` работает
- [ ] `gergios.on("window:open", fn)` срабатывает при открытии окна

### Phase D
- [ ] `theme { colors = { background = color("#000") } }` меняет фон
- [ ] `color("red").r == 255` (CSS named colors)
- [ ] Material Design Icons отображаются в гаджетах
- [ ] 10 готовых гаджетов в `/usr/share/gergios/gadgets/`

### Phase E
- [ ] `gergios doctor` находит ошибки в конфигах
- [ ] `gergios-gadget init` создаёт шаблон гаджета
- [ ] Lua LSP definitions работают в VS Code
- [ ] `gergios install clock` устанавливает гаджет из репозитория

---

## 11. Репозиторий гаджетов (Gadget Hub)

**Идея**: `gergios.dev/gadgets` — сайт с гаджетами от сообщества.

```
gergios.dev/gadgets/
├── clock/              # Часы (официальный)
├── battery/            # Батарея (официальный)
├── weather/            # Погода (официальный)
├── cpu-monitor/        # CPU монитор (официальный)
├── network/            # Сеть (официальный)
├── spotify/            # Spotify Now Playing (community)
├── pomodoro/           # Pomodoro timer (community)
├── system-tray/        # Системный трей (официальный)
└── ...

Каждый гаджет:
├── clock.lua           # Код виджета
├── clock.json          # Метаданные
├── preview.png         # Превью (200×100)
├── screenshot.png      # Скриншот
└── README.md           # Документация
```

Установка:
```bash
gergios install weather               # с gergios.dev
gergios install ~/mywidget.gadget     # локальный файл
gergios install https://example.com/cool-gadget.gadget  # URL
```

---

## 12. Сравнение: AwesomeWM vs GergiOS Lua

| Возможность | AwesomeWM (Lua) | GergiOS (Lua) |
|-------------|-----------------|---------------|
| **Конфиг как код** | ✅ `rc.lua` | ✅ `compositor.lua` |
| **Keybindings** | ✅ `awful.key()` | ✅ `keybind()` |
| **Rules** | ✅ `awful.rules` | ✅ `wm.rule{}` |
| **Tags/Workspaces** | ✅ `awful.tag` | ✅ `tag{}` |
| **Widgets** | ✅ `wibox` | ✅ Gadgets |
| **Темы** | ✅ `beautiful` | ✅ `theme{}` |
| **Сигналы** | ✅ `connect_signal` | ✅ `gergios.on()` |
| **Layouts** | ✅ `awful.layout` | ✅ `wm.layout()` |
| **Песочница** | ❌ нет | ✅ встроенная |
| **Live reload** | ❌ нужен рестарт | ✅ `gergios reload` |
| **LSP definitions** | ❌ вручную | ✅ генерируются |
| **Установка виджетов** | ❌ ручное копирование | ✅ `gergios install` |
| **GUI Settings** | ❌ нет | ✅ через `gergios-settings` |
| **Rust API** | ❌ C API | ✅ `mlua` (Rust-native) |

**Главное отличие**: AwesomeWM — это WM для «экспертов Linux». GergiOS — это Lua для **всех**.

---

## 13. Риски и mitigation

| Риск | Impact | Mitigation |
|------|--------|------------|
| **Lua-скрипты замедляют compositor** (панель с 20 виджетами) | Средний | `render.*` операции батчатся, Lua вызывается только на изменение, не на каждый кадр |
| **Сложность API** (пользователи не поймут) | Высокий | Примеры из коробки, `gergios init` с дефолтами, `gergios doctor`, WYSIWYG редактор |
| **Несовместимость с AwesomeWM скриптами** | Низкий | Намеренно: GergiOS API проще и безопаснее. Можно сделать shim-слой, но не приоритет |
| **Регрессия производительности** | Средний | Все Lua-вызовы асинхронные (через calloop), не блокируют compositor. Instruction limits. |
| **Уязвимость через гаджеты** | Высокий | Песочница, permissions, подпись пакетов (Phase E) |
| **Фрагментация конфигов** (у каждого свой стиль) | Низкий | `gergios format`, дефолтные шаблоны, документация |

---

## 14. Related Documents

- `planning/11_gui_architecture.md` — GUI стек (Phase 1-6), §3.8 Lua, §4.4-4.6 WM/Panel
- `planning/28_gui_system_programs.md` — GUI приложения, Panel, Settings
- `planning/17_remaining_tasks.md` — Сводка оставшихся задач (GUI/J4-J5)
- `planning/03_migration_roadmap.md` — Общий план миграции
- `TODO.md` §3.2–3.4 — Display, Input, Window Management
