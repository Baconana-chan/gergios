# GergiOS Portable Edition — Переносная ОС на USB

> **Статус**: 🔮 Pre-planning / Concept
> **Связанные**: `planning/30_server_build.md`, `planning/26_security_model_modernization.md`,
>   `planning/29_admin_shell.md`, `planning/28_games_platform.md`
> **Репозитории**: `releasetools/`, `minix/servers/`, `etc/rc.d/`, `minix/commands/setup/`

---

## 1. Executive Summary

### 1.1 Что это?

**GergiOS Portable** — редакция ОС, которая:
- Загружается с **USB-флешки** (или CD/DVD)
- Работает **полностью из RAM** — ни одного обращения к диску после загрузки
- **Не оставляет следов** на железе — вынул флешку, и компьютер «не знает», что на нём работали
- **Amnesic mode** — все данные сессии пропадают при выключении
- **Persistent mode** — /home опционально сохраняется на USB (зашифрован)
- **Paranoid mode** — при выключении RAM затирается нулями

### 1.2 Для кого

| Пользователь | Зачем |
|-------------|-------|
| **DevOps / SRE** | Работа с серверами с любого компьютера — вставил USB, сделал работу, вынул |
| **Security researcher** | OS для пентеста без следов на машине жертвы |
| **Журналист / активист** | Приватная ОС — никаких данных на чужом железе |
| **Обычный пользователь** | «Моя ОС в кармане» — вставил в любой ПК, работает |
| **IT-специалист** | Live CD для восстановления/диагностики |

### 1.3 Ключевые сценарии

```
Сценарий 1: Работа с серверами
┌──────────┐   ┌──────────┐   ┌──────────┐
│ Чужой ПК │ → │ USB с    │ → │ Вход по  │
│ (библио- │   │ GergiOS  │   │ SSH-ключу│
│ тека)    │   │ Portable │   │ → fix    │
└──────────┘   └──────────┘   └──────────┘
                                ↓
                          Вынул USB → нет следов

Сценарий 2: Приватный серфинг
┌──────────┐   ┌──────────┐   ┌──────────────┐
│ Рабочий  │ → │ USB с    │ → │ Tor browser  │
│ ПК       │   │ GergiOS  │   │ в RAM        │
│ (офис)   │   │ Portable │   └──────────────┘
└──────────┘   └──────────┘       ↓
                          Вынул USB → 0 данных на диске C:

Сценарий 3: Демонстрация/презентация
┌──────────┐   ┌──────────┐   ┌──────────────┐
│ Конфе-   │ → │ USB с    │ → │ minix-admin  │
│ ренция   │   │ GergiOS  │   │ dashboard    │
└──────────┘   └──────────┘   └──────────────┘
```

---

## 2. Что уже есть (инфраструктура)

### 2.1 RAM-загрузка ✅

| Компонент | Статус | Описание |
|-----------|--------|----------|
| `x86_ramimage.sh` | ✅ Есть | Собирает RAM-образ: ядро + модули + корневая ФС |
| `bootramdisk=1` | ✅ Есть | Параметр ядра: вся корневая в RAM |
| `create_ramdisk_image()` | ✅ Есть | Функция в `image.functions` |
| RAM-загрузка в QEMU | ✅ Работает | `qemu -kernel kernel -append "bootramdisk=1" -initrd "mod*"` |
| RAM-загрузка с USB | ✅ Работает | `x86_usbimage.sh`: USB + `bootramdisk=1` |

### 2.2 USB-загрузка ✅

| Компонент | Статус | Описание |
|-----------|--------|----------|
| `x86_usbimage.sh` | ✅ Есть | Скрипт: USB image + MFS root + bootloader |
| `nbpartition` | ✅ Есть | Создание partition table |
| `nbinstallboot` | ✅ Есть | Установка bootsector |
| `boot_monitor` | ✅ Есть | Загрузчик в образе |
| GRUB/Limine | ✅ Есть | UEFI + BIOS загрузка |

### 2.3 Файловые системы ✅

| ФС | Статус | Для чего |
|----|--------|----------|
| **MFS** (MINIX FS) | ✅ Работает | Корневая ФС в RAM |
| **ext4** | ✅ Работает | /home на USB (persistent mode) |
| **tmpfs** | ❌ Нет | Нужен для /tmp, /var/log, amnesic /home |
| **MFS в RAM** | ✅ Работает | Через `create_ramdisk_image` |

### 2.4 Что отсутствует ❌

