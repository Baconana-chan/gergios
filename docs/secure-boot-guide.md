# GergiOS Secure Boot Guide

> **Phase 3**: Подпись загрузчика + MOK enrollment
> **Related**: `planning/16_bootloader_modernization.md` §4.4
> **Requires**: Phase 2 (Limine UEFI), `sbsigntools`, `openssl`

---

## 1. Overview

Secure Boot ensures that only cryptographically signed binaries execute during boot.
For GergiOS, the chain is:

```
UEFI Firmware → signed BOOTX64.EFI (Limine) → Limine protocol → kernel + modules
                     ↑                            ↑
                sbsign with                   Optional: BLAKE2B
                custom key                    checksums in limine.conf
```

Two enrollment options:

| Option | Description | Best for |
|--------|-------------|----------|
| **MOK** (Machine Owner Key) | User-level key, managed via `mokutil` + MokManager | Development, dual-boot |
| **Direct firmware** | Key enrolled in firmware `db` via `cert-to-efi-sig-list` | Production, dedicated hardware |

**Recommendation for development**: MOK — safer, no risk of bricking.

---

## 2. Prerequisites

Install tools on the build host (Linux):

```bash
# Debian/Ubuntu
sudo apt install sbsigntool openssl efitools

# Arch Linux
sudo pacman -S sbsigntools openssl efitools

# Fedora
sudo dnf install sbsigntools openssl efitools
```

---

## 3. Generate Signing Keys

```bash
# From project root
./releasetools/gen_secure_boot_keys.sh

# Or specify output directory
./releasetools/gen_secure_boot_keys.sh /path/to/my-keys
```

This creates:

| File | Format | Purpose |
|------|--------|---------|
| `signing.key` | PEM RSA 2048 | Private key (keep safe!) |
| `signing.pem` | PEM X.509 | Certificate for `sbsign --cert` |
| `signing.der` | DER X.509 | For `mokutil --import` |
| `signing.esl` | EFI Sig List | For firmware `db` enrollment |

**Security**: The signing key signs BOOTX64.EFI. Anyone with this key can sign
binaries that will boot with Secure Boot enabled. Keep it safe.

---

## 4. Build a Signed Boot Image

### Option A: Automatic (via x86_hdimage.sh)

```bash
# Generate keys first
./releasetools/gen_secure_boot_keys.sh

# Build with Limine + signing
MKLIMINE=yes EFI_SIZE=$((64*1024*1024)) \
    SB_KEY_DIR=./secure-boot-keys \
    ./releasetools/x86_hdimage.sh
```

The script will:
1. Detect `sbsign`, `sbverify`, `openssl`
2. Locate signing keys (from `SB_KEY_DIR` or auto-detect)
3. Build Limine ESP with kernel + modules
4. **Sign** `EFI/BOOT/BOOTX64.EFI` with `sbsign`
5. Copy `gergios-sb.der` to ESP root for MOK enrollment

### Option B: Manual Signing

```bash
# Sign the Limine bootloader
sbsign --key secure-boot-keys/signing.key \
       --cert secure-boot-keys/signing.pem \
       --output BOOTX64.signed.efi \
       /usr/share/limine/BOOTX64.EFI

# Verify the signature
sbverify --cert secure-boot-keys/signing.pem BOOTX64.signed.efi

# Expected output:
# Signature verification OK
```

---

## 5. Enroll the Key

### Option 1: MOK (Recommended for Development)

```bash
# Import the DER certificate into MOK database
sudo mokutil --import secure-boot-keys/signing.der

# You'll be prompted for a temporary password.
# Reboot the machine — MokManager will appear at boot.
```

**MokManager enrollment steps**:
1. System reboots → MokManager UI appears automatically
2. Select **"Enroll MOK"**
3. Select **"Continue"**
4. Enter the temporary password you set with `mokutil`
5. Select **"Yes"** to enroll the key
6. System continues booting

After enrollment, Secure Boot will accept your signed `BOOTX64.EFI`.

### Option 2: Direct Firmware Enrollment (Production)

```bash
# Convert PEM certificate to EFI Signature List
cert-to-efi-sig-list -g $(uuidgen) signing.pem signing.esl

# Sign the ESL with your KEK (if you have custom PK/KEK)
sign-efi-sig-list -a -k KEK.key -c KEK.crt db signing.esl signing.auth

# Copy to FAT32 USB and enroll via firmware UI
cp signing.esl /media/usb/
```

