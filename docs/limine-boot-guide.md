# Limine Boot Guide for GergiOS

> **Полный цикл**: сборка Limine → создание образа → загрузка в QEMU
> **Статус**: Phase 0 prototype
> **Related**: `planning/16_bootloader_modernization.md`

---

## 1. Обзор

Этот guide описывает как собрать GergiOS с Limine bootloader и запустить в QEMU.
На данный момент реализована **Phase 0** — код поддержки Limine protocol в ядре
написан, но требуется ручная сборка образа для тестирования.

**Что нужно**:
- Linux (рекомендуется Ubuntu/Debian) или WSL2
- QEMU (qemu-system-x86_64)
- GCC cross-toolchain для MINIX/GergiOS (если собираете с нуля)
- Либо готовый GergiOS образ (kernel + модули)

---

## 2. Получение Limine

### Вариант A: Сборка из исходников (рекомендуется)

```bash
# Зависимости
sudo apt install git build-essential nasm

# Клонирование
git clone https://github.com/limine-bootloader/limine.git
cd limine

# Сборка (собирает host tool + bootloader бинарники)
make -j$(nproc)

# Проверка
./limine --help
# Должен показать help с командами bios-install, help и т.д.

# Сохраняем путь для дальнейшего использования
export LIMINE_DIR=$(pwd)
```

### Вариант B: Pre-built binaries

```bash
# Скачать с GitHub releases
wget https://github.com/limine-bootloader/limine/releases/download/v8.x.x/limine-binary.tar.gz
tar xzf limine-binary.tar.gz
cd limine-binary
export LIMINE_DIR=$(pwd)
```

---

## 3. Подготовка GergiOS

### 3.1 Если есть готовая сборка

```bash
# Структура файлов для образа
mkdir -p gergios_boot/boot

# Kernel (ELF64)
cp /path/to/build/kernel gergios_boot/boot/kernel

# Модули (серверы)
mkdir -p gergios_boot/boot/modules
cp /path/to/build/mod01_ds  gergios_boot/boot/modules/
cp /path/to/build/mod02_rs  gergios_boot/boot/modules/
cp /path/to/build/mod03_pm  gergios_boot/boot/modules/
cp /path/to/build/mod04_sched gergios_boot/boot/modules/
cp /path/to/build/mod05_vfs gergios_boot/boot/modules/
cp /path/to/build/mod06_memory gergios_boot/boot/modules/
cp /path/to/build/mod07_tty gergios_boot/boot/modules/
cp /path/to/build/mod08_mib gergios_boot/boot/modules/
cp /path/to/build/mod09_vm gergios_boot/boot/modules/
cp /path/to/build/mod10_pfs gergios_boot/boot/modules/
cp /path/to/build/mod11_mfs gergios_boot/boot/modules/
cp /path/to/build/mod12_init gergios_boot/boot/modules/
```

### 3.2 Если собираете с нуля (cross-compilation)

```bash
# Предполагается настроенный MINIX cross-toolchain
# (arm-elf32-minix- для ARM, i586-elf32-minix- для x86, и т.д.)

export CROSS_TOOLS=/path/to/cross/tools
export DESTDIR=/path/to/destdir

# Сборка через CMake
cd /path/to/gergios
mkdir build && cd build
cmake .. \
    -DCMAKE_TOOLCHAIN_FILE=../cmake/toolchain-minix.cmake \
    -DMACHINE_ARCH=x86_64 \
    -DUSE_LIMINE=ON

make kernel -j$(nproc)

# Результат: kernel-${MACHINE_ARCH}.bin
ls kernel-x86_64.bin
```

---

## 4. Создание загрузочного образа

### 4.1 Скрипт автоматической сборки образа

Сохраните как `make_limine_image.sh`:

