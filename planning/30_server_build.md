# GergiOS Server Edition — VPS-ориентированная сборка

> **Статус**: 🔮 Pre-planning / Concept
> **Связанные**: `planning/01_microkernel_architecture.md`, `planning/25_network_stack_modernization.md`,
>   `planning/26_security_model_modernization.md`, `planning/29_admin_shell.md`
> **Репозитории**: `releasetools/`, `minix/kernel/`, `etc/`, `sbin/`

---

## 1. Executive Summary

### 1.1 Зачем server edition?

Текущая сборка GergiOS — **user/desktop-oriented**: игры, man-страницы, компилятор, тесты,
графический стек. Для VPS это избыточно: 500MB+ лишнего кода, больше поверхности атаки,
дольше сборка.

**Server Edition** — минимальная, оптимизированная сборка для VPS и bare-metal серверов.
Цель: сделать GergiOS привлекательным выбором для хостинг-провайдеров и их клиентов.

### 1.2 Что делает GergiOS Server уникальным (vs Linux)

| Аспект | Linux VPS | GergiOS Server |
|--------|-----------|----------------|
| **Безопасность** | SELinux/AppArmor (сложно) | MAC + Capabilities + Audit **из коробки** |
| **Изоляция** | cgroups/namespaces | **Микроядро**: каждый драйвер/сервис — отдельный процесс |
| **Live Update** | ksplice (проприетарный) | **Встроенный**: RS restart без перезагрузки |
| **Сетевая безопасность** | +IPsec/WireGuard (настраивать) | IPsec + WireGuard + DTLS + SYN cookies **уже в lwIP** |
| **Audit** | auditd (настраивать) | auditd + macd + auditctl **уже работают** |
| **Размер** | 500MB-2GB+ | **~100MB** для минимальной сборки |
| **Время загрузки** | 10-30 сек | **<1 секунда** до готовности сети |
| **Драйверы пользователя** | DKMS (сложно) | **Любой юзер может написать драйвер** (безопасно) |

---

## 2. Целевая аудитория

### 2.1 Кому это нужно

| Роль | Потребность | Что даёт GergiOS |
|------|-------------|------------------|
| **VPS провайдер** | Минимальная поверхность атаки, изоляция клиентов | Микроядро: драйвер клиента не может уронить весь сервер |
| **DevOps инженер** | Быстрая установка, автоматизация | Unattended install + cloud-init API |
| **Security-ориентированный пользователь** | MAC по умолчанию, аудит | Всё включено из коробки |
| **Edge/IoT хостинг** | Малый размер, быстрый старт | ~100MB, <1s загрузка |
| **VPN/WireGuard хост** | Крипто-ускорение, изоляция | WireGuard + IPsec в lwIP, микроядро |

### 2.2 Use cases

| Use case | Почему GergiOS | Linux альтернатива |
|----------|----------------|-------------------|
| **WireGuard VPN endpoint** | WireGuard в lwIP, микроядро — нет лишнего кода | WireGuard + iptables + selinux |
| **DNS resolver** (Unbound/Bind) | Изоляция сетевого стека, audit | chroot/selinux |
| **Web server** (nginx/h2o) | Live update без перезагрузки | nginx reload + kubernetes |
| **IoT gateway** | <1s boot, малый размер, Rust драйверы | Yocto/buildroot |
| **Security monitoring** | auditd + MAC + capabilities по умолчанию | SELinux + auditd + config |
| **VPN provider node** | WireGuard + IPsec + DTLS в одном стеке | StrongSwan + WireGuard |

---

## 3. Архитектура Server Edition

### 3.1 Компоненты

```
┌─────────────────────────────────────────────────────┐
│                  GergiOS Server                       │
│                                                       │
│  ┌──────────────────────────────────────────────┐   │
│  │ CORE (обязательно)                           │   │
│  │  • Микроядро (minix_base)                    │   │
│  │  • init + rc.d + базовые сервисы              │   │
│  │  • lwIP сетевой стек (WireGuard + IPsec)      │   │
│  │  • auditd + macd (безопасность по умолчанию) │   │
│  │  • minix-admin (управление)                  │   │
│  │  • sshd (единственный вход)                   │   │
│  └──────────────────────────────────────────────┘   │
│                                                       │
│  ┌──────────────────────────────────────────────┐   │
│  │ СЕРВИСЫ (опционально через pkgin)            │   │
│  │  • nginx / h2o                               │   │
│  │  • Unbound / Bind                            │   │
│  │  • PostgreSQL / SQLite                        │   │
│  │  • Prometheus node_exporter                   │   │
│  │  • gergios-admin web dashboard                 │   │
│  └──────────────────────────────────────────────┘   │
│                                                       │
│  ┌──────────────────────────────────────────────┐   │
│  │ НЕ ВКЛЮЧЕНО                                  │   │
│  │  • X11 / Wayland                             │   │
│  │  • games                                     │   │
│  │  • man-страницы                              │   │
│  │  • компилятор/заголовки                      │   │
│  │  • тесты                                     │   │
│  │  • framebuffer / звук                        │   │
│  └──────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────┘
```

