// SPDX-License-Identifier: MIT OR Apache-2.0
//! One board table and one configuration for the ARM boards.
//!
//! The Raspberry Pi 5 and the AMD Kria KV260 each need a few small text files on their boot card. Until now the Pi's files were written
//! by one program and the KV260's by a shell recipe, with two lists of start modes. This crate holds ONE table of boards ([`Board`])
//! and ONE list of start modes ([`Mode`]), and writes every file from them as plain text ([`files`]):
//!
//! * [`Variant::Baseline`]: what the first images carry. The Pi: `config.txt`, `cmdline.txt` and `cmdline-<mode>.txt`, `iyakki-boot.txt`,
//!   `tryboot.txt`, `board`. The KV260: `extlinux/extlinux.conf` and `board`. The golden tests compare every byte with the files of the
//!   tested images, so a later switch of the image scripts to this crate changes nothing on the card.
//! * [`Variant::Loader`]: `iyakki-boot` in front. The loader's `boot.cfg` (entries, boot-tries counter, one-shot choice, rollback) for both
//!   boards, and on the Pi a `config.txt` that starts U-Boot (`kernel=u-boot.bin`), which starts the loader as an EFI program. The KV260
//!   needs no `extlinux.conf` here: the board's U-Boot then reaches `EFI/BOOT/BOOTAA64.EFI` by itself.
//!
//! Pure functions, no file access, no dependencies; the caller (a build script, `iyakki-usb`) writes the returned files. Nothing here is a
//! claim that a board boots: see the first-boot guides.

use std::fmt::Write as _;

/// A board.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoardId {
    Rpi5,
    Kv260,
}

/// Everything that differs between the boards. The one table: [`TABLE`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Board {
    pub id: BoardId,
    /// The word in the card's `board` file and on the command line (`iyakki.board=`).
    pub name: &'static str,
    pub title: &'static str,
    /// The kernel file on the boot volume (an arm64 `Image`: it carries the EFI stub, so the loader starts it as it is).
    pub kernel: &'static str,
    /// The device tree file the Pi firmware loads (`None`: the board's own firmware supplies it, as on the KV260).
    pub dtb: Option<&'static str>,
    /// The first-stage RAM disk (fast start) and the full one (fallback), as file names on the boot volume. The KV260 has one.
    pub initrd: &'static str,
    pub initrd_full: Option<&'static str>,
    /// The screen console word and the serial console word of the kernel command line (the last `console=` is `/dev/console`).
    pub screen_console: &'static str,
    pub serial_console: &'static str,
    /// What follows the consoles, before the mode's own words.
    pub cmdline_tail: &'static str,
    /// Whether the command line carries `iyakki.board=<name>` (the Pi does; the KV260's baseline does not).
    pub board_word: bool,
    /// The seed-only mode exists (the Pi's volume carries the seed root; the KV260 image is a plain RAM-disk start).
    pub seed_mode: bool,
    /// The program U-Boot starts as the loader's EFI path on the card (the removable-media name for aarch64).
    pub efi_path: &'static str,
    /// The display overlay of `config.txt`. The Pi 5 firmware has no `overlay_map.dtb`, so it needs the file that carries the `-pi5` name.
    pub display_overlay: &'static str,
}

pub const RPI5: Board = Board {
    id: BoardId::Rpi5,
    name: "rpi5",
    title: "Raspberry Pi 5",
    kernel: "kernel8.img",
    dtb: Some("bcm2712-rpi-5-b.dtb"),
    initrd: "initrd-lean",
    initrd_full: Some("initrd"),
    screen_console: "console=tty1",
    serial_console: "console=ttyAMA10,115200",
    cmdline_tail: "rootwait panic=10 cgroup_enable=memory cgroup_memory=1",
    board_word: true,
    seed_mode: true,
    efi_path: "EFI/BOOT/BOOTAA64.EFI",
    display_overlay: "vc4-kms-v3d-pi5",
};

pub const KV260: Board = Board {
    id: BoardId::Kv260,
    name: "kv260",
    title: "AMD Kria KV260",
    kernel: "Image",
    dtb: None,
    initrd: "rootfs.cpio.gz",
    initrd_full: None,
    screen_console: "console=tty0",
    serial_console: "console=ttyPS1,115200",
    cmdline_tail: "earlycon clk_ignore_unused panic=20",
    board_word: false,
    seed_mode: false,
    efi_path: "EFI/BOOT/BOOTAA64.EFI",
    display_overlay: "",
};

