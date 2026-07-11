# Games Platform — Post-1.0 Vision

> **Status**: 🔮 Pre-planning / Concept
> **Связанные**: `games/` (оставшиеся игры), `planning/11_gui_architecture.md`,
>   `planning/25_network_stack_modernization.md` (multiplayer),
>   `planning/27_rust_bluetooth_stack.md` (BT gamepads)
> **Репозитории**: `rust/gertoys/`, `games/` (оставшиеся 10 C игр),
>   `minix/servers/fb/`, `minix/lib/libcurses/` (нуждается в портировании)

## 1. Текущее состояние

### 1.1 Что есть сейчас (10 C игр в games/)

| Игра | LOC | Тип | Стоит портировать в Rust? |
|------|-----|-----|--------------------------|
| **rogue** | 11,914 | Рогалик (легенда) | 🔮 Возможно, но огромный объём |
| **adventure** | 4,694 | Text adventure (классика) | 🟡 Можно (средний объём) |
| **monop** | 3,973 | Монополия (полноценная) | 🟡 Можно |
| **tetris** | 2,395 | Тетрис (бессмертный) | 🟢 Да! (мало, чёткая логика) |
| **primes** | 1,603 | Математическая утилита | 🟢 Уже почти — можно в gertoys |
| **snake** | 1,166 | Змейка (классика) | 🟢 Да |
| **fish** | 513 | Go Fish (карты) | 🟢 Да |
| **worm** | 369 | Worm (змея) | 🟢 Да (мало, просто) |
| **wargames** | 52 | Пасхалка («хочу сыграть?») | 🟢 Уже переписана |
| **wtf** | ~200 | Акронимы | 🟢 В gertoys |

**ИТОГО**: ~26,000 LOC C — потенциально все можно мигрировать в Rust.

### 1.2 Инфраструктура для игр (текущие пробелы)

| Компонент | Статус | Что нужно |
|-----------|--------|-----------|
| **Терминальный ввод** | ✅ Есть (stdin, termios) | OK для пошаговых игр |
| **ANSI escape / цвет** | ✅ Есть (терминал) | Цвета, курсор, очистка экрана |
| **framebuffer** | ✅ Есть (`/dev/fb`, `minix/drivers/video/fb/`) | Прямой доступ к пикселям |
| **input server** | ✅ Есть (`minix/servers/input/`) | Ввод с клавиатуры / мыши |
| **libcurses / ncurses** | ❌ НЕТ | Порт ncurses или своя минимальная curses-подобная библиотека |
| **LibreSSL / OpenSSL** | ❌ заменён на wolfSSL | OK — wolfSSL есть |
| **Звук** | ❌ НЕТ | Нет аудиодрайверов (HDA, USB audio — только планируются) |
| **Сеть для мультиплеера** | ⚠️ lwIP есть, UDP/TCP работают | DTLS, WireGuard — Phase 4 (в работе) |
| **Bluetooth gamepad** | ✅ Есть (`minix-term::gamepad` + `minix-bt-stack::hidp`) | DS4/DS5/Switch Pro/Xbox/Generic парсеры, `Gamepad` API |
| **Rust game engines** | ❌ НЕТ | `crossterm`, `ratatui`, или самописный рендерер |
| **Таймеры / FPS** | ⚠️ `sys_setalarm()` / `SIGALRM` | Работает, но не real-time |

---

## 2. Инфраструктурные задачи (что нужно сделать до игр)

### 2.1 🔺 P0: Минимальный терминальный UI (curses)

**Вариант A**: Портировать `ncurses` (5.x)
- +Совместимость со всеми ncurses-играми (nethack, angband, bastet, moon-buggy)
- +Тысячи готовых игр работают
- −Большой объём портирования (~500K LOC)
- −MINIX TTY архитектура может не поддерживать все terminfo-фичи

**Вариант B**: Минимальная Rust библиотека (аналог crossterm/termion)
- + 200-500 LOC Rust vs 500K LOC C
- + Поддержка только нужного: цвет, курсор, raw mode, клавиши
- + Работает через `write()` с ANSI escape кодами
- −Несовместимо с существующими ncurses-программами

