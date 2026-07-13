# GUI System Programs — Post-GUI Application Roadmap

> **Part of**: GergiOS Modernization Roadmap
> **Related**: `planning/11_gui_architecture.md` (GUI stack), `planning/03_migration_roadmap.md`
> **Status**: Planning
> **Dependencies**: Wayland compositor, GUI toolkit, Rust toolchain

---

## Context

После стабилизации GUI (Phases 1–6 из `planning/11_gui_architecture.md`) появляется возможность создавать графические приложения.
Данный документ описывает обязательные (core) и бонусные (bonus) программы, которые необходимо реализовать.

**Уже портировано в Rust** (~70+ CLI утилит): `cat`, `ls`, `cp`, `mv`, `rm`, `mkdir`, `grep`, `sort`, `head`,
`tail`, `echo`, `env`, `date`, `df`, `du`, `ps`-like, `find`-like, `pwd`, `kill`, и т.д.
Они работают в терминале и не требуют GUI.

---

## 1. Core GUI Applications (Обязательные)

Эти приложения составляют минимальный графический рабочий стол — аналог CDE/GNOME/KDE minimal.

### 1.1 Terminal Emulator

| Свойство | Значение |
|----------|----------|
| **Крейт** | `gergios-term` (существует `minix-term` как библиотека) |
| **Зависимости** | `minix-compositor` или Wayland → toolkit |
| **Статус** | 🟡 `minix-term` — библиотека ввода/терминала (key, mouse, gamepad). Нужен GUI-фронтенд |
| **Что делать** | Связать `minix-term` с композитором. Поддержка: 256 цветов, scrollback, copy-paste, вкладки |

**MVP**: Окно 80×24, shell prompt, прокрутка.
**Post-MVP**: Вкладки, разделение экрана (tmux-like), профили, темы.

### 1.2 File Manager

| Свойство | Значение |
|----------|----------|
| **Крейт** | `gergios-fm` |
| **Зависимости** | GUI toolkit + `minix-rs` (stat, readdir), `ext4-core` (для ext4-specific) |
| **Статус** | 🔴 С нуля |
| **Что делать** | Древовидный просмотр, copy/move/delete, drag-n-drop, preview |

**MVP**: Две панели (MC-style) или одна панель с деревом (Nautilus-style).
Навигация, копирование/перемещение/удаление файлов. Поддержка `ext4` (через `ext4-core`).

**Post-MVP**: Табы, закладки, встроенный preview (текст, изображения), SSH (через `minix-net`),
сетевое окружение, корзина.

### 1.3 Text Editor

| Свойство | Значение |
|----------|----------|
| **Крейт** | `gergios-edit` |
| **Зависимости** | GUI toolkit, syntax highlighting (например, `syntect`) |
| **Статус** | 🔴 С нуля |
| **Что делать** | Редактор с подсветкой синтаксиса, поддержкой UTF-8, поиском |

**MVP**: Открытие/сохранение файлов, редактирование, курсор, scroll, поиск (Ctrl+F).
Поддержка UTF-8 и моноширинного рендеринга (через `minix-compositor` → `rustybuzz`).

**Post-MVP**: Подсветка синтаксиса (через `syntect`), множественные буферы (tabs),
undo/redo, регулярные выражения, git integration.

### 1.4 Settings / Control Panel

| Свойство | Значение |
|----------|----------|
| **Крейт** | `gergios-settings` |
| **Зависимости** | GUI toolkit, `minix-rs` (sysctl), `minix-admin` (system control) |
| **Статус** | 🔴 С нуля |
| **Что делать** | GUI для системных настроек |

**MVP**: Панель с базовыми настройками:
- Сеть (Wi-Fi/Ethernet — через `minix-net`)
- Пользователи (через `pwhash`)
- Дата/время
- Экран (разрешение, тема)
- Звук (через `minix-hda`)

**Post-MVP**: Bluetooth (через `minix-bt-stack`), firewall (через `packet-filter`),
автообновления, принтеры.

### 1.5 Launcher / Application Menu

| Свойство | Значение |
|----------|----------|
| **Крейт** | В составе `gergios-panel` |
| **Зависимости** | GUI toolkit |
| **Статус** | 🔴 С нуля |
| **Что делать** | Меню приложений + поиск |

**MVP**: Список установленных приложений, поиск по имени, запуск по клику.
**Post-MVP**: Категории, избранное, поиск по файлам (через `locate`-like index).

---

## 2. Desktop Infrastructure (Обязательная инфраструктура)

### 2.1 Desktop Panel / Status Bar

