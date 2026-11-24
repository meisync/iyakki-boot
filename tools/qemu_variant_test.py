#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Boot a finished ARM card image (the "loader" variant made by make-arm-variant.sh) in QEMU virt with the aarch64 edk2 firmware, the way
U-Boot would hand it over: firmware -> EFI/BOOT/BOOTAA64.EFI (the loader) -> boot.cfg -> the card's own kernel and RAM disk.

    qemu_variant_test.py IMAGE.img rpi5|kv260 --scratch DIR [--ovmf-code F --ovmf-vars F] [--keep]

The image is copied first and never written. In the copy only the console words of boot.cfg change (QEMU's UART is ttyAMA0, the boards' are
not) and the boot self-test is requested, so the run ends by itself. What it checks: the loader starts from the card without U-Boot, reads
boot.cfg, starts the card's real kernel and RAM disk, the system runs its self-test, the boot-tries counter goes down across resets (the
variable store is kept between runs), and (rpi5, `tries` cut to 2 in the copy) a start that is never confirmed falls back to the full RAM disk
by itself, which replaces the baseline's iyakki-boot.txt switch. Not proven: U-Boot, the board's firmware, any real device.
"""
import argparse
import os
import re
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import qemu_test as qt  # noqa: E402


def mtools(img, off, *cmd):
    env = dict(os.environ, MTOOLS_SKIP_CHECK="1")
    return subprocess.run([cmd[0], "-i", f"{img}@@{off}"] + list(cmd[1:]), check=True, capture_output=True, env=env).stdout


def part1_offset(img):
    out = subprocess.run(["sfdisk", "-d", img], check=True, capture_output=True, text=True).stdout
    m = re.search(r"^\S+1\s*:\s*start=\s*(\d+),", out, re.M)
    if not m:
        raise SystemExit("no first partition in the image")
    return int(m.group(1)) * 512


def main():
    p = argparse.ArgumentParser()
    p.add_argument("image")
    p.add_argument("board", choices=("rpi5", "kv260"))
    p.add_argument("--scratch", required=True)
    p.add_argument("--ovmf-code", default="/usr/share/AAVMF/AAVMF_CODE.fd")
    p.add_argument("--ovmf-vars", default="/usr/share/AAVMF/AAVMF_VARS.fd")
    p.add_argument("--keep", action="store_true")
    a = p.parse_args()
    a.arch, a.qemu = "aarch64", "qemu-system-aarch64"
    os.makedirs(a.scratch, exist_ok=True)
    work = tempfile.mkdtemp(prefix="variant-", dir=a.scratch)
    img = os.path.join(work, "card.img")
    shutil.copyfile(a.image, img)
    off = part1_offset(img)
    fails = []

    def check(cond, msg):
        print(("ok   " if cond else "FAIL ") + msg, flush=True)
        if not cond:
            fails.append(msg)

    root = [x.split()[-1].lower() for x in mtools(img, off, "mdir", "::").decode().splitlines() if x.strip()]
    check("extlinux" not in " ".join(root), "no extlinux.conf on the card: U-Boot can only reach the EFI path")
    check(mtools(img, off, "mtype", "::EFI/BOOT/BOOTAA64.EFI")[:2] == b"MZ", "EFI/BOOT/BOOTAA64.EFI is a PE program")
    cfg = mtools(img, off, "mtype", "::boot.cfg").decode()
    # the test copy: QEMU's console, the self-test (the run ends by itself); tries cut to 2 on the Pi so the fall back is reached
    cfg = re.sub(r"console=ttyAMA10,115200 ?|console=ttyPS1,115200 ?|console=tty1 ?|console=tty0 ?", "", cfg)
    cfg = re.sub(r"(\n  cmdline )(.*)", lambda m: m.group(1) + m.group(2) + " console=ttyAMA0,115200 iyakki.selftest=1 panic=-1", cfg)
    if a.board == "rpi5":
        cfg = cfg.replace("\ntries 9\n", "\ntries 2\n")
    tmp = os.path.join(work, "boot.cfg")
    with open(tmp, "w") as f:
        f.write(cfg)
    mtools(img, off, "mcopy", "-o", tmp, "::boot.cfg")
    vars_path = os.path.join(work, "vars.fd")
    shutil.copyfile(a.ovmf_vars, vars_path)  # kept between the runs: the loader's counter lives in it, like on a card that is restarted

    def run(name, expect_tries, kernel):
        vm = qt.Vm(a, work, name, [(img, False)], vars_path=vars_path)
        try:
            m = vm.wait_for(r"start: (\w+) kernel=(\S+)", 120)
            check(bool(m), f"{name}: the loader started an entry from the card's own boot.cfg")
            t = vm.text()
            if expect_tries is not None:
                check(bool(re.search(rf"tries_left={expect_tries}\b", t)), f"{name}: the counter says tries_left={expect_tries}")
            check(bool(m) and m.group(2) == kernel, f"{name}: the kernel started is {kernel} (got {m and m.group(2)})")
            end = vm.wait_for(r"IYAKKI-SELFTEST end", 400)
            check(bool(end), f"{name}: the real kernel and RAM disk ran the boot self-test to its end")
            check("Linux version" in vm.text() or "Booting Linux" in vm.text(), f"{name}: the kernel banner is in the serial log")
            if not end:
                print(vm.text()[-1500:])
            return vm.text()
        finally:
            vm.stop()

    if a.board == "kv260":
        run("run1", 8, "/Image")
        run("run2", 7, "/Image")
    else:
        run("run1", 1, "/kernel8.img")
        run("run2", 0, "/kernel8.img")
        t = run("run3", None, "/kernel8.img")
        m3 = re.search(r"start: (\w+) kernel=", t)
        check(bool(m3) and m3.group(1) == "full", "run3: the loader fell back to the entry `full` (the full RAM disk) by itself, without the menu")
        print("   run3 decision lines: " + "; ".join(re.findall(r"decide: [^\n]*", t)), flush=True)
    log = mtools(img, off, "mtype", "::EFI/iyakki-boot/last-boot.log").decode("utf-8", "replace")
    print("--- last-boot.log (loader's own log on the card), first lines ---\n" + "\n".join(log.splitlines()[:6]))
    if not a.keep:
        shutil.rmtree(work, ignore_errors=True)
    print(f"{'FAILED' if fails else 'PASS'}: {len(fails)} failed check(s)")
    sys.exit(1 if fails else 0)


if __name__ == "__main__":
    main()