/// The table of boards.
pub const TABLE: [Board; 2] = [RPI5, KV260];

impl Board {
    pub fn parse(s: &str) -> Option<Board> {
        match s.trim().to_ascii_lowercase().as_str() {
            "pi5" | "rpi5" | "raspberrypi5" => Some(RPI5),
            "kv260" | "kria" | "kria-kv260" => Some(KV260),
            _ => None,
        }
    }
}

/// The start modes: one list for every board (the Pi's `cmdline-<mode>.txt` files, the KV260's extlinux labels, the loader's entries).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Normal,
    HwCheck,
    SafeGraphics,
    SelfTest,
    Ssh,
    Web,
    /// The root is taken from the volume's own seed only (`iyakki.root=boot`): the rollback of last resort (boards with `seed_mode`).
    SeedOnly,
}

impl Mode {
    pub const ALL: [Mode; 7] = [Mode::Normal, Mode::HwCheck, Mode::SafeGraphics, Mode::SelfTest, Mode::Ssh, Mode::Web, Mode::SeedOnly];
    pub fn name(self) -> &'static str {
        match self {
            Mode::Normal => "normal",
            Mode::HwCheck => "hwcheck",
            Mode::SafeGraphics => "safegraphics",
            Mode::SelfTest => "selftest",
            Mode::Ssh => "ssh",
            Mode::Web => "web",
            Mode::SeedOnly => "seed",
        }
    }
    /// The words added to the command line (with the leading space).
    fn extra(self) -> &'static str {
        match self {
            Mode::Normal => "",
            Mode::HwCheck => " iyakki.hwcheck=1",
            // the PC menu's safe-graphics row blacklists the GPU drivers; on a Pi that leaves the firmware's own framebuffer
            Mode::SafeGraphics => " modprobe.blacklist=vc4,v3d",
            Mode::SelfTest => " iyakki.selftest=1",
            Mode::Ssh => " iyakki.ssh=1",
            Mode::Web => " iyakki.web=1",
            Mode::SeedOnly => " iyakki.root=boot",
        }
    }
    /// The menu title in the loader's `boot.cfg`.
    fn title(self) -> &'static str {
        match self {
            Mode::Normal => "IYAKKI",
            Mode::HwCheck => "Hardware check (keyboard, mouse, touch, display)",
            Mode::SafeGraphics => "Safe graphics (no GPU drivers)",
            Mode::SelfTest => "Boot self-test (verbose, powers off)",
            Mode::Ssh => "Remote diagnostics (SSH, keys only)",
            Mode::Web => "Also in a browser (HTTPS on port 8443)",
            Mode::SeedOnly => "Seed system only (ignore updates)",
        }
    }
    /// The modes a board offers.
    pub fn for_board(b: &Board) -> Vec<Mode> {
        Mode::ALL.into_iter().filter(|m| b.seed_mode || *m != Mode::SeedOnly).collect()
    }
}

/// The kernel command line of a board in a mode: the screen console first, the serial console last (the LAST `console=` becomes
/// `/dev/console`, so the boot messages reach a serial adapter and the screen). No trailing newline.
pub fn cmdline(b: &Board, mode: Mode) -> String {
    let mut s = format!("{} {} {}", b.screen_console, b.serial_console, b.cmdline_tail);
    if b.board_word {
        let _ = write!(s, " iyakki.board={}", b.name);
    }
    s.push_str(mode.extra());
    s
}

/// Which files to write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    /// The first images: the Pi firmware (or the KV260's U-Boot extlinux) starts the kernel directly.
    Baseline,
    /// `iyakki-boot` in front: the loader's `boot.cfg`, and on the Pi U-Boot started by `config.txt`.
    Loader,
}

/// Which RAM disk the Pi's next baseline start reads (`iyakki-boot.txt`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Initrd {
    Lean,
    Full,
}

/// One file of the boot volume: path (no leading slash) and bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BootFile {
    pub path: String,
    pub text: String,
}

fn file(path: &str, text: String) -> BootFile {
    BootFile { path: path.to_string(), text }
}