| Свойство | Значение |
|----------|----------|
| **Крейт** | `gergios-panel` |
| **Зависимости** | Compositor (`layer-shell` protocol), Lua scripting |
| **Статус** | 🔴 С нуля |
| **Что делать** | Панель с часами, треем, меню |

**MVP**: Панель вверху/внизу экрана. Часы, системный трей, меню приложений.
**Post-MVP**: Виджеты (погода, загрузка CPU/RAM, сеть, батарея),
Lua-скриптуемые блоки.

### 2.2 Desktop Notifications

| Свойство | Значение |
|----------|----------|
| **Крейт** | `gergios-notify` |
| **Зависимости** | Compositor, `notification-daemon` protocol |
| **Статус** | 🔴 С нуля |
| **Что делать** | Система уведомлений |

**MVP**: Всплывающие уведомления с текстом и иконкой. Очередь уведомлений.
**Post-MVP**: Интерактивные уведомления (кнопки), история, группировка.

### 2.3 Clipboard Manager

| Свойство | Значение |
|----------|----------|
| **Крейт** | `gergios-clipboard` |
| **Зависимости** | Compositor (`wl_data_device` protocol) |
| **Статус** | 🔴 С нуля |
| **Что делать** | Буфер обмена с историей |

**MVP**: Copy/paste между приложениями (текст).
**Post-MVP**: История буфера обмена, изображения, файлы.

---

## 3. Multimedia & Graphics

### 3.1 Image Viewer

| Свойство | Значение |
|----------|----------|
| **Крейт** | `gergios-image` |
| **Зависимости** | `image` crate (png, jpeg, gif decoding), GUI toolkit |
| **Статус** | 🔴 С нуля |
| **Сложность** | Средняя |

**MVP**: Открытие PNG/JPEG, zoom, pan, fullscreen.
**Post-MVP**: Слайдшоу, поддержка GIF, WebP, SVG, редактирование (rotate, crop).

### 3.2 Audio Player

| Свойство | Значение |
|----------|----------|
| **Крейт** | `gergios-music` |
| **Зависимости** | `minix-hda` (audio output), кодеки (Symphonia или minimp3-rs) |
| **Статус** | 🟡 `minix-hda` + `minix-audio` существуют |
| **Сложность** | Высокая |

**MVP**: Play/Pause/Stop MP3/WAV. Плейлист. Громкость.
**Post-MVP**: Поддержка FLAC, OGG, AAC. Эквалайзер. Библиотека музыки.
Стриминг (через `minix-net`).

### 3.3 Screenshot Tool

| Свойство | Значение |
|----------|----------|
| **Крейт** | `gergios-screenshot` |
| **Зависимости** | Compositor (`wlr-screencopy` protocol), PNG export |
| **Статус** | 🟡 В `MemBackend` уже есть PNG export (через `image` crate) |
| **Сложность** | Низкая |

**MVP**: Скриншот всего экрана → save to file.
**Post-MVP**: Скриншот области / окна, задержка, редактирование.

---

## 4. Network Applications

### 4.1 Web Browser (Minimal)

| Свойство | Значение |
|----------|----------|
| **Крейт** | `gergios-webkit` или `gergios-net-surf` |
| **Зависимости** | `minix-net` (TCP/IP), TLS (wolfSSL), HTML/CSS engine |
| **Статус** | 🔴 С нуля |
| **Сложность** | 🟢 Очень высокая |
| **Примечание** | Реалистично: портировать NetSurf или Dillo. Не писать с нуля |

**MVP**: Просмотр HTML-страниц без CSS/JS.
**Post-MVP**: CSS, JS (через SpiderMonkey или QuickJS), TLS, закладки, история.

### 4.2 Terminal-based Network Tools (CLI — уже готовы)

- `bt-tool` — Bluetooth CLI (✅ существует)
- `minix-net` — TCP/UDP библиотека (✅ существует)
- `packet-filter` — фильтрация пакетов (✅ существует)
- `net-parse` — парсинг DNS/TCP/UDP (✅ существует)

**Что нужно**: GUI-обёртки для этих инструментов (например, GUI для `bt-tool`, сетевой монитор).

---

## 5. Development Tools

### 5.1 Integrated Development Environment (IDE)

| Свойство | Значение |
|----------|----------|
| **Крейт** | `gergios-ide` |
| **Зависимости** | Text editor, syntax highlighting, terminal |
| **Статус** | 🔴 С нуля |
| **Сложность** | 🟢 Очень высокая |