```bash
#!/bin/bash
set -e

# Конфигурация
IMAGE="gergios-limine.img"
IMAGE_SIZE_MB=512
LIMINE_DIR="${LIMINE_DIR:-./limine}"
BOOT_DIR="./gergios_boot"
KERNEL="${BOOT_DIR}/boot/kernel"
MODULES_DIR="${BOOT_DIR}/boot/modules"

# Цветной вывод
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
info()  { echo -e "${GREEN}[INFO]${NC} $1"; }
warn()  { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# Проверки
[ -f "$KERNEL" ] || error "Kernel not found at $KERNEL"
[ -d "$LIMINE_DIR" ] || error "Limine directory not found at $LIMINE_DIR"
[ -f "$LIMINE_DIR/limine" ] || error "limine tool not found (run 'make' in limine dir)"
command -v qemu-img >/dev/null || error "qemu-img not found"
command -v parted >/dev/null || error "parted not found"
command -v mkfs.vfat >/dev/null || error "mkfs.vfat not found (install dosfstools)"

# =========================================================================
# Шаг 1: Создание пустого образа
# =========================================================================
info "Creating disk image: ${IMAGE} (${IMAGE_SIZE_MB}MB)"
rm -f "$IMAGE"
qemu-img create -f raw "$IMAGE" "${IMAGE_SIZE_MB}M"

# =========================================================================
# Шаг 2: Разметка GPT с ESP
# =========================================================================
info "Creating GPT partition table with EFI System Partition"
parted -s "$IMAGE" mklabel gpt
parted -s "$IMAGE" mkpart primary fat32 2048s 100%
parted -s "$IMAGE" set 1 esp on

# Определяем смещение ESP в секторах (2048) и размер
ESP_OFFSET=$((2048 * 512))  # в байтах
ESP_SIZE=$(( (IMAGE_SIZE_MB * 1024 * 1024) - ESP_OFFSET ))

# =========================================================================
# Шаг 3: Форматирование ESP (FAT32)
# =========================================================================
info "Formatting ESP as FAT32"
# Mount образ как loop device
LOOP_DEV=$(sudo losetup --show -f -P "$IMAGE")
echo "Loop device: ${LOOP_DEV}"

# ESP это первый раздел (${LOOP_DEV}p1)
sudo mkfs.vfat -F 32 -n "GERGIOS" "${LOOP_DEV}p1"

# =========================================================================
# Шаг 4: Копирование файлов
# =========================================================================
info "Copying boot files to ESP"
MOUNT_DIR=$(mktemp -d)
sudo mount "${LOOP_DEV}p1" "$MOUNT_DIR"

# Структура директорий на ESP
sudo mkdir -p "$MOUNT_DIR"/EFI/BOOT
sudo mkdir -p "$MOUNT_DIR"/boot/modules

# Limine UEFI бинарник
if [ -f "$LIMINE_DIR/bin/BOOTX64.EFI" ]; then
    sudo cp "$LIMINE_DIR/bin/BOOTX64.EFI" "$MOUNT_DIR/EFI/BOOT/BOOTX64.EFI"
elif [ -f "$LIMINE_DIR/BOOTX64.EFI" ]; then
    sudo cp "$LIMINE_DIR/BOOTX64.EFI" "$MOUNT_DIR/EFI/BOOT/BOOTX64.EFI"
else
    warn "BOOTX64.EFI not found — building from source may be needed"
    warn "Trying to find limine.efi..."
    find "$LIMINE_DIR" -name "*.EFI" -o -name "*.efi" 2>/dev/null | head -5
fi

# Limine stage 2 для BIOS режима
if [ -f "$LIMINE_DIR/bin/limine-bios.sys" ]; then
    sudo cp "$LIMINE_DIR/bin/limine-bios.sys" "$MOUNT_DIR/"
elif [ -f "$LIMINE_DIR/limine-bios.sys" ]; then
    sudo cp "$LIMINE_DIR/limine-bios.sys" "$MOUNT_DIR/"
fi

# Kernel
sudo cp "$KERNEL" "$MOUNT_DIR/boot/kernel"

# Модули
if [ -d "$MODULES_DIR" ]; then
    for mod in "$MODULES_DIR"/*; do
        [ -f "$mod" ] && sudo cp "$mod" "$MOUNT_DIR/boot/modules/"
    done
fi

# Limine config
sudo cp "${BOOT_DIR}/limine.conf" "$MOUNT_DIR/limine.conf" 2>/dev/null || \
    warn "limine.conf not found in boot dir, using default"

# =========================================================================
# Шаг 5: Дефолтный limine.conf если не скопирован
# =========================================================================
if [ ! -f "$MOUNT_DIR/limine.conf" ]; then
    info "Creating default limine.conf"
    sudo tee "$MOUNT_DIR/limine.conf" > /dev/null << 'EOF'
# Limine configuration for GergiOS (QEMU test)
TIMEOUT=10

:GergiOS 1.0 (UEFI/BIOS)
    PROTOCOL=limine
    KERNEL_PATH=boot:///boot/kernel
    MODULE_PATH=boot:///boot/modules/mod01_ds
    MODULE_PATH=boot:///boot/modules/mod02_rs
    MODULE_PATH=boot:///boot/modules/mod03_pm
    MODULE_PATH=boot:///boot/modules/mod04_sched
    MODULE_PATH=boot:///boot/modules/mod05_vfs
    MODULE_PATH=boot:///boot/modules/mod06_memory
    MODULE_PATH=boot:///boot/modules/mod07_tty
    MODULE_PATH=boot:///boot/modules/mod08_mib
    MODULE_PATH=boot:///boot/modules/mod09_vm
    MODULE_PATH=boot:///boot/modules/mod10_pfs
    MODULE_PATH=boot:///boot/modules/mod11_mfs
    MODULE_PATH=boot:///boot/modules/mod12_init
    CMDLINE=rootdevname=c0d0p0

:GergiOS 1.0 (Safe Mode)
    PROTOCOL=limine
    KERNEL_PATH=boot:///boot/kernel
    MODULE_PATH=boot:///boot/modules/mod01_ds
    MODULE_PATH=boot:///boot/modules/mod02_rs
    MODULE_PATH=boot:///boot/modules/mod03_pm
    MODULE_PATH=boot:///boot/modules/mod04_sched
    MODULE_PATH=boot:///boot/modules/mod05_vfs
    MODULE_PATH=boot:///boot/modules/mod06_memory
    MODULE_PATH=boot:///boot/modules/mod07_tty
    MODULE_PATH=boot:///boot/modules/mod08_mib
    MODULE_PATH=boot:///boot/modules/mod09_vm
    MODULE_PATH=boot:///boot/modules/mod10_pfs
    MODULE_PATH=boot:///boot/modules/mod11_mfs
    MODULE_PATH=boot:///boot/modules/mod12_init
    CMDLINE=rootdevname=c0d0p0 bootopts=-s
EOF
fi

# =========================================================================
# Шаг 6: Установка Limine (BIOS режим)
# =========================================================================
sudo umount "$MOUNT_DIR"
rmdir "$MOUNT_DIR"

info "Installing Limine (BIOS stage 1/2)"
sudo "$LIMINE_DIR/limine" bios-install "$IMAGE"

sudo losetup -d "$LOOP_DEV"

info "Image created: ${IMAGE}"
ls -lh "$IMAGE"
```