**Рекомендация**: **Вариант B** — `rust/minix-term/` crate для терминального UI (✅ **Реализован**)
```
pub enum Key {
    Char(char),
    Up, Down, Left, Right,
    Esc, Enter, Backspace,
    Ctrl(char),
    Unknown,
}

pub struct Terminal {
    // raw mode state
}

impl Terminal {
    pub fn new() -> io::Result<Self>;  // включить raw mode
    pub fn read_key(&mut self) -> Key; // читать одну клавишу
    pub fn clear(&self);               // ESC[2J
    pub fn set_cursor(&self, row: u16, col: u16); // ESC[row;colH
    pub fn hide_cursor(&self);         // ESC[?25l
    pub fn show_cursor(&self);         // ESC[?25h
    pub fn set_fg(&self, color: u8);   // ESC[38;5;Nm
    pub fn set_bg(&self, color: u8);   // ESC[48;5;Nm
    pub fn reset_style(&self);         // ESC[0m
    pub fn size(&self) -> (u16, u16);  // терминал: строки, колонки
    pub fn poll(&self, ms: u64) -> bool; // есть ли ввод с таймаутом
}
impl Drop for Terminal; // восстановить original termios
```

### 2.2 🔺 P1: Framebuffer доступ для 2D игр

framebuffer уже есть (`minix/drivers/video/fb/`), но:
- Нет Rust FFI bindings для `/dev/fb`
- Нет двойной буферизации для плавного рендеринга
- Нет утилит для работы с поверхностями (blit, fill, sprite)

**Нужно**: `rust/minix-fb/` crate — минимальный framebuffer API:
```rust
pub struct Framebuffer {
    pub width: u32,
    pub height: u32,
    pub bpp: u8,
    pub pitch: u32,
    buffer: &'static mut [u8],
}

impl Framebuffer {
    pub fn open() -> io::Result<Self>;  // mmap /dev/fb0
    pub fn pixel(&mut self, x: u32, y: u32, r: u8, g: u8, b: u8);
    pub fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, r: u8, g: u8, b: u8);
    pub fn blit(&mut self, src: &[u8], sx: u32, sy: u32, w: u32, h: u32, dx: u32, dy: u32);
    pub fn flip(&mut self);  // double buffer swap
}
```

### 2.3 🟡 P2: Input для игр

- Клавиатура в raw mode: ✅ Есть (termios)
- Мышь: ✅ **Реализован** `minix-term::mouse` — чтение `/dev/mouse` с парсингом `input_event` (кнопки, движение, скролл)
- Gamepad: ✅ **Реализован** — `minix-term::gamepad` (USB: `/dev/gamepad*`) + `minix-bt-stack::hidp` (BT: HID over GATT). Поддержка DS4, DS5, Switch Pro, Xbox BLE, Generic

### 2.4 🟡 P2: Аудио

- PCM playback: ✅ **Реализован** `rust/minix-audio/` — AudioDevice (set_format, write, drain, flush, volume), Mixer (громкость), WAV loader, звуковой синтезатор (sine/square/saw/triangle wave, laser, explosion, powerup, coin, death)
- Miixeur / volume: ✅ **Включено** в `minix-audio::Mixer` (read/write через /dev/mixer)
- HDA / AC97 / ES1370 / SB16 / другие драйверы: ✅ **Уже есть** в `minix/drivers/audio/`

**Для игр**: PC speaker beeper хватит для MVP.

### 2.5 🔵 P3: Сетевой мультиплеер

- TCP клиент-сервер: ✅ **Реализован** `rust/minix-net` — NetServer (bind/listen/accept/recv/send/broadcast), NetClient (connect/send/recv), ServerEvent (Connected/Message/Disconnected), Message framing (length+tag+payload)
- UDP peer-to-peer: ✅ **Реализован** `rust/minix-net` — UdpPeer (bind/send_to/recv_from/send_message/recv_message), broadcast, TTL, timeout
- NAT traversal / STURN: ⚠️ Нужен порт
- WebSocket (для Web-клиентов): ⚠️ Порт на lwIP

---

## 3. Типы игр, возможные на GergiOS

### По сложности реализации 🔵 🟡 🔴