### 3.2 Kernel config (SERVER profile)

```makefile
# sys/arch/x86_64/conf/SERVER

include "std.minix"

# ── Архитектура ──
ident          GERGIOS-SERVER
makeoptions    CPUFLAGS="-march=x86-64-v2"

# ── Ядро ──
# Микроядро — обязательный минимум
no options E1000              # Только virtio для VPS
no options RTL8139             
no options FXP                 
no options LANCE               
no options DP8390              

# ── Безопасность (включено) ──
options        USE_CAPCTL     # Capability system
options        USE_MAC        # MAC framework
options        USE_AUDIT      # Audit system
options        KASLR          # Address randomization
options        ENFORCE_WX     # W^X enforcement

# ── Сеть (включено) ──
options        LWIP_IPSEC     # IPsec ESP/AH
options        LWIP_WIREGUARD # WireGuard в lwIP
options        LWIP_DTLS      # DTLS over UDP
options        LWIP_SYN_COOKIES # SYN flood protection
options        LWIP_INGRESS_FILTER # BCP 38

# ── НЕ включено ──
no options USE_APM            # Power management не нужен
no options USE_SOUND          # Звук не нужен
no options USE_MOUSE          # Мышь не нужна
no options USE_FB             # Framebuffer не нужен
no options USE_PCI            # VPS обычно только virtio
```

### 3.3 SETS configuration

```bash
# Для server сборки:
SETS="minix-base"           # Только базовый набор

# Размеры:
# minix-base:     ~80MB
# minix-comp:     ~200MB (НЕ включён)
# minix-games:    ~20MB  (НЕ включён)
# minix-man:      ~50MB  (НЕ включён)
# minix-tests:    ~30MB  (НЕ включён)
# ─────────────────────
# Server total:   ~80MB  (kernel + base system)
# С пакетами:     ~150MB (+ nginx + unbound + sqlite)
```

---

## 4. Почему VPS провайдер выберет GergiOS

### 4.1 Безопасность из коробки (zero-config)

| Фича | Linux (нужно настраивать) | GergiOS Server |
|------|--------------------------|----------------|
| **MAC** | SELinux — сложные политики, многие выключают | **macd** — `allow ALL` по умолчанию, `deny ALL = 1 команда` |
| **Аудит** | auditd + rules + parsing | **auditd** работает, `auditctl -s`, `audit2txt`, audit в логе |
| **Capabilities** | systemd + ambiant sets | **cap_get_proc/set_proc** + наследование на fork/exec |
| **W^X** | SELinux + EXECMEM | **mmap W\|X = EPERM**, NX bit в PTE, mprotect |
| **KASLR** | Есть | **3-phase KASLR** + физический + PIE |
| **SYN flood** | iptables + synproxy | **SYN cookies в lwIP** (включено) |
| **IPsec/WireGuard** | strongSwan + WireGuard | **В lwIP** — единый стек |

### 4.2 Микроядро — изоляция без контейнеров

```text
Linux:                            GergiOS:
┌──────────┐                     ┌──────────┐
│  nginx   │  ← crash = падение  │  nginx   │  ← crash = RS перезапустит
├──────────┤   всего сервера      ├──────────┤
│  ядро    │                     │  ядро    │
│  (монолит)│                     │  (микро) │  ← ни один сервис не трогает ядро
├──────────┤                     ├──────────┤
│  драйверы │                     │  драйверы │ ← каждый в user-space
└──────────┘                     └──────────┘
```

Для VPS провайдера это означает:
- **Ошибка в драйвере клиента** → падает только клиентский драйвер, не вся нода
- **Ошибка в nginx** → RS перезапускает, нода жива
- **No "root hole"** — даже как root, процесс ограничен MAC + caps

### 4.3 Live Update без kubernetes

```bash
# На Linux VPS:
# yum update kernel → reboot → 30 секунд downtime
# На GergiOS:
svrctl update lwip     # ← перезагрузка без даунтайма
svrctl update auditd   # ← auditd обновлён, соединения живы
```