### 4.2 Запуск

```bash
chmod +x make_limine_image.sh
sudo ./make_limine_image.sh
```

---

## 5. Запуск в QEMU

### 5.1 BIOS (Legacy) режим

```bash
qemu-system-x86_64 \
    -machine q35 \
    -m 1G \
    -drive format=raw,file=gergios-limine.img \
    -serial stdio \
    -name "GergiOS (Limine BIOS)"
```

### 5.2 UEFI режим (OVMF)

```bash
# Установка OVMF (UEFI firmware для QEMU)
sudo apt install ovmf

# Запуск
qemu-system-x86_64 \
    -machine q35 \
    -m 1G \
    -bios /usr/share/ovmf/OVMF.fd \
    -drive format=raw,file=gergios-limine.img \
    -serial stdio \
    -name "GergiOS (Limine UEFI)"
```

### 5.3 UEFI + Secure Boot (для Phase 3)

```bash
# Требуется: подписанный BOOTX64.EFI + Enrolled MOK
qemu-system-x86_64 \
    -machine q35 \
    -m 1G \
    -bios /usr/share/ovmf/OVMF_CODE.secboot.fd \
    -drive format=raw,file=gergios-limine.img \
    -drive if=pflash,format=raw,readonly=on,file=/usr/share/ovmf/OVMF_VARS.fd \
    -serial stdio \
    -name "GergiOS (Limine Secure Boot)"
```

### 5.4 Отладка (GDB)

```bash
qemu-system-x86_64 \
    -machine q35 \
    -m 1G \
    -drive format=raw,file=gergios-limine.img \
    -serial stdio \
    -s -S \
    -name "GergiOS (debug)"

# В другом терминале:
gdb kernel-x86_64.bin \
    -ex "target remote :1234" \
    -ex "set architecture i386:x86-64" \
    -ex "break kmain" \
    -ex "continue"
```

### 5.5 UEFI режим на Windows

```powershell
# Установка OVMF через MSYS2 или скачать edk2-ovmf
# Путь к OVMF: C:\Program Files\qemu\share\ovmf\OVMF.fd

qemu-system-x86_64 ^
    -machine q35 ^
    -m 1G ^
    -bios "C:\Program Files\qemu\share\ovmf\OVMF.fd" ^
    -drive format=raw,file=gergios-limine.img ^
    -serial stdio ^
    -name "GergiOS (Limine UEFI)"
```

---

## 6. Проверка работоспособности

### 6.1 Что должно произойти при загрузке