| Категория | Примеры | Сложность | Что нужно |
|-----------|---------|-----------|-----------|
| **Text-based пошаговые** | Шахматы, Го, Нарды, Морской Бой | 🟢 **Легко** | только stdin/stdout + ANSI |
| **Text-based карточные** | Пасьянс, Джин, Покер, Blackjack | 🟢 **Легко** | только stdin/stdout + ANSI |
| **Curses-игры** | NetHack, Angband, Moon-buggy | 🟡 Порт curses | ncurses или minix-term |
| **ASCII-roguelike** | Свой roguelike с генерацией уровней | 🟡 **Средне** | minix-term (или просто ANSI) |
| **Тетрис / змейка** | Tetris, Snake, Pong | 🟡 **Средне** | minix-term + таймер (sys_setalarm) |
| **Text-based RPG** | MUD-like, Interactive fiction | 🟡 **Средне** | Интерпретатор + data файлы |
| **SDL-игры** | Quake, Doom (через SDL) | 🔴 **Сложно** | SDL port (огромная работа) |
| **2D игры (framebuffer)** | HoMM-like, Arcanoid, Platformer | 🔴 **Сложно** | minix-fb + рендерер + звук |
| **Сетевые игры** | online chess, multiplayer roguelike | 🔴 **Сложно** | Сеть + протокол + синхронизация |

---

## 4. План реализации (пост-1.0 roadmap)

### Phase A: Фундамент

| Задача | Приоритет | Статус | Зависимости |
|--------|-----------|--------|-------------|
| **A1**: Создать `rust/minix-term/` crate | 🔺 P0 | ✅ **Готов** (v0.1.0) | Rust workspace |
| **A2**: Создать `rust/minix-fb/` crate | 🔺 P1 | ✅ **Готов** (v0.1.0) | `/dev/fb` |
| **A3**: Raw mode keyboard input wrapper | 🔺 P0 | ✅ **Включено** в `minix-term` | `minix-term` |
| **A4**: Mouse input wrapper (`mouse.rs`) | 🟡 P2 | ✅ **Готов** (v0.1.1) | `minix-term`, input server |

### Phase B: Первые игры в Rust (2-3 месяца)

Игры, которые можно сделать на **только ANSI терминале** (без curses):

| Игра | Команда | LOC | Почему именно эта |
|------|---------|-----|-------------------|
| **snake** | `gertoys snake` | ~300 | Классика, все знают |
| **tetris** | `gertoys tetris` | ~500 | Бессмертная, чёткая логика |
| **pong** | `gertoys pong` | ~400 | Двухплеер, простой рендеринг |
| **minesweeper** | `gertoys minesweeper` | ~400 | Пошаговая, стратегия |
| **chess** | `gertoys chess` | ~800 | Классика, чёткие правила |
| **breakout** | `gertoys breakout` | ~400 | Аркада с мячиком |

**Примечание**: Эти игры будут в `gertoys` как субкоманды, используя `minix-term`.

### Phase C: Продвинутые терминальные игры (3-6 месяцев)

| Игра | Зависимости | LOC | Описание |
|------|-------------|-----|----------|
| **Roguelike** (`minix-rogue`) | `minix-term` | ~3,000 | Свой roguelike с генерацией уровней (не порт rogue, а новый) |
| **MUD** (`minix-mud`) | `minix-term` + lwIP | ~5,000 | Мультиплеерный text-based RPG сервер |
| **Battleship** | `minix-term` + lwIP | ~800 | Морской бой по сети |
| **Hack / NetHack порт** | ncurses port | ~100,000 | Если будет ncurses |

### Phase D: 2D игры (6-12 месяцев)

Зависит от `minix-fb` + базового рендерера + аудио.

---

## 5. 🎮 Особый проект: HoMM-like игра для GergiOS

**Идея**: Легковесная стратегия в духе **Heroes of Might and Magic** (HoMM 2/3),
работающая в терминале через ASCII/ANSI графику (не framebuffer).

### 5.1 Почему HoMM подходит для терминала?

| Аспект | HoMM | Терминальная реализация |
|--------|------|------------------------|
| **Карта приключений** | Hex-сетка (квадратная или гексагональная) | ASCII символы (`#` лес, `~` вода, `.` трава) |
| **Герои** | Маленькие спрайты | `@` (герой) с цветом фракции |
| **Замки** | 2D buildings | `[]` с флагом фракции |
| **Битвы** | Hex-сетка 6×10 | `H` ходячий, `A` лучник, `F` летающий |
| **Экономика** | Золото + ресурсы | Числа с цветами |
| **Диалоги** | Окна с текстом | Текстовые панели с рамками |
| **Музыка** | MIDI/MP3 | ❌ Нет (PC speaker beep?) |