Для VPS провайдера:
- Обновление безопасности **без перезагрузки**
- Нет downtime при kernel patch
- Live update встроен в RS (Reincarnation Server)

### 4.4 Размер и скорость

| Метрика | Linux VPS (Ubuntu 24.04) | GergiOS Server (target) |
|---------|--------------------------|------------------------|
| **Размер образа** | 1.2GB (cloud image) | **~100MB** |
| **RAM idle** | 150-300MB | **~10-20MB** |
| **Boot time** | 10-30 сек | **<1 сек** |
| **Attach surface** | ~15M строк кода в ядре | **~50K строк** (lwIP + микроядро) |
| **CVE surface (2024)** | 258 (kernel CVEs) | **~0** (research-grade kernel) |

---

## 5. Build features — что сделает GergiOS лучше Linux

### 5.1 Unattended install (первая загрузка VPS)

```bash
# Провайдер собирает образ с:
gergios-server.img

# Первая загрузка — cloud-init аналог:
# /etc/gergios-firstboot.conf
hostname: vm-42
ssh_key: "ssh-ed25519 AAA..."
users:
  - name: admin
    groups: wheel
    ssh_authorized_keys:
      - "ssh-ed25519 AAA..."
network:
  dhcp: true
  # или статика:
  # interfaces:
  #   eth0:
  #     address: 10.0.0.42/24
  #     gateway: 10.0.0.1
packages:
  - nginx
  - unbound
```

**Как работает**:
1. Образ загружается, init находит `/etc/gergios-firstboot.conf`
2. Настраивает hostname, сеть, SSH
3. Устанавливает пакеты через pkgin
4. Создаёт пользователя, удаляет конфиг
5. **Всё за <5 секунд**

**Что нужно реализовать**:

| Компонент | Статус | LOC | Описание |
|-----------|--------|-----|----------|
| `firstboot.c` | ❌ Нет | ~300 | Парсинг YAML/JSON конфига, выполнение шагов |
| init скрипт `/etc/rc.d/firstboot` | ❌ Нет | ~50 | Запуск firstboot при отсутствии `/var/db/firstboot.done` |
| sshd включён по умолчанию | 🔶 Комментарий | ~10 | Убрать `#` в `/etc/rc.conf` |
| dhcpcd по умолчанию | 🔶 Есть `netconf` | ~20 | Интеграция с firstboot |

### 5.2 Cloud-init API (MetaData)

Для VPS провайдеров критична интеграция с их API.

```bash
# Провайдер запускает VM с:
# -iso /var/lib/gergios/meta-data.iso
#
# ISO содержит:
# meta-data:
#   instance-id: vm-42
#   hostname: vm-42.example.com
#   public-keys:
#     - "ssh-ed25519 AAA..."
# user-data:
#   ssh_authorized_keys:
#     - "ssh-ed25519 AAA..."
#   write_files:
#     - path: /etc/nginx/nginx.conf
#       content: "..."

# GergiOS при загрузке монтирует ISO и выполняет meta-data
```

**Но**: MINIX не имеет ISO драйвера в user-space легко. 
**Альтернатива**: первый boot читает `/etc/cloud-config.conf` из корневой ФС.

### 5.3 Image Builder (клик-образы)

```bash
# Провайдер собирает образ для своего VPS:
gergios-build-image \
  --arch x86_64 \
  --kernel SERVER \
  --sets minix-base \
  --disk-size 2G \
  --output gergios-vps-2g.img

# Или с nginx:
gergios-build-image \
  --include nginx,unbound,sqlite \
  --firstboot /path/to/firstboot.conf \
  --output gergios-web.img
```

**Статус**: `releasetools/x86_hdimage.sh` уже создаёт .img. Нужна только обёртка.

| Компонент | Статус | LOC | Описание |
|-----------|--------|-----|----------|
| `gergios-build-image.sh` | ❌ Нет | ~200 | Обёртка над releasetools с выбором SETS/kernel |
| `gergios-build-image --include` | ❌ Нет | ~100 | Установка пакетов в образ через pkgin |
| `gergios-build-image --firstboot` | ❌ Нет | ~50 | Генерация firstboot.conf в образ |

### 5.4 Security hardened by default

```bash
# После первого boot — всё уже включено:
auditctl -s
# Enabled: yes
# MAC: enforcing
# Capabilities: restricted per service
# SYN cookies: enabled
# W^X: enforced
# IPsec available: yes
# WireGuard: yes
```