**MVP**: Редактор кода + встроенный терминал. Подсветка синтаксиса (через `syntect`).
**Post-MVP**: GDB integration (через `planning/24_gdb_stub_debugger.md`),
проекты, git GUI, отладчик, autocomplete (через LSP).

### 5.2 Disk Usage Analyzer

| Свойство | Значение |
|----------|----------|
| **Крейт** | `gergios-duc` или `gergios-baobab` |
| **Зависимости** | GUI toolkit |
| **Статус** | 🔴 С нуля |
| **Сложность** | Средняя |

**MVP**: Древовидная карта дискового пространства (treemap или sunburst).
**Post-MVP**: Фильтрация по типу файлов, анализ ext4 через `ext4-core`.

### 5.3 System Monitor

| Свойство | Значение |
|----------|----------|
| **Крейт** | `gergios-monitor` |
| **Зависимости** | GUI toolkit, `procfs-path` (process list) |
| **Статус** | 🔴 С нуля |
| **Сложность** | Средняя |

**MVP**: Список процессов, CPU/RAM графики, kill процесс.
**Post-MVP**: Сеть I/O, диск I/O, температура, управление сервисами (через RS).

---

## 6. Productivity

### 6.1 Calculator

| Свойство | Значение |
|----------|----------|
| **Крейт** | `gergios-calc` |
| **Зависимости** | GUI toolkit |
| **Статус** | 🔴 С нуля |
| **Сложность** | Низкая |
| **Примечание** | Отличный starter project для знакомства с GUI toolkit |

**MVP**: Базовые операции (+ − × ÷), память (MC/MR/MS/M+).
**Post-MVP**: Научный режим (sin, cos, log), история, программируемый.

### 6.2 Calendar

| Свойство | Значение |
|----------|----------|
| **Крейт** | `gergios-cal` |
| **Зависимости** | GUI toolkit |
| **Статус** | 🟡 CLI `cal` уже есть в Rust |
| **Сложность** | Низкая |

**MVP**: Календарь на месяц/год. Выбор даты.
**Post-MVP**: Напоминания, события, импорт/экспорт iCal.

### 6.3 Clock / Alarm

| Свойство | Значение |
|----------|----------|
| **Крейт** | `gergios-clock` |
| **Зависимости** | GUI toolkit |
| **Статус** | 🔴 С нуля |
| **Сложность** | Низкая |

**MVP**: Аналоговые/цифровые часы. Будильник.
**Post-MVP**: Таймер, секундомер, мировое время.

---

## 7. Graphics & Creative

### 7.1 Simple Drawing / Paint

| Свойство | Значение |
|----------|----------|
| **Крейт** | `gergios-paint` |
| **Зависимости** | GUI toolkit, `pixel_buffer` (pixel ops) |
| **Статус** | 🟡 `PixelBuffer` уже имеет fill/draw/line/triangle |
| **Сложность** | Средняя |

**MVP**: Кисть, ластик, линии, прямоугольники, заливка. Выбор цвета.
**Post-MVP**: Слои, фильтры, поддержка PNG экспорта.

### 7.2 Screensaver

| Свойство | Значение |
|----------|----------|
| **Крейт** | `gergios-screensaver` |
| **Зависимости** | Compositor |
| **Статус** | 🔴 С нуля |
| **Сложность** | Низкая |

**MVP**: Отключение экрана / простые анимации.
**Post-MVP**: GL-accelerated screensavers (через wgpu), lock screen.

---

## 8. Games

### 8.1 Simple Games (gertoys-based)

| Игра | Статус | Примечание |
|------|--------|------------|
| `arithmetic` | 🟡 text CLI | Нужен GUI |
| `banner` | 🟡 text CLI | ASCII art → GUI |
| `bcd` | 🟡 text CLI | Brainfuck numerals |
| `caesar` | 🟡 text CLI | Caesar cipher |
| `factor` | 🟡 text CLI | Prime factorization |
| `fortune` | 🟡 text CLI | Fortune cookies |
| `morse` | 🟡 text CLI | Morse code |
| `number` | 🟡 text CLI | Number converter |
| `pig` | 🟡 text CLI | Pig Latin |
| `ppt` | 🟡 text CLI | Paper scissors rock |
| `random` | 🟡 text CLI | Random numbers |

**Все CLI-игры в `gertoys` уже портированы на Rust**. После GUI нужно:

1. **2048 / Snake / Tetris** (classic, полчаса реализации с `PixelBuffer`)
2. **Minesweeper** (классика)
3. **Chess** (через `shakmaty` или `cozy-chess` crate)

### 8.2 Dedicated Games