### 5.2 Архитектура (Rust)

```
rust/minix-homm/
├── Cargo.toml
└── src/
    ├── main.rs              — Точка входа + главное меню
    ├── game.rs              — Игровой цикл (turn-based)
    ├── map.rs               — Генерация и рендеринг карты
    ├── map_gen.rs           — Процедурная генерация
    ├── hero.rs              — Герой (статы, инвентарь, уровень)
    ├── army.rs              — Армия (юниты, стек, мораль)
    ├── unit.rs              — Юниты (типы, статы, способности)
    ├── town.rs              — Замок (здания, найм, гарнизон)
    ├── combat.rs            — Битва (hex-сетка, AI, пошагово)
    ├── combat_ai.rs         — AI для битв
    ├── economy.rs           — Ресурсы, доход, постройки
    ├── objects.rs           — Объекты на карте (сундуки, артефакты)
    ├── quest.rs             — Квесты/сценарии
    ├── save.rs             — Сохранение/загрузка
    ├── term.rs              — minix-term обёртка
    └── ui.rs                — Меню, окна, подсказки
```

**Примерный объём**: 5,000-8,000 LOC (против ~100,000 LOC у HoMM 3).

### 5.3 Визуальный концепт

```
╔══════════════════════════════════════════════╗
║             ~ HEROES of GERGIOS ~            ║
║   День 7     Неделя 1     Месяц 1            ║
║                                             ║
║  ╔═══╗  ║          ~~~~~~~~~~              ║
║  ║ R ║  ║   . . . .~ ~~~~~                 ║
║  ╚═══╝  ║  . @ . . . ~ . . . .             ║
║  Castle ║  . . . . . . . . . . .           ║
║         ║   ### ##      . . . . .  ~~~      ║
║  Gold:  ║   ## ####    . H . .   ~~~        ║
║   1500  ║    ###         . . . .            ║
║  Wood:  ║                                  ║
║    12   ║        ╔═══╗                     ║
║         ║        ║ E ║  Enemy              ║
╚═════════╩════════╚═══╝══════════════════════╝
   [W]alk  [I]nfo  [C]astle  [N]ext Hero  [Q]uit
```

### 5.4 Сложность реализации

| Компонент | LOC | Сложность | Комментарий |
|-----------|-----|-----------|-------------|
| **Карта + генерация** | 1,000 | 🟡 Средне | Perlin-like шум, биомы, реки |
| **Герои** | 500 | 🟢 Легко | Статы, уровни, артефакты |
| **Армия + юниты** | 800 | 🟢 Легко | 20+ юнитов, статы, способности |
| **Замки** | 800 | 🟡 Средне | 6-8 фракций, градация зданий |
| **Битвы (самое сложное)** | 2,000 | 🔴 Сложно | Hex-сетка, AI путей, мораль, магия |
| **Экономика** | 400 | 🟢 Легко | Ресурсы, доход, цены |
| **Объекты** | 500 | 🟢 Легко | Сундуки, артефакты, лагеря |
| **UI/терминал** | 800 | 🟡 Средне | Окна, меню, тултипы |
| **Сохранение** | 300 | 🟢 Легко | JSON / bincode serialization |
| **Сценарии/кампания** | 500 | 🟡 Средне | Скрипты для миссий |

**Итого**: ~7,500 LOC — реалистично для одного разработчика за 3-6 месяцев.

### 5.5 Milestones

| Milestone | Что готово | Время |
|-----------|-----------|-------|
| **M1** | Меню, генерация карты, герой ходит | 2-3 недели |
| **M2** | Замки, экономика, найм юнитов | +2 недели |
| **M3** | Битвы (base: hero vs monster) | +4 недели |
| **M4** | AI для битв, полные армии | +3 недели |
| **M5** | Объекты, артефакты, уровни героя | +2 недели |
| **M6** | Сохранение/загрузка, сценарии | +2 недели |
| **M7** | Полировка, баланс, 2+ фракции | +3 недели |