/// The U-Boot file the Pi variant starts instead of the kernel. Fetched by the person's click (GPL-2.0+, never shipped).
pub const UBOOT_BIN: &str = "u-boot.bin";
/// What the person downloads to get `u-boot.bin` for the Pi 5: a pinned Debian package (GPL-2.0+; built from upstream U-Boot 2025.01 with
/// the BCM2712 device tree, EFI loader, EFI variables kept in a file on the boot volume, FAT write support: read from the package's own
/// `config.rpi_arm64.gz`). The files are NEVER shipped or kept in this repository; a button downloads them at the person's click with the
/// licence shown first, checks size and SHA-256, and extracts `u-boot.bin` (an `ar` archive holding `data.tar.xz`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DownloadPin {
    pub url: &'static str,
    pub size: u64,
    pub sha256: &'static str,
}

/// The pinned download of the Pi 5 loader's U-Boot, taken 2026-11-24 (downloaded once and hashed; the package's checksum equals the
/// `Packages` index of Debian trixie).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UbootPin {
    pub package: DownloadPin,
    /// The licence text of that exact version (the package's `debian/copyright`: GPL-2.0+ among others), shown before the download.
    pub licence: DownloadPin,
    /// The file inside the package (`data.tar.xz`, path below) and its own size and checksum, checked after the extraction.
    pub inner_path: &'static str,
    pub inner_size: u64,
    pub inner_sha256: &'static str,
    pub licence_spdx: &'static str,
}

pub const RPI5_UBOOT: UbootPin = UbootPin {
    package: DownloadPin {
        url: "https://deb.debian.org/debian/pool/main/u/u-boot/u-boot-rpi_2025.01-3+deb13u1_arm64.deb",
        size: 540_796,
        sha256: "c9e1aa3a17ccc1ff85858ace8cc43c917d1e0f70f04258438f1ae9b84b8b8c72",
    },
    licence: DownloadPin {
        url: "https://sources.debian.org/data/main/u/u-boot/2025.01-3+deb13u1/debian/copyright",
        size: 906_557,
        sha256: "303b441e3c744e78b8fdbf7388a869de978cf88afbf683ae02cdbecdc7f9efb3",
    },
    inner_path: "./usr/lib/u-boot/rpi_arm64/u-boot.bin",
    inner_size: 681_256,
    inner_sha256: "ec5a73ac3f8774445260d50acb60c43e581a2bc7036c2c8b3c8760f80f0fdffb",
    licence_spdx: "GPL-2.0-or-later",
};

/// The loader's own folder on the card and its menu file.
pub const LOADER_CFG: &str = "boot.cfg";

/// `iyakki-boot.txt` of the Pi baseline: one line, the RAM disk. `followkernel` puts it right after the kernel in memory.
pub fn pi_iyakki_boot_txt(b: &Board, which: Initrd) -> String {
    let file = match which {
        Initrd::Lean => b.initrd,
        Initrd::Full => b.initrd_full.unwrap_or(b.initrd),
    };
    format!("# IYAKKI: which RAM disk the next start reads (written by the first stage after a failed fast start and by the running system when the fast tree is ready)\ninitramfs {file} followkernel\n")
}

/// `config.txt` of the Pi baseline.
pub fn pi_config_txt(b: &Board) -> String {
    let mut t = String::new();
    t.push_str("# IYAKKI boot configuration for the Raspberry Pi 5. Written by iyakki-usb; an update rewrites it. Do not edit by hand: put your own\n");
    t.push_str("# lines in config-user.txt (included last).\n");
    t.push_str("[all]\narm_64bit=1\n");
    let _ = writeln!(t, "kernel={}", b.kernel);
    let _ = writeln!(t, "device_tree={}", b.dtb.unwrap_or(""));
    let _ = writeln!(t, "dtoverlay={}", b.display_overlay);
    t.push_str("max_framebuffers=2\nenable_uart=1\nuart_2ndstage=1\ndisable_splash=1\n");
    t.push_str("include iyakki-boot.txt\ninclude config-user.txt\n");
    t
}

