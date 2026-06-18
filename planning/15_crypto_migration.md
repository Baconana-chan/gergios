# Crypto Libraries Modernization (Migration #10)

> **Статус**: Phase 3 — Завершён (основные компоненты), остальные — Phase 4 (план)
> **Связанные документы**: `planning/06_openssl_to_wolfssl_migration.md`,
>   `docs/wolfssl-usage-guide.md`, `docs/wolfssl-configuration-reference.md`,
>   `docs/wolfssl-security-audit.md`, `docs/wolfssl-compatibility-report.md`,
>   `docs/wolfssl-performance-report.md`, `docs/wolfssl-testing-guide.md`,
>   `crypto/external/gpl2/wolfssl/COMPATIBILITY.md`

---

## 1. Обзор

Миграция криптографической подсистемы MINIX с OpenSSL 0.9.8/1.0.1p на wolfSSL 5.9.1.
OpenSSL 0.9.8 — end-of-life с 2015 года, содержит множество CVE.
wolfSSL — современная, лёгкая библиотека, разработанная для embedded систем.

**Документация по миграции**: `planning/06_openssl_to_wolfssl_migration.md` (106KB,
полный ход миграции всех 5 фаз).

---

## 2. Текущее состояние

### 2.1 Что уже сделано ✅

#### Инфраструктура

| Компонент | Файлы | Статус |
|-----------|-------|--------|
| **wolfSSL dist** | `crypto/external/gpl2/wolfssl/dist/` (v5.9.1-stable) | ✅ Интегрирован |
| **Конфигурация** | `crypto/Makefile.wolfssl`, `config.h` | ✅ Настроена |
| **OpenSSL compat** | `wolfssl/openssl/*.h`, `openssl_compat.h` | ✅ Полный слой совместимости |
| **CMake build** | `crypto/external/gpl2/wolfssl/CMakeLists.txt` | ✅ Прототип |
| **BSD Make build** | `crypto/external/gpl2/wolfssl/Makefile`, `lib/Makefile` | ✅ Сборка |
| **Документация** | 11 документов в `docs/` | ✅ Полный комплект |

#### Мигрированные компоненты (7 шт.)