Then reboot, enter firmware setup (F2/F10/Del), navigate to:
**Secure Boot → Key Management → Enroll db key** and select `signing.esl`.

---

## 6. Testing in QEMU

QEMU with OVMF can emulate Secure Boot for testing:

```bash
# Build signed image
MKLIMINE=yes EFI_SIZE=$((64*1024*1024)) \
    SB_KEY_DIR=./secure-boot-keys \
    ./releasetools/x86_hdimage.sh

# Boot with Secure Boot enabled
qemu-system-x86_64 -m 256M \
    -drive file=minix_x86.img,if=ide,format=raw \
    -bios /usr/share/ovmf/OVMF_CODE.fd \
    -global driver=cfi.pflash01,property=secure,value=on \
    -serial stdio
```

**Note**: QEMU's OVMF doesn't enforce Secure Boot by default unless
you enroll keys into its NVRAM. For full Secure Boot testing:

```bash
# Create a copy of OVMF vars with enrolled key
cp /usr/share/ovmf/OVMF_VARS.fd my-ovmf-vars.fd

# Run QEMU with custom vars
qemu-system-x86_64 -m 256M \
    -drive file=minix_x86.img,if=ide,format=raw \
    -drive if=pflash,format=raw,readonly=on,file=/usr/share/ovmf/OVMF_CODE.fd \
    -drive if=pflash,format=raw,file=my-ovmf-vars.fd \
    -serial stdio
```

Then within the guest, enroll the key using MokManager or by booting
an enrollment ISO.

---

## 7. CI/CD Integration

Add to your CI pipeline (GitHub Actions, etc.):

```yaml
- name: Generate Secure Boot keys
  run: |
    ./releasetools/gen_secure_boot_keys.sh

- name: Build signed image
  env:
    MKLIMINE: "yes"
    EFI_SIZE: "67108864"
    SB_KEY_DIR: "./secure-boot-keys"
  run: |
    ./releasetools/x86_hdimage.sh

- name: Verify signature
  run: |
    sbverify --cert secure-boot-keys/signing.pem \
      $OBJ/efi/EFI/BOOT/BOOTX64.EFI
```

**Key management in CI**:
- Use GitHub Secrets or equivalent for the signing key
- Generate keys once per release cycle, not per commit
- Store the private key (`signing.key`) in encrypted storage

---

## 8. Troubleshooting

| Problem | Cause | Solution |
|---------|-------|----------|
| `sbsign: command not found` | sbsigntools not installed | `sudo apt install sbsigntool` |
| `Failed to open file: Permission denied` | Can't read private key | `chmod 600 signing.key` |
| `No signature table` | Binary not PE format | Only .efi files can be signed |
| `Secure boot violation` on boot | Key not enrolled | Run `mokutil --import` and reboot |
| MokManager doesn't appear | Shim not installed | Boot once with Secure Boot disabled |
| `cert-to-efi-sig-list not found` | efitools not installed | `sudo apt install efitools` |
| QEMU doesn't enforce Secure Boot | OVMF vars without keys | Enroll key via firmware UI or `sbctl` |

### Check Secure Boot Status

```bash
# On the running GergiOS system (if EFI runtime services available)
# Or from a Linux host:
mokutil --sb-state        # enabled/disabled
mokutil --list-enrolled   # list enrolled MOK keys
bootctl status            # comprehensive EFI status
```

---

## 9. Key Management Best Practices

1. **Development vs Production keys**: Use separate keys for dev and prod
2. **Key expiration**: Keys are valid for 10 years (3650 days)
3. **Backup**: Keep a backup of `signing.key` offline
4. **Revocation**: To revoke a key, enroll a new one and remove the old
   from MOK database via `mokutil --delete`
5. **Rotation**: Generate new keys for each major release

---

## 10. Reference

- [Limine Boot Protocol Documentation](https://github.com/limine-bootloader/limine)
- [sbsigntools Manual](https://man.archlinux.org/man/sbsign.1.en)
- [Ubuntu Secure Boot Signing](https://wiki.ubuntu.com/UEFI/SecureBoot/Signing)
- [Arch Wiki: Unified Extensible Firmware Interface](https://wiki.archlinux.org/title/Unified_Extensible_Firmware_Interface#Secure_Boot)
- [sbctl — Simplified Secure Boot](https://github.com/foxboron/sbctl)