/// `config.txt` of the Pi with the loader: the firmware starts U-Boot (`kernel=u-boot.bin`) and hands it the device tree; U-Boot's
/// distro boot finds `EFI/BOOT/BOOTAA64.EFI` (the loader) on the first FAT partition. No `initramfs` line: the loader reads the RAM disk
/// itself. Same display and serial lines as the baseline.
pub fn pi_config_txt_loader(b: &Board) -> String {
    let mut t = String::new();
    t.push_str("# IYAKKI boot configuration for the Raspberry Pi 5 with the IYAKKI boot loader. Written by iyakki-usb; an update rewrites it. Do not edit by hand: put your own\n");
    t.push_str("# lines in config-user.txt (included last). The firmware starts U-Boot, U-Boot starts the loader (EFI/BOOT/BOOTAA64.EFI), the loader starts the kernel.\n");
    t.push_str("[all]\narm_64bit=1\n");
    let _ = writeln!(t, "kernel={UBOOT_BIN}");
    let _ = writeln!(t, "device_tree={}", b.dtb.unwrap_or(""));
    let _ = writeln!(t, "dtoverlay={}", b.display_overlay);
    t.push_str("max_framebuffers=2\nenable_uart=1\nuart_2ndstage=1\ndisable_splash=1\n");
    t.push_str("include config-user.txt\n");
    t
}

/// `tryboot.txt` of the Pi baseline (read once by the firmware after `reboot "0 tryboot"`).
pub fn pi_tryboot_txt(b: &Board) -> String {
    let mut t = String::new();
    t.push_str("# IYAKKI trial boot of an update: read ONCE by the firmware after `reboot \"0 tryboot\"`, never by a normal start.\n");
    t.push_str("[all]\narm_64bit=1\n");
    let stem = b.kernel.strip_suffix(".img").unwrap_or(b.kernel);
    let _ = writeln!(t, "kernel={stem}-next.img");
    let _ = writeln!(t, "device_tree={}", b.dtb.unwrap_or(""));
    let _ = writeln!(t, "dtoverlay={}", b.display_overlay);
    t.push_str("max_framebuffers=2\nenable_uart=1\nuart_2ndstage=1\ndisable_splash=1\n");
    let _ = writeln!(t, "initramfs {}-next followkernel", b.initrd);
    t.push_str("include config-user.txt\n");
    t
}

/// The extlinux entries of the KV260 baseline: label, menu text, the device tree file (`None`: U-Boot's own), the words after the base line.
const KV_ENTRIES: [(&str, &str, Option<&str>, &str); 6] = [
    ("iyakki-dtb", "IYAKKI (carrier card rev B, default)", Some("/dtb/zynqmp-smk-k26-revA-sck-kv-g-revB.dtb"), " panic=20"),
    ("iyakki", "IYAKKI (the board's own device tree: may show no display)", None, " panic=20"),
    ("iyakki-dtb-reva", "IYAKKI (device tree for carrier card rev A)", Some("/dtb/zynqmp-smk-k26-revA-sck-kv-g-revA.dtb"), " panic=20"),
    ("iyakki-selftest", "IYAKKI (verbose, boot self-test, then power off)", None, " iyakki.selftest=1"),
    ("iyakki-ssh", "IYAKKI (remote diagnostics, SSH with keys only)", None, " iyakki.ssh=1"),
    ("iyakki-web", "IYAKKI (also in a browser, HTTPS on port 8443)", None, " iyakki.web=1"),
];

/// `extlinux/extlinux.conf` of the KV260 baseline.
pub fn kv_extlinux_conf(b: &Board) -> String {
    let mut t = String::new();
    t.push_str("# Read by the board's own U-Boot (primary boot firmware in the SOM's QSPI flash) from the SD card's FIRST bootable FAT partition:\n");
    t.push_str("# /extlinux/extlinux.conf (also found in /boot/extlinux/). The DEFAULT entry \"iyakki-dtb\" passes the device tree for carrier card\n");
    t.push_str("# rev B from the SD card: on the KV260 starter kit with rev B the board's own tree (entry \"iyakki\", no `fdt` line) gives no display\n");
    t.push_str("# (KI-1031). The entry \"iyakki\" keeps U-Boot's own device tree, for a board where it works. Serial: PS UART1 = ttyPS1, 115200 8N1\n");
    t.push_str("# (the micro-USB connector). The LAST console= word becomes /dev/console, so the serial port is last. Menu times are in tenths of a second.\n");
let _ = writeln!(t, "menu title IYAKKI ({})", b.title);
    t.push_str("default iyakki-dtb\ntimeout 30\nprompt 1\n");
    let base = format!("{} {} {}", b.screen_console, b.serial_console, b.cmdline_tail);
    // the baseline's words end with `panic=20` for the first three and have no panic word for the rest: split the tail the same way
    let base_no_panic = base.strip_suffix(" panic=20").unwrap_or(&base).to_string();
    for (label, menu, fdt, extra) in KV_ENTRIES {
        let _ = writeln!(t, "\nlabel {label}\n  menu label {menu}\n  linux /{}\n  initrd /{}", b.kernel, b.initrd);
        if let Some(f) = fdt {
            let _ = writeln!(t, "  fdt {f}");
        }
        let line = format!("{base_no_panic}{extra}");
        let _ = writeln!(t, "  append {line}");
    }
    t
}