**Всего**: ~16-20 недель.

---

## 6. Другие крупные игровые проекты (кандидаты после HoMM)

### 6.1 🏰 Civilization-like (Terminal ASCII)

- Гексагональная карта (уже сделано для HoMM)
- Технологическое дерево (data-driven, легко)
- Города + рабочие + армии (средняя сложность)
- AI для 8 цивилизаций (сложно, но пошагово — проще чем real-time)

**Оценка**: 8,000-12,000 LOC, 6-9 месяцев

### 6.2 🧙 MUD (Multi-User Dungeon)

- Мультиплеер (lwIP TCP)
- Комнаты, предметы, мобы (text-based)
- Квесты, уровни, классы
- Сервер + telnet-клиент

**Оценка**: 5,000-8,000 LOC, 4-6 месяцев

### 6.3 ♟️ Dwarf Fortress-like (ASCII)

- Процедурная генерация мира (есть опыт с HoMM)
- Симуляция существ с AI
- Экономика и строительство
- Текстовый UI

**Оценка**: 15,000+ LOC, 12+ месяцев (очень ambitious)

### 6.4 🌌 Tradewars-like (BBS style)

- Космическая торговля и сражения
- Мультиплеер через TCP
- Экономика, корабли, апгрейды
- Текстовый UI

**Оценка**: 3,000-5,000 LOC, 2-3 месяца

---

## 7. Сводная карта (значимость vs. сложность)

```
Значимость
   ^
10 |  HoMM       DF
   |    🎮      🏔️
 8 |        Civ
   |         🏛️
 6 |  MUD
   |   🧙    Tradewars
 4 |           🚀
   |     Tetris Snake Chess
 2 |       🟢 🟢 🟢
   +--------------------------------→ Сложность
     2   4   6   8   10  12  14
```

**Золотая середина**: HoMM (значимость 8-9, сложность 5-6).

---

## 8. Дорожная карта (рекомендованная)

```
Месяц 1-2:   ▓▓▓▓▓▓░░░░  Phase A: Фундамент (minix-term, minix-fb)
Месяц 2-4:   ░░▓▓▓▓▓▓░░  Phase B: Первые игры (tetris, snake, chess)
Месяц 4-5:   ░░░░░░▓▓▓░  Phase C: Продвинутые (roguelike, battleship)
Месяц 5-10:  ░░░░░░░▓▓▓  Phase D: HoMM (начало: 5-8 мес)
Месяц 10+:   ░░░░░░░░░░  HoMM (продолжение), сетевые режимы, DLC фракции
```

---

## 9. Технические решения

### 9.1 Почему Rust, не C?

| Аспект | Rust | C |
|--------|------|---|
| **Безопасность** | ✅ Без segfaults, use-after-free | ⚠️ Легко ошибиться с памятью |
| **Сериализация** | ✅ `serde` (JSON, bincode, msgpack) | ❌ Всё руками |
| **Сеть** | ✅ `std::net` (через lwIP FFI) | ⚠️ Нет стандартной библиотеки |
| **Итерация** | ✅ `cargo check` за секунды | ⚠️ Полная сборка минут |
| **Экосистема** | ⚠️ Нет крейтов под MINIX | ⚠️ Нет пакетов вообще |
| **Размер бинарника** | ⚠️ ~1-5 МБ | ✅ ~100-500 КБ |

**Вердикт**: Rust лучше для игр (безопасность + serde + быстрая итерация), 
даже с учётом большего размера бинарника.

### 9.2 Процедурная генерация (для HoMM и roguelike)

- **Шум**: Симплекс-шум (алгоритм без таблиц, 100 LOC)
- **Биомы**: Температура + влажность → биом (20 LOC)
- **Реки**: Алгоритм случайного блуждания от гор к морю
- **Города**: Размещение по правилам (равнина, расстояние, ресурсы)
- **Подземелья**: BSP partitioning или random-walk

### 9.3 Рендеринг в терминале