| Компонент | Почему | Сложность |
|-----------|--------|-----------|
| **tmpfs driver** | Нет в MINIX — нужно написать | 🔴 Сложно |
| **Amnesic mode** | Нет init скрипта | 🟢 Легко |
| **Persistent /home** | Есть ext4 — нужен init скрипт монтирования | 🟢 Легко |
| **Шифрование persist** | Нужен crypto в user-space | 🟡 Средне |
| **Overlayfs** (read-only USB) | Нет в MINIX | 🔴 Сложно |
| **USB auto-detection** | Есть devman — нужен скрипт | 🟡 Средне |

---

## 3. Архитектура

### 3.1 Общая схема

```
┌────────────────────────────────────────────────────────────────┐
│                     USB-флешка (FAT32 / MFS)                    │
│                                                                  │
│  /boot/grub/grub.cfg          ← Limine или GRUB                  │
│  /boot/kernel                 ← ядро GergiOS                     │
│  /boot/mod*                   ← boot modules (сервисы)            │
│  /boot/minix_root.mfs.gz      ← корневая ФС (сжатая)             │
│  /persist/home.mfs            ← /home (опционально, зашифрован)   │
│  /gergios-portable.conf        ← конфигурация Portable Edition    │
└────────────────────────────────────────────────────────────────┘
         │
         ▼ загрузка
┌────────────────────────────────────────────────────────────────┐
│                      Boot process                               │
│                                                                  │
│  1. BIOS/UEFI → GRUB/Limine                                     │
│  2. GRUB загружает kernel + mod*                                 │
│  3. kernel: bootramdisk=1                                        │
│  4. init: находит корневую ФС на USB                             │
│  5. init: копирует всю корневую ФС в RAM                         │
│  6. init: монтирует /home в RAM (или с USB)                      │
│  7. init: запускает сервисы                                      │
│  8. Готово: система работает полностью из RAM                   │
└────────────────────────────────────────────────────────────────┘
         │
         ▼ выключение
┌────────────────────────────────────────────────────────────────┐
│                      Shutdown process                           │
│                                                                  │
│  Amnesic mode:                                                   │
│  • sync + unmount                                                │
│  • RAM теряется при выкл. питания → никаких следов               │
│  • USB не был смонтирован на запись → чист                       │
│                                                                  │
│  Persistent mode:                                                │
│  • sync + unmount USB                                            │
│  • /home на USB сохранён                                         │
│                                                                  │
│  Paranoid mode:                                                  │
│  • dd if=/dev/zero of=/home/zero bs=1M                           │
│  • rm -f /home/zero                                              │
│  • shutdown                                                      │
└────────────────────────────────────────────────────────────────┘
```

### 3.2 Режимы работы

| Режим | /home | /tmp, /var/log | Следы на USB | Следы в RAM |
|-------|-------|---------------|-------------|-------------|
| **Amnesic** | RAM (tmpfs) | RAM (tmpfs) | ❌ Нет | ❌ Пропадают при выкл. |
| **Persistent** | USB (ext4) | RAM (tmpfs) | ✅ /home сохранён | ❌ Пропадают |
| **Persistent + Encrypted** | USB (LUKS) | RAM (tmpfs) | ✅ /home зашифрован | ❌ Пропадают |
| **Paranoid** | RAM (tmpfs) + затирание | RAM + затирание | ❌ Нет | ✅ Затёрты нулями |

### 3.3 Файл конфигурации `/gergios-portable.conf`

```bash
# /gergios-portable.conf — конфигурация Portable Edition
#
# Режимы: amnesic | persistent | paranoid
MODE="amnesic"

# Настройки для persistent mode
PERSIST_DEVICE="/dev/c0d0p1"     # или auto-detect
PERSIST_FS="ext4"                # или mfs
PERSIST_ENCRYPT="no"             # yes | no
PERSIST_PASSPHRASE=""            # если пусто — запросить при загрузке

# Сеть
NETWORK_DHCP="yes"               # DHCP или статика
NETWORK_SSID=""                   # WiFi SSID (если есть WiFi)
NETWORK_PSK=""                    # WiFi пароль

# SSH-ключи для входа
SSH_AUTHORIZED_KEYS=""            # можно вставить ключи
SSH_PASSWORD_AUTH="no"

# Аудит и безопасность
AUDIT_ENABLED="yes"
MAC_ENFORCING="no"                # permissive по умолчанию (логи без блокировок)

# Параноидальный режим
PARANOID_PASSES="1"               # количество проходов затирания
PARANOID_ZERO="yes"               # затереть RAM нулями
PARANOID_RANDOM="no"              # затереть RAM случайными данными
```