**Что отличает от Linux**:
- SELinux на 99% VPS **выключен** (админы его не понимают)
- AppArmor на Ubuntu VPS — profiles для nginx нет по умолчанию
- GergiOS: **всё включено**, но не мешает (permissive MAC = логи без блокировок)

### 5.5 Minimal resource consumption

```bash
# После загрузки GergiOS Server:
ps -xm | head -5
# PID  NAME          SIZE      RSS
#   0  kernel        4.2M     2.1M
#   1  init          0.5M     0.3M
#   2  rs            0.8M     0.4M
#   3  pm            0.6M     0.3M
#   4  vfs           2.1M     1.0M
#   5  lwip          3.0M     1.5M
#   6  auditd        0.4M     0.2M
#   7  sshd          0.8M     0.4M
# ──────────────────────────────
# TOTAL:             12.4M    5.2M
```

Сравнение с Linux VPS (Ubuntu Server, idle):

| Компонент | Linux | GergiOS |
|-----------|-------|---------|
| systemd | 30MB | **Нет** (init + rc.d) |
| Network stack | ~50MB (kernel + modules) | **~3MB** (lwIP service) |
| Audit | ~10MB (auditd + kernel) | **~0.4MB** |
| Total idle | 150-300MB | **~15-20MB** |

Для VPS провайдера: можно продавать **те же ресурсы дешевле**, или **больше клиентов на ноду**.

### 5.6 Rust ecosystem

- **Rust e1000 driver** — уже работает (TSO, CSO)
- **Rust BPF verifier** — 0 unsafe, 41 тест
- **Rust virtio-net pilot driver** — 11 тестов
- **Rust minix-audio, minix-blk** — driver framework

Для VPS: **virtio-net + virtio-blk на Rust** — безопасные драйверы, 0 RCE риска.

### 5.7 Per-VM audit без дополнительных затрат

```bash
# Каждая VM GergiOS автоматически логирует:
auditctl -s
# Total events: 12345
# Violations:   0
# Denied IPC:   3
# Service crashes: 0

# audit2txt — читаемый вывод:
[12:34:56]  AUTH_SUCCESS    subj=sshd  obj=root  OK
[12:35:01]  SERVICE_START   subj=rs    obj=nginx OK
[12:35:12]  FILE_DENIED     subj=nginx obj=/etc/shadow  EACCES
```

Для VPS провайдера:
- Мониторинг **каждой VM** без дополнительного ПО
- Compliance из коробки
- SOC готовые логи

### 5.8 KV/NoSQL в ядре

GergiOS имеет **DS (Data Store)** — in-memory key-value store:

```c
// Любой сервис может:
ds_publish("nginx.status", "running");
ds_retrieve("network.stats", &stats);
```

Потенциально:
- **Service discovery** без etcd/consul
- **Metrics** без Prometheus node exporter
- **Config** без consul

---

## 6. План реализации

### 6.1 Milestones

| Milestone | Что готово | LOC | Время |
|-----------|-----------|-----|-------|
| **M1** | Kernel profile SERVER + SETS конфиг | ~200 | 1 неделя |
| **M2** | gergios-build-image.sh (image builder) | ~300 | 1 неделя |
| **M3** | Unattended firstboot (cloud-init аналог) | ~400 | 2 недели |
| **M4** | Server packages (nginx, unbound, node_exporter) | ~500 | 2 недели |
| **M5** | Документация + reference deployment | ~500 | 1 неделя |
| **M6** | Performance benchmarks vs Linux VPS | ~200 | 1 неделя |

**Итого**: ~2100 LOC, ~8 недель

### 6.2 Структура репозитория

```
releasetools/
  server.image.defaults     — NEW: конфиг для server сборки
  gergios-build-image.sh    — NEW: image builder CLI
  x86_cdimage.sh            — MOD: поддержка SERVER kernel profile

etc/
  rc.d/
    firstboot               — NEW: firstboot скрипт
  gergios-firstboot.conf    — NEW: example конфиг

sbin/
  firstboot                  — NEW: firstboot C utility

sys/arch/x86_64/conf/
  SERVER                     — NEW: kernel profile for server
  std.minix                  — MOD: базовые опции

minix/usr.sbin/
  gergios-build-image/       — NEW: image builder (опционально Rust)

docs/
  server-deployment.md      — NEW: deployment guide
  server-vps-comparison.md  — NEW: сравнение с Linux
```

---

## 7. Что нужно от VPS провайдера

### 7.1 Для интеграции