/// The text of the `board` file.
pub fn board_file(b: &Board) -> String {
    format!("{}\n", b.name)
}

/// The loader's `boot.cfg` for a board: one menu and one rollback for every board. The first level is the normal start (and "More
/// options"); the other modes are on the second level. A start that is never confirmed good is counted; after `tries` of them the
/// loader starts the `previous` entry (the Pi: the full RAM disk, in place of the old lean/full switch of `iyakki-boot.txt`) and, if that
/// fails too, stops and waits for the person. A `BootConfirm` variable written by the running system refills the tries.
pub fn loader_boot_cfg(b: &Board) -> String {
    let mut t = String::new();
    // the KV260's own U-Boot tree gives no display on carrier card rev B (KI-1031): every entry names the rev B tree on the card
    let fdt = if b.id == BoardId::Kv260 { "  fdt /dtb/zynqmp-smk-k26-revA-sck-kv-g-revB.dtb\n" } else { "" };
    let _ = writeln!(t, "# IYAKKI boot loader menu for the {}. Written by board-cfg (one board table, one configuration for every board).", b.title);
    t.push_str("# The first level is the normal start and \"More options\"; the other starts are on the second level. Paths are on the boot volume.\n");
    t.push_str("# nine counted starts, like the PC images: a start the running system never confirms (the confirmation is a UEFI variable the system writes\n# at run time; whether U-Boot keeps it is still to be shown on a real board) costs one, then the previous entry starts, then the menu waits\ntimeout 0\ntries 9\nscan no\n");
    for m in Mode::for_board(b) {
        let id = if m == Mode::Normal { "iyakki" } else { m.name() };
        let _ = write!(t, "\nentry {id}\n  title {}\n  kind linux\n  kernel /{}\n  initrd /{}\n  cmdline {}\n{fdt}", m.title(), b.kernel, b.initrd, cmdline(b, m));
        if m == Mode::Normal {
            t.push_str("  default\n");
            if b.initrd_full.is_some() {
                t.push_str("  previous full\n");
            }
        } else {
            t.push_str("  more\n");
        }
    }
    if let Some(full) = b.initrd_full {
        let _ = write!(
            t,
            "\nentry full\n  title Full start (slower; if the fast start does not work)\n  kind linux\n  kernel /{}\n  initrd /{full}\n  cmdline {}\n{fdt}  more\n  requires /{full}\n",
            b.kernel,
            cmdline(b, Mode::Normal)
        );
    }
    t
}