---

## 4. План реализации

### 4.1 Milestones

| Milestone | Что готово | LOC | Время | Статус |
|-----------|-----------|-----|-------|--------|
| **M1** | `gergios-portable.sh` — image builder (обёртка USB + RAM) | ~150 | 1 день | 🔄 |
| **M2** | `rc.d/portable` — init скрипт (amnesic mode, монтирование RAM) | ~100 | 1 день | 🔄 |
| **M3** | `gergios-portable.conf` — конфиг | ~30 | 1 час | 🔄 |
| **M4** | `tmpfs` driver для MINIX | ~300 | 2-3 недели | 🔴 Сложно |
| **M5** | Persistent + encrypted /home | ~500 | 2 недели | 🟡 Средне |
| **M6** | Paranoid mode (затирание RAM при shutdown) | ~100 | 1 день | 🟢 Легко |
| **M7** | USB read-only + overlayfs | ~400 | 3 недели | 🔴 Сложно |
| **M8** | Документация + QEMU тесты | ~200 | 2 дня | 🟢 Легко |

**Итого**: ~1780 LOC, ~4-6 недель (без учёта tmpfs/overlayfs)

### 4.2 Структура репозитория

```
releasetools/
  gergios-portable.sh           — NEW: сборщик Portable образа
  portable.image.defaults       — NEW: конфиг для Portable сборки
  x86_usbimage.sh               — MOD: добавить поддержку portable.conf

etc/
  rc.d/
    portable                     — NEW: init скрипт
  gergios-portable.conf          — NEW: конфиг по умолчанию

sbin/
  portable-init                  — NEW: C/Rust утилита парсинга конфига

minix/usr.sbin/
  mkfs.tmpfs/                    — NEW: tmpfs драйвер (опционально)

docs/
  portable-edition-guide.md      — NEW: документация
```

---

## 5. Реализация MVP (M1+M2+M3) — 2 дня

### Шаг 1: `gergios-portable.sh`

```bash
#!/usr/bin/env bash
# gergios-portable.sh — собирает USB образ для Portable Edition

: ${ARCH=x86_64}
: ${SETS="minix-base"}
: ${IMG=gergios-portable-$(date +%Y%m%d).img}
: ${USB_DEVICE=""}        # если указать — записать сразу на USB

# 1. Собрать RAM-образ (как x86_ramimage.sh)
# 2. Скопировать на USB FAT32:
#    /boot/kernel
#    /boot/mod*
#    /boot/minix_root.mfs.gz
#    /gergios-portable.conf
# 3. Установить GRUB/Limine
# 4. Опционально: записать на USB-флешку

echo "Portable image created: ${IMG}"
echo "Write to USB: dd if=${IMG} of=/dev/sdX bs=4M"
```

### Шаг 2: `etc/rc.d/portable`

```bash
#!/bin/sh
# /etc/rc.d/portable — Portable Edition init скрипт

case $1 in
start)
    if [ -f /gergios-portable.conf ]; then
        . /gergios-portable.conf
    else
        MODE="amnesic"
    fi
    
    case "$MODE" in
    amnesic|paranoid)
        # /home в RAM — всё пропадёт
        mount -t tmpfs tmpfs /home 2>/dev/null || true
        cp -r /skel/. /home/ 2>/dev/null || true
        ;;
    persistent)
        # Монтируем /home с USB
        DEV="${PERSIST_DEVICE:-/dev/c0d0p1}"
        FS="${PERSIST_FS:-ext4}"
        mount -t "$FS" "$DEV" /home 2>/dev/null || {
            echo "Persist mount failed, falling back to RAM"
            mount -t tmpfs tmpfs /home
            cp -r /skel/. /home/
        }
        ;;
    esac
    
    # /tmp и /var/log всегда в RAM
    mount -t tmpfs tmpfs /tmp 2>/dev/null || true
    mount -t tmpfs tmpfs /var/log 2>/dev/null || true
    
    # SSH
    if [ -n "$SSH_AUTHORIZED_KEYS" ]; then
        mkdir -p /root/.ssh
        echo "$SSH_AUTHORIZED_KEYS" > /root/.ssh/authorized_keys
        chmod 600 /root/.ssh/authorized_keys
    fi
    ;;
    
stop)
    case "$MODE" in
    paranoid)
        echo "Paranoid wipe: zeroing RAM..."
        dd if=/dev/zero of=/home/zero bs=1M 2>/dev/null || true
        rm -f /home/zero
        dd if=/dev/zero of=/tmp/zero bs=1M 2>/dev/null || true
        rm -f /tmp/zero
        ;;
    esac
    sync
    ;;
esac
```