| Компонент | Что нужно |
|-----------|-----------|
| **Образ** | RAW/ISO образ ~100MB |
| **Console** | Serial console (MINIX стандартно) |
| **Network** | DHCP (lwIP поддерживает) |
| **Block** | VirtIO / AHCI |
| **Boot** | GRUB / Limine / Multiboot |

### 7.2 Что GergiOS Server НЕ требует (в отличие от Linux)

- ❌ No systemd → no journald → no log rotation config
- ❌ No udev → no device manager overhead
- ❌ No NetworkManager → no dhclient overhead
- ❌ No cron → no need for anacron
- ❌ No selinux policy rebuild
- ❌ No apparmor profiles
- ❌ No polkit / dbus
- ❌ No 500MB initramfs
- ❌ No kernel modules loading at boot

---

## 8. Риски

| Риск | Impact | Вероятность | Mitigation |
|------|--------|-------------|------------|
| **VirtIO блок не работает** | High | Medium | AHCI fallback; тестировать на QEMU/KVM |
| **lwIP не выдерживает production нагрузку** | High | Low | lwIP 2.2.1 + TSO + multi-queue = 1-2 Gbps |
| **Нет готового nginx для MINIX** | Medium | Medium | Собрать из pkgsrc; или h2o/stunnel |
| **Отсутствие контейнеров (Docker)** | Medium | High | Микроядро + MAC = изоляция без контейнеров |
| **Провайдер не знает MINIX** | Medium | High | minix-admin + документация; знакомый SSH |
| **Нет cloud-init** | Low | Medium | Свой firstboot — проще и быстрее |
| **Нет Docker/Snap/Flatpak** | Low | Low | Не нужно для VPS use case |

---

## 9. План действий (первый шаг)

### Шаг 1: Создать SERVER kernel profile

Скопировать `std.minix` → выключить лишнее:

```bash
# sys/arch/x86_64/conf/SERVER
include "std.minix"
no options E1000
no options SOUND
no options FB
no options MOUSE
options KASLR
options USE_AUDIT
options LWIP_SYN_COOKIES
```

### Шаг 2: Создать `releasetools/server.image.defaults`

```bash
# На основе x86_hdimage.sh, но только minix-base
SETS="minix-base"
KERNEL="SERVER"
# Без игр, без компилятора, без man
```

### Шаг 3: Проверить сборку

```bash
./releasetools/cmake-build.sh configure x86_64
./releasetools/cmake-build.sh build
SETS="minix-base" KERNEL=SERVER ./releasetools/x86_hdimage.sh
```

---

## Приложение A: Сравнение image sizes

| Компонент | Linux (Ubuntu 24.04 Cloud) | GergiOS Server (target) |
|-----------|---------------------------|------------------------|
| Kernel | ~15MB (vmlinuz + modules) | ~3MB (kernel + unpaged) |
| Init system | ~30MB (systemd + udev) | ~1MB (init + rc.d) |
| Core utils | ~100MB (coreutils, bash, etc) | ~30MB (MINIX base) |
| Network stack | ~50MB (kernel net + modules) | ~3MB (lwIP service) |
| Security | ~10MB (auditd, apparmor) | ~0.5MB (auditd + macd) |
| SSH | ~5MB | ~0.5MB |
| Первый boot | ~50MB (cloud-init) | ~0.2MB (firstboot) |
| **Total** | **~500MB** (cloud image сжатый) | **~80MB** |

## Приложение B: GergiOS-x-Linux VPS feature matrix

| Feature | Linux VPS | GergiOS Server | Notes |
|---------|-----------|----------------|-------|
| Live kernel patching | ❌ (ksplice $) | ✅ RS live update | Встроено |
| Driver isolation | ❌ (monolithic) | ✅ (microkernel) | Драйвер в user-space |
| MAC by default | ❌ | ✅ (macd) | SELinux выключен на 99% VPS |
| Audit by default | ❌ | ✅ (auditd + auditctl) | auditd часто не включён |
| WireGuard in stack | ❌ (внешний модуль) | ✅ (lwIP) | Без лишнего модуля |
| IPsec in stack | ❌ (strongSwan) | ✅ (lwIP) | В lwIP |
| Size | 500MB+ | ~80MB | |
| Boot time | 10-30s | <1s | |
| Idle RAM | 150-300MB | ~15-20MB | |
| Userspace drivers | ❌ (kernel only) | ✅ | Любой может написать |
| Capability inheritance | ✅ | ✅ | GergiOS с Phase 2 |
| KASLR | ✅ | ✅ (3-phase PIE) | |
| Rust drivers | ❌ (RFL WIP) | ✅ (e1000, virtio-net) | Уже работает |