/// Every file of a board's boot text, for the variant asked for. (Binary files, the kernel, the RAM disks and the loader program itself
/// are copied by the image script; this is only what is generated.)
pub fn files(b: &Board, variant: Variant) -> Vec<BootFile> {
    let mut v = Vec::new();
    match (b.id, variant) {
        (BoardId::Rpi5, Variant::Baseline) => {
            v.push(file("config.txt", pi_config_txt(b)));
            v.push(file("cmdline.txt", format!("{}\n", cmdline(b, Mode::Normal))));
            for m in Mode::for_board(b) {
                v.push(file(&format!("cmdline-{}.txt", m.name()), format!("{}\n", cmdline(b, m))));
            }
            v.push(file("iyakki-boot.txt", pi_iyakki_boot_txt(b, Initrd::Lean)));
            v.push(file("tryboot.txt", pi_tryboot_txt(b)));
            v.push(file("board", board_file(b)));
        }
        (BoardId::Rpi5, Variant::Loader) => {
            v.push(file("config.txt", pi_config_txt_loader(b)));
            v.push(file(LOADER_CFG, loader_boot_cfg(b)));
            v.push(file("board", board_file(b)));
        }
        (BoardId::Kv260, Variant::Baseline) => {
            v.push(file("extlinux/extlinux.conf", kv_extlinux_conf(b)));
            v.push(file("board", board_file(b)));
        }
        (BoardId::Kv260, Variant::Loader) => {
            // no extlinux.conf on purpose: U-Boot's distro boot then falls through to the EFI path
            v.push(file(LOADER_CFG, loader_boot_cfg(b)));
            v.push(file("board", board_file(b)));
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(dir: &str, name: &str) -> String {
        let p = format!("{}/tests/golden/{dir}/{name}", env!("CARGO_MANIFEST_DIR"));
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{p}: {e}"))
    }

    fn get(v: &[BootFile], path: &str) -> String {
        v.iter().find(|f| f.path == path).unwrap_or_else(|| panic!("no file {path}")).text.clone()
    }

    #[test]
    fn pi_baseline_files_are_the_bytes_of_the_tested_image() {
        let v = files(&RPI5, Variant::Baseline);
        let names: Vec<&str> = v.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            names,
            ["config.txt", "cmdline.txt", "cmdline-normal.txt", "cmdline-hwcheck.txt", "cmdline-safegraphics.txt", "cmdline-selftest.txt", "cmdline-ssh.txt",
             "cmdline-web.txt", "cmdline-seed.txt", "iyakki-boot.txt", "tryboot.txt", "board"]
        );
        for f in &v {
            assert_eq!(f.text, g("baseline-rpi5", &f.path), "{} differs from the golden file", f.path);
        }
    }

    #[test]
    fn pi_full_initrd_line() {
        assert_eq!(pi_iyakki_boot_txt(&RPI5, Initrd::Full), g("baseline-rpi5", "iyakki-boot-full.txt"));
        assert_eq!(pi_iyakki_boot_txt(&RPI5, Initrd::Lean), g("baseline-rpi5", "iyakki-boot.txt"));
    }

    #[test]
    fn kv260_baseline_files_are_the_bytes_of_the_tested_image() {
        let v = files(&KV260, Variant::Baseline);
        assert_eq!(v.len(), 2);
        assert_eq!(get(&v, "extlinux/extlinux.conf"), g("baseline-kv260", "extlinux.conf"));
        assert_eq!(get(&v, "board"), g("baseline-kv260", "board"));
    }

    #[test]
    fn loader_files_match_the_golden_files() {
        let pi = files(&RPI5, Variant::Loader);
        assert_eq!(pi.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["config.txt", "boot.cfg", "board"]);
        assert_eq!(get(&pi, "config.txt"), g("loader-rpi5", "config.txt"));
        assert_eq!(get(&pi, "boot.cfg"), g("loader-rpi5", "boot.cfg"));
        let kv = files(&KV260, Variant::Loader);
        assert_eq!(kv.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["boot.cfg", "board"]);
        assert_eq!(get(&kv, "boot.cfg"), g("loader-kv260", "boot.cfg"));
    }

    #[test]
    fn the_kv260_loader_variant_has_no_extlinux() {
        assert!(files(&KV260, Variant::Loader).iter().all(|f| !f.path.contains("extlinux")));
        assert!(files(&KV260, Variant::Baseline).iter().any(|f| f.path == "extlinux/extlinux.conf"));
    }

    #[test]
    fn the_pi_loader_config_starts_uboot_and_no_kernel_or_initramfs() {
        let c = pi_config_txt_loader(&RPI5);
        assert!(c.contains("\nkernel=u-boot.bin\n"));
        assert!(!c.contains("kernel8"), "{c}");
        assert!(!c.contains("initramfs"), "{c}");
        assert!(c.contains("device_tree=bcm2712-rpi-5-b.dtb"));
    }

    #[test]
    fn one_mode_list_for_every_board() {
        // the loader menu and the baseline files name the same modes, in the same words
        for b in TABLE {
            let cfg = loader_boot_cfg(&b);
            for m in Mode::for_board(&b) {
                assert!(cfg.contains(&format!("cmdline {}\n", cmdline(&b, m))), "{} lacks the {} line", b.name, m.name());
            }
        }
        assert!(!Mode::for_board(&KV260).contains(&Mode::SeedOnly));
        assert!(Mode::for_board(&RPI5).contains(&Mode::SeedOnly));
    }

    #[test]
    fn the_loader_menu_has_one_default_a_rollback_and_the_second_level() {
        for b in TABLE {
            let cfg = loader_boot_cfg(&b);
            assert_eq!(cfg.matches("\n  default\n").count(), 1, "{}", b.name);
            assert!(cfg.contains("\ntimeout 0\n") && cfg.contains("\ntries 9\n"));
            assert!(cfg.matches("\n  more\n").count() >= 5);
            // every initrd a row names is one the board has
            for l in cfg.lines().filter(|l| l.trim_start().starts_with("initrd ")) {
                let p = l.trim().trim_start_matches("initrd /");
                assert!(p == b.initrd || Some(p) == b.initrd_full, "{p}");
            }
        }
        let pi = loader_boot_cfg(&RPI5);
        assert!(pi.contains("  previous full\n") && pi.contains("entry full\n") && pi.contains("  requires /initrd\n"));
        assert!(!loader_boot_cfg(&KV260).contains("\n  previous "));
    }

    #[test]
    fn the_last_console_word_is_the_serial_port() {
        for b in TABLE {
            for m in Mode::ALL {
                let c = cmdline(&b, m);
                let last = c.split(' ').rev().find(|w| w.starts_with("console=")).unwrap();
                assert_eq!(last, b.serial_console);
            }
        }
    }

    #[test]
    fn nothing_generated_names_a_person_a_path_of_the_build_host_or_a_blob() {
        for b in TABLE {
            for var in [Variant::Baseline, Variant::Loader] {
                for f in files(&b, var) {
                    for bad in ["/home/", "/Users/", "/mnt/", "start4.elf", "fixup", "bootcode", ".bin\n"] {
                        assert!(!f.text.replace("kernel=u-boot.bin\n", "").contains(bad), "{} contains {bad}", f.path);
                    }
                    assert!(f.text.is_ascii() && f.text.ends_with('\n'), "{}", f.path);
                }
            }
        }
    }

    #[test]
    fn the_loaders_own_parser_accepts_every_generated_menu() {
        for b in TABLE {
            let text = loader_boot_cfg(&b);
            let cfg = boot_core::config::Config::parse(&text).unwrap_or_else(|e| panic!("{}: {e:?}", b.name));
            assert_eq!(cfg.tries, 9);
            assert_eq!(cfg.timeout_secs, 0);
            let ids: Vec<&str> = cfg.entries.iter().map(|e| e.id.as_str()).collect();
            assert_eq!(ids.len(), Mode::for_board(&b).len() + usize::from(b.initrd_full.is_some()), "{ids:?}");
            let def: Vec<&str> = cfg.entries.iter().filter(|e| e.default).map(|e| e.id.as_str()).collect();
            assert_eq!(def, ["iyakki"]);
            // every `previous` names an entry that exists
            for e in &cfg.entries {
                for p in &e.previous {
                    assert!(ids.contains(&p.as_str()), "{} names a previous entry {p} that is not there", e.id);
                }
                assert_eq!(e.kernel, format!("/{}", b.kernel));
            }
        }
    }

    #[test]
    fn the_uboot_pin_is_complete_and_points_at_https() {
        let p = RPI5_UBOOT;
        for d in [p.package, p.licence] {
            assert!(d.url.starts_with("https://") && d.size > 0 && d.sha256.len() == 64 && d.sha256.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
        }
        assert!(p.inner_sha256.len() == 64 && p.inner_size > 100_000 && p.inner_path.ends_with("/u-boot.bin") && !p.inner_path.contains(".."));
        assert!(p.package.url.contains("2025.01-3+deb13u1") && p.licence.url.contains("2025.01-3+deb13u1"), "both pins name the same version");
        assert_eq!(p.licence_spdx, "GPL-2.0-or-later");
    }

    #[test]
    fn parse_names() {
        assert_eq!(Board::parse("PI5"), Some(RPI5));
        assert_eq!(Board::parse(" kv260 "), Some(KV260));
        assert_eq!(Board::parse("pi3"), None);
    }
}