### Шаг 3: Тестирование в QEMU

```bash
# Собрать образ
./releasetools/gergios-portable.sh

# Загрузить в QEMU (ramdisk)
qemu-system-x86_64 --enable-kvm -m 1G \
    -kernel boot/kernel \
    -append "bootramdisk=1" \
    -initrd "boot/mod*"

# Или загрузить с USB-образа
qemu-system-x86_64 --enable-kvm -m 1G \
    -drive file=gergios-portable.img,if=virtio,format=raw
```

---

## 6. Технические риски

| Риск | Impact | Вероятность | Mitigation |
|------|--------|-------------|------------|
| **tmpfs не реализован в MINIX** | High | Medium | Обойтись MFS в RAM — вреда нет, только размер |
| **USB auto-detect сложен** | Medium | Low | Жёстко задать device в config |
| **QEMU virtio не работает с USB-образом** | Low | Medium | AHCI fallback |
| **GRUB не загружает с USB** | Low | Low | Использовать Limine (Multiboot2) |
| **Шифрование persist без LUKS** | Medium | Medium | Свой AES в user-space (wolfSSL) |
| **Затирание RAM не гарантировано на SSD** | Low | Low | SSD сам затирает при power-off (secure erase) |

---

## 7. Приложение

### 7.1 Сравнение с аналогами

| OS | Размер | RAM usage | Следы | Шифрование | Кастомизация |
|----|--------|-----------|-------|-------------|-------------|
| **Tails** | ~1.2GB | ~500MB | ✅ Amnesic | ✅ LUKS | ❌ Сложно |
| **Kali Linux Live** | ~3.5GB | ~1GB | ❌ (пишет в swap) | ❌ | ✅ |
| **SystemRescue** | ~800MB | ~500MB | ⚠️ (может писать) | ❌ | ✅ |
| **GergiOS Portable** | **~100MB** | **~15-20MB** | ✅ Amnesic + Paranoid | 🟡 План | ✅ (minix-admin) |

### 7.2 Аппаратные требования (target)

| Компонент | Минимально | Рекомендуется |
|-----------|-----------|---------------|
| **CPU** | x86_64, 1 core | x86_64, 2 cores |
| **RAM** | 256MB | 1GB+ |
| **USB** | USB 2.0, 1GB | USB 3.0, 4GB+ |
| **Графика** | VGA text mode | Framebuffer (в разработке) |
| **Сеть** | VirtIO / e1000 | Любая |

### 7.3 Use case: DevOps toolchain на USB

```bash
# Что включено в GergiOS Portable + Server Edition:
#
# Инструменты:
#   • WireGuard — VPN до серверов
#   • SSH — доступ
#   • curl/wget — HTTP
#   • git — репозитории
#   • minix-admin — управление
#   • auditctl + audit2txt — анализ
#
# Безопасность:
#   • MAC enforcing — защита от rogue процессов
#   • Audit — лог всех действий
#   • Capabilities — минимальные привилегии
#   • W^X — защита памяти
#   • SYN cookies — защита сети
#
# Конфиденциальность:
#   • Amnesic mode — 0 следов
#   • SSH ключи — никаких паролей
```

### 7.4 Use case: Приватный серфинг

```bash
# 1. Вставляешь USB в любой ПК
# 2. Загружаешься с USB (F12 → выбор загрузки)
# 3. GergiOS загружается за <1 секунду
# 4. Серфишь через Tor/SSH tunnel
# 5. Всё в RAM — никаких следов на HDD
# 6. Вынимаешь USB — компьютер «не знает», что на нём работали
# 7. Paranoid mode: RAM затёрта нулями
```

---

## 8. Первый шаг

Начать с **MVP за 2 дня**:
1. `releasetools/gergios-portable.sh` — обёртка на базе `x86_usbimage.sh`
2. `etc/rc.d/portable` — init скрипт для amnesic mode
3. `etc/gergios-portable.conf` — конфиг
4. Протестировать: `bootramdisk=1` в QEMU
5. Проверить: amnesic mode не пишет на диск