| Игра | Крейт | Сложность |
|------|-------|-----------|
| 🟢 Сudoku | `gergios-sudoku` | Низкая |
| 🟡 Chess | `gergios-chess` | Средняя (AI через Stockfish?) |
| 🟡 Tetris | `gergios-tetris` | Средняя |
| 🟡 2048 | `gergios-2048` | Низкая |
| 🔴 Solitaire | `gergios-solitaire` | Средняя |

---

## 9. Implementation Priority Matrix

| Приоритет | Приложение | LOC (оценка) | Зависимости | Время |
|-----------|------------|-------------|-------------|-------|
| **P0** | Terminal | ~1500 | compositor + `minix-term` | 2-4 нед |
| **P0** | File Manager | ~2000 | GUI toolkit + `minix-rs` | 3-6 нед |
| **P0** | Panel / Launcher | ~1000 | compositor (layer-shell) | 2-3 нед |
| **P0** | Notifications | ~500 | compositor (notification) | 1-2 нед |
| **P1** | Text Editor | ~1500 | GUI toolkit | 3-6 нед |
| **P1** | Settings Panel | ~2000 | GUI toolkit + `minix-admin` | 4-8 нед |
| **P1** | Calculator | ~300 | GUI toolkit | 1 нед |
| **P1** | Image Viewer | ~800 | `image` crate | 2-3 нед |
| **P1** | Clock / Calendar | ~500 | GUI toolkit | 1 нед |
| **P2** | Audio Player | ~2000 | `minix-hda` + кодеки | 4-8 нед |
| **P2** | Screenshot | ~300 | compositor | 1 нед |
| **P2** | Simple Games | ~500 each | `PixelBuffer` | 1-2 нед each |
| **P2** | Paint | ~1500 | `PixelBuffer` | 3-6 нед |
| **P2** | System Monitor | ~1000 | GUI toolkit | 3-4 нед |
| **P3** | IDE | ~5000 | text editor + terminal | 3-6 мес |
| **P3** | Web Browser | ~10000 | NetSurf/Dillo port | 6-12 мес |
| **P3** | Disk Analyzer | ~800 | GUI toolkit | 2-3 нед |

**P0**: Must-have для минимального графического рабочего стола.
**P1**: Core usability — без них десктоп неполноценен.
**P2**: Улучшение качества жизни.
**P3**: Долгосрочные цели.

---

## 10. Архитектура GUI-приложения

```rust
// Типовая структура GUI-приложения на GergiOS
// После стабилизации compositor'a + toolkit'a

use gergios_toolkit::prelude::*;

fn main() {
    let app = App::new();

    let mut window = Window::new("File Manager", 800, 600);
    let mut vbox = VBox::new();

    // Toolbar
    let mut toolbar = Toolbar::new();
    toolbar.add_button("New Folder", icons::FOLDER_NEW, || {
        // callback
    });
    toolbar.add_button("Delete", icons::TRASH, || {
        // callback
    });

    // File list
    let mut file_list = ListView::new();
    file_list.set_model(FileSystemModel::new("/"));

    vbox.add(&toolbar);
    vbox.add(&file_list);
    window.set_child(&vbox);

    app.run();
}
```

---

## 11. Технические заметки

### 11.1 Реюз существующих C-утилит

Для некоторых GUI-функций можно использовать существующие C-утилиты через `std::process::Command`:
- `cp`, `mv`, `rm`, `mkdir` — file operations в файловом менеджере
- `grep`, `find` — поиск
- `pwhash` — управление пользователями
- `ifconfig`, `route` — сетевые настройки

Это снижает объём новой разработки.

### 11.2 Иконки

- Начать с Material Design Icons (open source, ~5000 icons)
- Использовать `include_bytes!()` для встраивания SVG/PNG в бинарник
- Формат: SVG для scalability, PNG для производительности

### 11.3 Темы оформления

- CSS-like темизация через Lua-скрипты
- Цветовые схемы (light/dark)
- Поддержка HiDPI (через `fractional-scale` protocol)

### 11.4 Локализация

- Использовать `gettext` или Rust-аналог `fluent-rs`
- Начать с: English (en), Русский (ru), Deutsch (de), 中文 (zh)

---

## 12. Related Documents

- `planning/11_gui_architecture.md` — GUI stack (Phase 1–6)
- `planning/03_migration_roadmap.md` — General migration plan
- `planning/18_complex_packages.md` — Deferred complex packages
- `planning/27_rust_bluetooth_stack.md` — Bluetooth apps
- `planning/25_network_stack_modernization.md` — Network apps
- `planning/24_gdb_stub_debugger.md` — Development tools