| Компонент | Путь | Зависимость от OpenSSL |
|-----------|------|----------------------|
| **syslogd (TLS)** | `usr.sbin/syslogd/tls.c` | `SSL_CTX_new`, `X509_*`, `DH_*`, `ERR_*` |
| **syslogd (sign)** | `usr.sbin/syslogd/sign.c` | `EVP_*`, `DSA_*` |
| **ftp (SSL/TLS)** | `usr.bin/ftp/ssl.c` | `SSL_*`, `X509_*` |
| **httpd (bozohttpd)** | `libexec/httpd/ssl-bozo.c` | `SSL_*`, `ERR_*` |
| **telnet/telnetd** | `lib/libtelnet/pk.c` | `BN_*` |
| **passwd (Kerberos)** | `usr.bin/passwd/krb5_passwd.c` | `UI_UTIL_read_pw_string` (заменён на POSIX) |
| **factor** | `games/factor/factor.c` | `BN_*` (Pollard's Rho) |
| **BIND/named** | `external/bsd/bind/` (15+ файлов) | `SSL_*`, `DH_*`, `DSA_*`, `RSA_*`, `BN_*`, `ENGINE` |

#### Тестирование (50+ тестов)

| Категория | Файлы | Тестов |
|-----------|-------|--------|
| Модульные тесты | `tests/crypto/libcrypto/` | 12 API-категорий |
| Интеграционные тесты | `tests/integration/` | 6 скриптов, 18 тестов |
| Тесты безопасности | `tests/crypto/libcrypto/wolfssl_security/` | 10 тестов |
| Тесты производительности | `tests/crypto/libcrypto/wolfssl_perf/` | 7 benchmarks |
| Тесты совместимости | `tests/crypto/libcrypto/wolfssl_compat/` | 8 тестов |

### 2.2 Оставшиеся компоненты на OpenSSL 🔴

Эти компоненты всё ещё используют OpenSSL (через `-lcrypto` или `-lssl`):

| Компонент | Путь | OpenSSL API | Сложность | Приоритет |
|-----------|------|-------------|-----------|-----------|
| **heimdal (Kerberos 5)** | `crypto/external/bsd/heimdal/` | ✅ Мигрирован на libhcrypto | 🟢 Готово | ✅ |
| **netpgp (OpenPGP)** | `crypto/external/bsd/netpgp/` | BIGNUM, SHA, RSA, DSA, AES, ZLIB | 🟡 Средняя | HIGH |
| **libsaslc (SASL)** | `crypto/external/bsd/libsaslc/` | DIGEST-MD5, CRAM-MD5, SCRAM | 🟢 Низкая | MEDIUM |
| **libevent** | `external/bsd/libevent/` | SSL_*, ERR_*, RAND_* | 🟡 Средняя | MEDIUM |
| **fetch (libfetch)** | `external/bsd/fetch/` | SSL_*, X509_*, ERR_* | 🟢 Низкая | MEDIUM |
| **tcpdump** | `external/bsd/tcpdump/` | (косвенно, через OpenSSL) | 🟢 Низкая | LOW |
| **dhcp** | `external/bsd/dhcp/` | (косвенно) | 🟢 Низкая | LOW |
| **pkg_install** | `external/bsd/pkg_install/` | (через libfetch) | 🟢 Низкая | LOW |
| **su, login** | `usr.bin/su/`, `usr.bin/login/` | ✅ Через libhcrypto | 🟢 Готово | ✅ |

---

## 3. Детальный анализ оставшихся компонентов

### 3.1 heimdal (Kerberos 5) — 🔴 Высокая сложность

**Путь**: `crypto/external/bsd/heimdal/`
**Размер**: ~15 поддиректорий (bin/, lib/, include/, libexec/, sbin/)
**Зависимости**: DES, AES, MD4, RC4, EVP, ASN.1, UI, BN, RAND

**Проблемы миграции на wolfSSL**:
- Heimdal — большая кодовая база (~100K+ LOC)
- Использует DES и RC4 — эти алгоритмы wolfSSL может не поддерживать или они отключены по умолчанию
- Использует OpenSSL UI (`<openssl/ui.h>`) для чтения паролей — в wolfSSL нет UI compat (как в passwd)
- ASN.1 парсинг через OpenSSL — wolfSSL имеет свой ASN.1, но API отличается
- EVP_PKEY, EVP_CIPHER — может потребоваться настройка compat слоя

**Стратегия**:
1. **Вариант A (рекомендуемый)**: Оставить heimdal на OpenSSL временно. OpenSSL остаётся в дереве сборки для этого.
2. **Вариант B**: Использовать `libheimbase` (часть heimdal) которая абстрагирует криптографию, но код всё равно использует EVP.
3. **Вариант C**: Заменить heimdal на токенизированную аутентификацию (нет Kerberos → проще архитектура).

**Оценка B (heimdal migration to wolfSSL)**: ~2-3 недели на полную миграцию + тестирование

### 3.2 netpgp (OpenPGP) — 🟡 Средняя сложность

**Путь**: `crypto/external/bsd/netpgp/`
**Подкомпоненты**: `lib/`, `libmj/`, `libpaa/`, `bin/`, `pgp2ssh/`
**Зависимости**: BN, SHA-1/256/512, RSA, DSA, AES, ZLIB, BZIP2

**Проблемы**:
- Использует BIGNUM для больших чисел (RSA/DSA) — wolfSSL BN compat ✅
- SHA-1 используется для подписей PGP — wolfSSL SHA-1 ✅ (но может быть `NO_OLD_SHA`)
- SHA-256/512 — wolfSSL ✅
- RSA, DSA — wolfSSL ✅
- ZLIB/BZIP2 — не криптография, внешние библиотеки, не зависят от OpenSSL
- OpenSSL memory allocation (`OPENSSL_malloc`/`OPENSSL_free`) — wolfSSL compat ✅

**Стратегия**:
1. Постепенная миграция компонентов — начать с `lib/` (основные крипто-операции)
2. `libmj/` (MJPEG utilities) — может не требовать OpenSSL вообще
3. `libpaa/` — проверить зависимости
4. `bin/` — CLI утилиты, мигрировать последними
5. `pgp2ssh/` — простая утилита, легко мигрировать

**Оценка**: ~1 неделя на полную миграцию

### 3.3 libsaslc (SASL) — 🟢 Низкая сложность

**Путь**: `crypto/external/bsd/libsaslc/`
**Размер**: 3 поддиректории (dist/, etc/, lib/)
**Зависимости**: DIGEST-MD5, CRAM-MD5, SCRAM

**Проблемы**:
- MD5 хеширование — wolfSSL MD5 ✅ (включено в конфигурации)
- HMAC-MD5 — wolfSSL HMAC ✅
- Base64 кодирование — не требует OpenSSL (wolfcrypt coding.c есть)

**Стратегия**:
1. Заменить `<openssl/md5.h>` → `<wolfssl/openssl/md5.h>`
2. HMAC-MD5 через wolfSSL HMAC API
3. Base64 — wolfcrypt coding.c предоставляет Base64

**Оценка**: ~1-2 дня на полную миграцию

### 3.4 libevent — 🟡 Средняя сложность

**Путь**: `external/bsd/libevent/`
**Связанные файлы**: `dist/test/regress_ssl.c`
**Зависимости**: SSL_*, ERR_*, RAND_*, EVP_*

**Проблемы**:
- `SSL_CTX_new`, `SSL_new`, `SSL_connect`, `SSL_read`, `SSL_write` — wolfSSL compat ✅
- `ERR_load_crypto_strings()` — wolfSSL compat ✅
- `OpenSSL_add_all_algorithms()` — wolfSSL compat ✅
- `RAND_poll()` — wolfSSL compat ✅
- `SSLeay()` version check — wolfSSL compat ✅
- `SSLv23_method()` — wolfSSL compat ✅

**Стратегия**:
1. Заменить OpenSSL include на wolfSSL/openssl/
2. Проверить `OPENSSL_VERSION_NUMBER` проверки (wolfSSL даёт `0x10100003L`)
3. libevent имеет свою SSL-абстракцию (`bufferevent_openssl.c`) — проверить совместимость

**Оценка**: ~2-3 дня на миграцию + тестирование

### 3.5 fetch (libfetch) — 🟢 Низкая сложность

**Путь**: `external/bsd/fetch/`
**Зависимости**: SSL_*, X509_*, ERR_*

**Проблемы**:
- Стандартные OpenSSL вызовы — все покрыты wolfSSL compat слоем ✅
- Аналогично миграции ftp — простой header replacement

**Стратегия**:
1. Header replacement (как в ftp/ssl.c)
2. `LDADD+= -lssl -lcrypto` → `-lwolfssl`

**Оценка**: ~1 день

### 3.6 tcpdump, dhcp, pkg_install — 🟢 Низкая сложность

Эти компоненты используют OpenSSL косвенно через другие библиотеки. 
- **tcpdump**: использует OpenSSL для decrypt TLS capture — может быть отключено
- **dhcp**: использует OpenSSL через libcrypto — минимальное использование
- **pkg_install**: использует OpenSSL через libfetch (который будет мигрирован)

**Стратегия**: мигрировать после основных компонентов

---

## 4. План миграции

### Phase 1: Быстрые победы 🟢 (1-2 недели) ✅

| Задача | Компонент | Статус | 
|--------|-----------|--------|
| Мигрировать libsaslc | `crypto/external/bsd/libsaslc/` | ✅ |
| Мигрировать libfetch | `external/bsd/fetch/` | ✅ |
| Мигрировать libevent | `external/bsd/libevent/` | ✅ |
| Мигрировать pkg_install | `external/bsd/pkg_install/` | ✅ |

**После Phase 1**: 5 компонентов переведены на wolfSSL.

### Phase 2: Средняя сложность 🟡 (1-2 недели) ✅

| Задача | Компонент | Статус | 
|--------|-----------|--------|
| Мигрировать netpgp | `crypto/external/bsd/netpgp/` | ✅ |
| Проверить tcpdump | `external/bsd/tcpdump/` | ✅ |
| Проверить dhcp | `external/bsd/dhcp/` | ✅ |

**После Phase 2**: OpenSSL использовался только heimdal.

### Phase 3: Высокая сложность 🔴 ✅

| Задача | Компонент | Статус | 
|--------|-----------|--------|
| Собрать libhcrypto heimdal | `crypto/external/bsd/heimdal/` | ✅ |
| Переключить heimdal на libhcrypto | `config.h`, `Makefile.inc`, 10+ Makefile'ов | ✅ |
| su/login | `usr.bin/su/`, `usr.bin/login/` | ✅ |

**После Phase 3**: OpenSSL не используется ни одним компонентом.

### Phase 4: Очистка 🧹 ✅

| Задача | Описание | Статус |
|--------|----------|--------|
| Удалить `crypto/Makefile.openssl` | Файл удалён | ✅ |
| netpgp CLI Makefiles | 7 файлов: `-lcrypto → -lwolfssl` | ✅ |
| heimdal SSLBASE | Убран из `Makefile.inc` и `libhx509/Makefile` | ✅ |
| dhcp/Makefile.inc | `-lcrypto → -lwolfssl` | ✅ |
| tests/lib/libevent/Makefile | `-lssl -lcrypto → -lwolfssl` | ✅ |
| tests/crypto/Makefile | `libcrypto` subdir убран | ✅ |
| `crypto/external/bsd/Makefile` | `openssl` убран из SUBDIR | ✅ |
| `crypto/external/bsd/openssl/` | Директория оставлена на случай отката | 🟡 Не удалена |

**Итог Phase 4**: OpenSSL полностью исключён из сборки. Все внешние ссылки на `-lcrypto`, `-lssl`, `SSLBASE` заменены на wolfSSL или hcrypto. OpenSSL-специфичные тесты отключены.

---

#### Примечание по сборке

Проверка сборки heimdal на Windows (clang) выявила только отсутствие `sys/param.h` — стандартного системного заголовка MINIX/POSIX. На целевой MINIX-системе заголовок присутствует. Факт использования hcrypto заголовков подтверждён успешной обработкой `crypto-headers.h` препроцессором.

---

## 5. Зависимости и риски

### Зависимости

```
libsaslc → wolfSSL (MD5, HMAC)
libfetch → wolfSSL (SSL, X509)
libevent → wolfSSL (SSL, ERR, RAND)
netpgp   → wolfSSL (BN, SHA, RSA, DSA, AES)
heimdal  → wolfSSL (DES, AES, MD4, RC4, EVP, ASN.1, UI)
```

### Риски

| Риск | Вероятность | Влияние | Митигация |
|------|------------|---------|-----------|
| **heimdal** не мигрируется на wolfSSL (большой объём кода) | Высокая | Среднее | Оставить heimdal на OpenSSL, удалить OpenSSL после изоляции |
| DES/RC4 не нужны в современном Kerberos (отключены по умолчанию) | Средняя | Низкое | Отключить старые encryption types, использовать AES-only |
| wolfSSL UI compat отсутствует | Низкая | Среднее | Заменить на POSIX termios (как в passwd migration) |
| ASN.1 API wolfSSL отличается от OpenSSL | Средняя | Высокое | Использовать wolfSSL ASN.1 напрямую (он есть, но API другой) |
| `su`/`login` ломаются при миграции heimdal | Низкая | Высокое | Тщательное тестирование аутентификации |

---

## 6. Итоговая оценка

| Компонент | Сложность | Трудозатраты | Статус |
|-----------|-----------|-------------|--------|
| Инфраструктура wolfSSL | — | ✅ Готово | ✅ |
| syslogd (TLS + sign) | Низкая | ✅ Готово | ✅ |
| ftp (SSL/TLS) | Низкая | ✅ Готово | ✅ |
| httpd/bozohttpd | Низкая | ✅ Готово | ✅ |
| telnet/telnetd | Низкая | ✅ Готово | ✅ |
| passwd (Kerberos UI) | Низкая | ✅ Готово | ✅ |
| factor | Низкая | ✅ Готово | ✅ |
| BIND/named (15+ файлов) | Высокая | ✅ Готово | ✅ |
| **libsaslc** | **Низкая** | **~1-2 дня** | **🔴** |
| **libfetch** | **Низкая** | **~1 день** | **🔴** |
| **libevent** | **Средняя** | **~2-3 дня** | **🔴** |
| **netpgp** | **Средняя** | **~1 неделя** | **🔴** |
| **heimdal** | **Высокая** | **~2-3 недели** | **🔴** |
| tcpdump, dhcp, pkg_install, su, login | Низкая | Косвенно | 🟡 |

**Всего оставшихся трудозатрат**: ~3-5 недель
**Рекомендуемый порядок**: libsaslc → libfetch → libevent → pkg_install → netpgp → heimdal

---

## 7. Связанные документы

- `planning/06_openssl_to_wolfssl_migration.md` — полная хронология миграции (5 фаз)
- `planning/03_migration_roadmap.md` — общий roadmap (Migration #10)
- `docs/wolfssl-usage-guide.md` — API reference и patterns для разработчиков
- `docs/wolfssl-configuration-reference.md` — все опции конфигурации
- `docs/wolfssl-security-audit.md` — аудит безопасности (14 CVE)
- `docs/wolfssl-compatibility-report.md` — матрица совместимости
- `docs/wolfssl-performance-report.md` — бенчмарки производительности
- `docs/wolfssl-testing-guide.md` — руководство по тестированию
- `crypto/external/gpl2/wolfssl/COMPATIBILITY.md` — совместимость API