```rust
// Минимальный рендерер для hex-сетки
struct HexRenderer {
    term: Terminal,
    hex_width: u16,   // ширина hex в символах (обычно 4)
    hex_height: u16,  // высота hex в символах (обычно 2)
}

impl HexRenderer {
    // Конвертация hex (q,r) в экранные координаты
    fn hex_to_screen(&self, q: i32, r: i32) -> (u16, u16) {
        let x = self.hex_width as i32 * q;  // нечётные q сдвиг
        let y = self.hex_height as i32 * r;
        // offset для нечётных рядов
        (x as u16, y as u16)
    }

    fn draw_hex(&self, q: i32, r: i32, terrain: &Terrain, fg: u8, bg: u8) {
        let (sx, sy) = self.hex_to_screen(q, r);
        self.term.set_cursor(sy, sx);
        self.term.set_fg(fg);
        self.term.set_bg(bg);
        write!(self.term, "{}", terrain.as_symbol());
    }
}
```

---

## 10. Смежные / не столь важные, но интересные идеи

### 10.1 ASCII Screensaver-ы (простые, эффектные)
- Matrix дождь из символов
- Fire эффект (только ANSI цвета)
- Plasma / синусоидные волны
- Starfield (3D симуляция пространства)

### 10.2 Эмуляторы ретро-консолей
- Chip-8 интерпретатор (~200 LOC)
- Game Boy (LR35902) эмулятор (~3,000 LOC, framebuffer нужен)
- NES эмулятор (~5,000 LOC, framebuffer)

### 10.3 Интерактивные истории / Visual novels (text-heavy, терминал)
- Twine-like движок для интерактивных историй
- Текстовые квесты с выбором

---

## 11. Известные проблемы / ограничения

### 11.1 Отсутствие ncurses
**Проблема**: Без ncurses нельзя просто взять и портировать NetHack, Angband,
Dwarf Fortress и сотни других игр.

**Решения**:
1. Написать минимальный совместимый слой (`libminix-curses`), который
   перехватывает базовые ncurses вызовы (initscr, mvprintw, getch) —
   ~500 LOC vs 500K LOC ncurses.
2. Ждать официального порта ncurses (низкий приоритет у сообщества MINIX).

### 11.2 Отсутствие аудио
**Проблема**: Без звука нет музыкального сопровождения, звуковых эффектов.

**Решения**:
- PC speaker beeper для MVP
- HDA драйвер (в Rust) для полноценного звука — отдельный проект

### 11.3 Производительность рендеринга
**Проблема**: ANSI escape коды медленнее прямого framebuffer доступа.

**Решения**:
- Минимизировать количество `write()` вызовов (буферизировать экран в памяти
  и выводить diff)
- Для fps-зависимых игр использовать framebuffer (P1 задача)

---

## Приложение A: Список игр для немедленного портирования в gertoys

| Игра | C LOC | Rust LOC | Команда gertoys |
|------|-------|----------|-----------------|
| tetris | 2,395 | ~800 | `gertoys tetris [-l]` |
| snake | 1,166 | ~400 | `gertoys snake` |
| worm | 369 | ~200 | `gertoys worm` |
| fish | 513 | ~300 | `gertoys fish` |
| monop | 3,973 | ~1,500 | `gertoys monop` (ML-инфраструктура) |

**Замечание**: `tetris` и `snake` — идеальные кандидаты, потому что они:
- Маленькие (2,395 + 1,166 = ~3.5K LOC C → ~1.2K LOC Rust)
- Относительно независимые (не требуют curses — используют ANSI)
- Немедленно играбельные
- Высокая удовлетворённость пользователей

## Приложение B: Глоссарий игровых терминов

| Термин | Описание |
|--------|----------|
| **Roguelike** | Жанр: пошаговое подземелье с процедурной генерацией, permadeath |
| **MUD** | Multi-User Dungeon — текстовая многопользовательская RPG |
| **HoMM** | Heroes of Might and Magic — пошаговая стратегия с героями и армиями |
| **TBS** | Turn-Based Strategy — пошаговая стратегия |
| **RTS** | Real-Time Strategy — стратегия в реальном времени |
| **Hex grid** | Гексагональная система координат для карты |
| **Permadeath** | При смерти персонажа игра заканчивается (нет сохранений) |
| **Procedural generation** | Автоматическая генерация контента (карт, уровней) алгоритмами |
| **BSP** | Binary Space Partition — алгоритм для генерации подземелий |
