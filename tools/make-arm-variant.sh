#!/bin/sh
# SPDX-License-Identifier: MIT OR Apache-2.0
# Make the "loader" variant of a tested ARM card image: the same card with iyakki-boot in front. The baseline image is only read.
#   make-arm-variant.sh rpi5|kv260 BASELINE.img OUT.img [--loader BOOT-UEFI.EFI] [--board-cfg PATH]
# What it does (all inside a copy of the image, with mtools; no root, no mount, no loop device):
#   * the boot partition (the first one, found with sfdisk) gets EFI/BOOT/BOOTAA64.EFI (the loader), EFI/iyakki-boot/ (the log directory),
#     and the files `board-cfg BOARD loader` writes: boot.cfg for both boards, config.txt for the Pi (kernel=u-boot.bin);
#   * the baseline-only boot files are removed: the KV260's extlinux/ (so U-Boot reaches the EFI path), the Pi's cmdline*.txt,
#     iyakki-boot.txt and tryboot.txt (the loader's boot.cfg replaces them).
# No third-party blob is written: the Pi's u-boot.bin comes from the person's download (docs/BOARD-RPI5-LOADER.md).
# Needs: sfdisk, mtools (mcopy mdel mmd mdeltree mdir), cargo (only when --board-cfg is not given).
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/.." && pwd)
[ $# -ge 3 ] || { sed -n 3,5p "$0" >&2; exit 2; }
BOARD=$1; BASE=$2; OUT=$3; shift 3
LOADER=${BOOT_EFI:-$REPO/target/aarch64-unknown-uefi/release/boot-uefi.efi}
BC=""
while [ $# -gt 0 ]; do
	case $1 in
		--loader) LOADER=$2; shift 2 ;;
		--board-cfg) BC=$2; shift 2 ;;
		*) echo "make-arm-variant: unknown option $1" >&2; exit 2 ;;
	esac
done
case $BOARD in rpi5|kv260) ;; *) echo "make-arm-variant: board must be rpi5 or kv260" >&2; exit 2 ;; esac
[ -f "$BASE" ] || { echo "make-arm-variant: no baseline image $BASE" >&2; exit 1; }
[ -f "$LOADER" ] || { echo "make-arm-variant: no loader program $LOADER (cargo build --release --target aarch64-unknown-uefi -p boot-uefi)" >&2; exit 1; }
[ "$(readlink -f "$BASE")" != "$(readlink -f "$OUT")" ] || { echo "make-arm-variant: the output must not be the baseline" >&2; exit 1; }
if [ -z "$BC" ]; then
	(cd "$REPO" && cargo build -q --release -p board-cfg)
	BC=${CARGO_TARGET_DIR:-$REPO/target}/release/board-cfg
fi
T=$(mktemp -d "${TMPDIR:-/tmp}/arm-variant.XXXXXX"); trap 'rm -rf "$T"' EXIT
"$BC" "$BOARD" loader "$T/gen"
printf 'The loader writes its log here (last-boot.log) at every start.\n' > "$T/README.txt"
START=$(sfdisk -d "$BASE" | sed -n 's/^[^ ]*1 *: *start= *\([0-9]*\),.*/\1/p' | head -n1)
[ -n "$START" ] || { echo "make-arm-variant: no first partition in $BASE" >&2; exit 1; }
mkdir -p "$(dirname "$OUT")"
cp "$BASE" "$OUT.part"
MTOOLS_SKIP_CHECK=1; export MTOOLS_SKIP_CHECK
IMG="$OUT.part@@$((START * 512))"
if [ "$BOARD" = kv260 ]; then
	mdeltree -i "$IMG" ::extlinux
else
	for f in cmdline.txt cmdline-normal.txt cmdline-hwcheck.txt cmdline-safegraphics.txt cmdline-seed.txt cmdline-selftest.txt cmdline-ssh.txt cmdline-web.txt iyakki-boot.txt tryboot.txt; do
		mdel -i "$IMG" "::$f"
	done
fi
mmd -i "$IMG" ::EFI ::EFI/BOOT ::EFI/iyakki-boot
mcopy -i "$IMG" "$LOADER" ::EFI/BOOT/BOOTAA64.EFI
mcopy -i "$IMG" "$T/README.txt" ::EFI/iyakki-boot/README.txt
mcopy -o -i "$IMG" "$T/gen/boot.cfg" ::boot.cfg
[ "$BOARD" = rpi5 ] && mcopy -o -i "$IMG" "$T/gen/config.txt" ::config.txt
mcopy -o -i "$IMG" "$T/gen/board" ::board
mv "$OUT.part" "$OUT"
echo "made $OUT (loader variant of $BOARD)"; mdir -/ -i "$OUT@@$((START * 512))" :: | head -40
