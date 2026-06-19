#!/usr/bin/env bash
#
# gen_secure_boot_keys.sh — Generate development Secure Boot keys for GergiOS
#
# Creates:
#   signing.key   — RSA 2048 private key (PEM)
#   signing.pem   — X.509 certificate (PEM, for sbsign --cert)
#   signing.der   — X.509 certificate (DER, for mokutil --import)
#   signing.esl   — EFI Signature List (for firmware enrollment)
#
# Usage:
#   ./releasetools/gen_secure_boot_keys.sh [output_dir]
#
# Default output: ./secure-boot-keys/
#

set -euo pipefail

OUT_DIR="${1:-secure-boot-keys}"

# Safety check: don't silently overwrite existing keys
if [ -f "${OUT_DIR}/signing.key" ] || [ -f "${OUT_DIR}/signing.pem" ]; then
	echo "WARNING: Keys already exist in ${OUT_DIR}"
	echo "Running this script will OVERWRITE them."
	echo "Use a different output directory or backup the existing keys."
	echo ""
	read -r -p "Overwrite existing keys? [y/N] " REPLY
	case "${REPLY}" in
		[Yy]|[Yy][Ee][Ss])
			echo "Overwriting..."
			;;
		*)
			echo "Aborted."
			exit 1
			;;
	esac
fi

mkdir -p "${OUT_DIR}"

# Generate GUID for ESL (use a fixed one for reproducibility, or random)
GUID="${GUID:-$(uuidgen 2>/dev/null || echo "77fa9abd-9d5f-4b4c-8f5e-8e7f9a1b2c3d")}"

echo "=== GergiOS Secure Boot Key Generation ==="
echo "Output: ${OUT_DIR}"
echo "GUID:   ${GUID}"
echo ""

# 1. Generate RSA 2048 private key
echo " [1/4] Generating RSA 2048 signing key..."
openssl genrsa -out "${OUT_DIR}/signing.key" 2048
chmod 600 "${OUT_DIR}/signing.key"

# 2. Generate X.509 certificate (10 years validity)
echo " [2/4] Generating X.509 certificate..."
openssl req -new -x509 -sha256 \
    -key "${OUT_DIR}/signing.key" \
    -out "${OUT_DIR}/signing.pem" \
    -days 3650 \
    -subj "/O=GergiOS/CN=GergiOS Secure Boot Key/" \
    -addext "keyUsage=digitalSignature" \
    -addext "extendedKeyUsage=codeSigning" \
    -addext "basicConstraints=critical,CA:FALSE"

# 3. Convert to DER format (for mokutil --import)
echo " [3/4] Converting to DER format..."
openssl x509 -in "${OUT_DIR}/signing.pem" \
    -out "${OUT_DIR}/signing.der" -outform DER

# 4. Create EFI Signature List (for firmware enrollment via cert-to-efi-sig-list)
echo " [4/4] Creating EFI Signature List..."
if command -v cert-to-efi-sig-list &>/dev/null; then
    cert-to-efi-sig-list -g "${GUID}" \
        "${OUT_DIR}/signing.pem" \
        "${OUT_DIR}/signing.esl"
    echo "      ESL created: ${OUT_DIR}/signing.esl"
else
    echo "      cert-to-efi-sig-list not found (install efitools package)"
    echo "      ESL file not created — mokutil enrollment still works with .der"
fi

echo ""
echo "=== Done ==="
echo ""
echo "Files created in ${OUT_DIR}:"
ls -la "${OUT_DIR}/"
echo ""
echo "=== How to use ==="
echo ""
echo "1. Sign a UEFI binary:"
echo "   sbsign --key ${OUT_DIR}/signing.key --cert ${OUT_DIR}/signing.pem \\"
echo "         --output file.signed.efi file.efi"
echo ""
echo "2. Verify a signature:"
echo "   sbverify --cert ${OUT_DIR}/signing.pem file.signed.efi"
echo ""
echo "3. Enroll key via MOK (recommended):"
echo "   sudo mokutil --import ${OUT_DIR}/signing.der"
echo "   # Reboot and follow MokManager prompts"
echo ""
echo "4. Enroll key via firmware (if MOK unavailable):"
echo "   Copy signing.esl (or signing.der) to FAT32 USB"
echo "   Enter firmware setup → Secure Boot → Enroll db key"
echo ""
echo "5. Check Secure Boot status:"
echo "   mokutil --sb-state"
