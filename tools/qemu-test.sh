#!/bin/sh
# SPDX-License-Identifier: MIT OR Apache-2.0
# Boot the UEFI loader under QEMU + OVMF and check the menu, the hold key, the fallback, boot counting and chain loading.
#   qemu-test.sh [--arch x86_64|aarch64] [--only a|a2|b|c|d|e|f|g|h|i|j|k|l] [--keep] [--lean-image IMAGE.img]
#                [--real-kernel FILE --real-initrd FILE [--real-cmdline WORDS]]
# Settings (environment; the defaults are relative to this repository):
#   CARGO_TARGET_DIR   where `cargo build --release --target x86_64-unknown-uefi -p boot-uefi -p test-efi` put its output
#   BOOT_EFI, TEST_EFI the two programs (default: under CARGO_TARGET_DIR, else ../target)
#   OVMF_CODE, OVMF_VARS  firmware (default /usr/share/OVMF/OVMF_CODE_4M.fd and OVMF_VARS_4M.fd; with --arch aarch64
#                      /usr/share/AAVMF/AAVMF_CODE.fd and AAVMF_VARS.fd, and the programs come from target/aarch64-unknown-uefi)
#   --arch aarch64     the same tests under `qemu-system-aarch64 -M virt` (TCG) with the aarch64 build; build it with
#                      `cargo build --release --target aarch64-unknown-uefi -p boot-uefi -p test-efi`
#   BOOT_SCRATCH       directory for the throwaway disks (default: $XDG_CACHE_HOME or ~/.cache, then boot-loader-test)
#   LEAN_IMAGE         a disk image whose first partition has /boot/bzImage and /boot/initrd-lean, for the real-kernel test
#                      (read only; that test is skipped without it)
#   INITRD_MARKER      regex of a line that init in that initrd prints (checked after the kernel runs /init)
# Only files in the scratch directory are written; no disk or device is touched.
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/.." && pwd)
TARGET=${CARGO_TARGET_DIR:-$REPO/target}
SCRATCH=${BOOT_SCRATCH:-${XDG_CACHE_HOME:-$HOME/.cache}/boot-loader-test}
LEAN=${LEAN_IMAGE:-}
EXTRA=""
ARCH=x86_64
while [ $# -gt 0 ]; do
	case $1 in
		--arch) ARCH=$2; EXTRA="$EXTRA --arch $2"; shift 2 ;;
		--real-kernel) EXTRA="$EXTRA --real-kernel $2"; shift 2 ;;
		--real-initrd) EXTRA="$EXTRA --real-initrd $2"; shift 2 ;;
		--real-cmdline) EXTRA="$EXTRA --real-cmdline $2"; shift 2 ;;
		--lean-image) LEAN=$2; shift 2 ;;
		--only) EXTRA="$EXTRA --only $2"; shift 2 ;;
		--keep) EXTRA="$EXTRA --keep"; shift ;;
		*) echo "usage: $0 [--arch x86_64|aarch64] [--only a|a2|b|c|d|e|f|g|h|i|j|k|l] [--keep] [--lean-image IMAGE.img]" >&2; exit 2 ;;
	esac
done
PATH=$PATH:/usr/sbin
export PATH
case $ARCH in
	aarch64) TRIPLE=aarch64-unknown-uefi; QEMU=qemu-system-aarch64; DCODE=/usr/share/AAVMF/AAVMF_CODE.fd; DVARS=/usr/share/AAVMF/AAVMF_VARS.fd ;;
	x86_64) TRIPLE=x86_64-unknown-uefi; QEMU=qemu-system-x86_64; DCODE=/usr/share/OVMF/OVMF_CODE_4M.fd; DVARS=/usr/share/OVMF/OVMF_VARS_4M.fd ;;
	*) echo "qemu-test: unknown --arch $ARCH" >&2; exit 2 ;;
esac
BOOT_EFI=${BOOT_EFI:-$TARGET/$TRIPLE/release/boot-uefi.efi}
TEST_EFI=${TEST_EFI:-$TARGET/$TRIPLE/release/test-efi.efi}
for t in "$QEMU" python3 sgdisk mkfs.vfat; do
	command -v "$t" >/dev/null 2>&1 || [ -x "/usr/sbin/$t" ] || { echo "qemu-test: $t is not installed" >&2; exit 1; }
done
# shellcheck disable=SC2086
exec python3 -I "$HERE/qemu_test.py" --boot-efi "$BOOT_EFI" --test-efi "$TEST_EFI" \
	--ovmf-code "${OVMF_CODE:-$DCODE}" --ovmf-vars "${OVMF_VARS:-$DVARS}" \
	--scratch "$SCRATCH" --lean-image "$LEAN" --initrd-marker "${INITRD_MARKER:-}" $EXTRA