```
1. QEMU запускает UEFI firmware (или BIOS)
2. Firmware находит Limine (BOOTX64.EFI или limine-bios.sys)
3. Limine читает limine.conf, показывает меню
4. Выбираем "GergiOS 1.0"
5. Limine загружает kernel + модули в память
6. Limine заполняет Limine protocol response структуры
7. Limine прыгает на ELF entry point (ENTRY(MINIX)) в 64-bit long mode
8. head.S: limine_pre_init() читает Limine responses
9. limine_pre_init() заполняет kinfo: модули, memory map
10. Настройка page tables → переход на high mapping
11. kmain() запускает серверы → GergiOS boot
```

### 6.2 Ожидаемый вывод в serial console

```
Limine v8.x  Copyright (C) 2023-2026 Limine Contributors
Booted with protocol: limine

GergiOS 1.0 (MINIX microkernel 3.4.0)
Copyright 2026, GergiOS Project
[... kernel boot messages ...]
limine: booted with 12 modules, 32 memmap entries
[... services starting ...]
GergiOS login:
```

### 6.3 Если что-то пошло не так

| Симптом | Причина | Решение |
|---------|---------|---------|
| QEMU показывает только "EFI Shell" | Limine не найден | Проверить BOOTX64.EFI в \EFI\BOOT\ |
| "No bootable device" | Неправильная GPT разметка | Проверить parted: тип раздела EFI (esp) |
| Limine не находит kernel | Неправильный KERNEL_PATH | Путь должен быть `boot:///boot/kernel` на ESP |
| Kernel загружается но виснет | Limine protocol не реализован | Проверить `#define USE_LIMINE` в head.S |
| Kernel падает с #GP | Неправильный entry mode | GRUB прыгает на multiboot_init, Limine на MINIX |
| "limine_pre_init not found" | limine.o не собран | Проверить unpaged object list в Makefile.inc |

---

## 7. Продвинутые сценарии

### 7.1 Сборка с MinGW (Windows cross)

```bash
# Cross-compilation для Windows (если собираете GergiOS на Linux)
sudo apt install mingw-w64

# Сборка Limine хост-инструментов для Windows
cd limine
make CC=x86_64-w64-mingw32-gcc
```

### 7.2 Тестирование без полного образа (through Limine + ramdisk)

```bash
# Limine может загрузить ELF напрямую через QEMU -kernel
# Но это не рекомендуется, т.к. Limine использует свой протокол

# Вместо этого используйте скрипт выше для создания полного образа
```

### 7.3 Автоматизация в CI/CD

```yaml
# .github/workflows/limine-test.yml (пример)
jobs:
  limine-test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install dependencies
        run: sudo apt install qemu-system-x86 ovmf dosfstools parted
      - name: Build kernel
        run: make kernel-x86_64 USE_LIMINE=1
      - name: Create boot image
        run: sudo ./make_limine_image.sh
      - name: Boot test (30 second timeout)
        run: |
          timeout 30 qemu-system-x86_64 \
            -machine q35 -m 1G \
            -bios /usr/share/ovmf/OVMF.fd \
            -drive format=raw,file=gergios-limine.img \
            -serial file:boot.log \
            -display none -no-reboot
          grep "GergiOS login:" boot.log && echo "BOOT SUCCESS" || echo "BOOT FAILED"
```

---

## 8. Структура файлов на ESP

```
ESP (FAT32)
├── EFI/
│   └── BOOT/
│       └── BOOTX64.EFI          ← Limine UEFI bootloader
├── limine-bios.sys               ← Limine stage 2 (BIOS mode)
├── limine.conf                   ← Boot configuration
├── boot/
│   ├── kernel                    ← ELF64 kernel
│   └── modules/
│       ├── mod01_ds              ← Data Store server
│       ├── mod02_rs              ← Reincarnation Server
│       ├── mod03_pm              ← Process Manager
│       ├── mod04_sched           ← Scheduler
│       ├── mod05_vfs             ← Virtual File System
│       ├── mod06_memory          ← Memory driver
│       ├── mod07_tty             ← TTY driver
│       ├── mod08_mib             ← MIB service
│       ├── mod09_vm              ← Virtual Memory
│       ├── mod10_pfs             ← PFS
│       ├── mod11_mfs             ← MFS
│       └── mod12_init            ← /sbin/init
```

---

## 9. Ссылки

- [Limine GitHub](https://github.com/limine-bootloader/limine)
- [Limine Protocol Specification](https://github.com/limine-bootloader/limine-protocol)
- [Arch Wiki: Limine](https://wiki.archlinux.org/title/Limine)
- `planning/16_bootloader_modernization.md` — Bootloader migration plan
- `docs/limine.h` — Limine protocol header for GergiOS kernel
- `minix/kernel/arch/x86_64/limine.c` — Limine protocol parser
- `minix/kernel/arch/x86_64/head.S` — Dual-boot entry point
