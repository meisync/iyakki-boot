// SPDX-License-Identifier: MIT OR Apache-2.0
//! `board-cfg BOARD VARIANT OUTDIR`: write the generated boot files of a board into a directory (for the image scripts).
//! BOARD is `rpi5` or `kv260`, VARIANT is `baseline` or `loader`. `board-cfg --list BOARD VARIANT` prints the paths only.
use board_cfg::{files, Board, Variant};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let list = args.first().map(String::as_str) == Some("--list");
    let a: Vec<&String> = args.iter().filter(|s| *s != "--list").collect();
    let usage = "usage: board-cfg [--list] rpi5|kv260 baseline|loader OUTDIR";
    if a.len() < 2 || (!list && a.len() != 3) {
        eprintln!("{usage}");
        std::process::exit(2);
    }
    let Some(board) = Board::parse(a[0]) else {
        eprintln!("board-cfg: unknown board '{}'; {usage}", a[0]);
        std::process::exit(2);
    };
    let variant = match a[1].as_str() {
        "baseline" => Variant::Baseline,
        "loader" => Variant::Loader,
        other => {
            eprintln!("board-cfg: unknown variant '{other}'; {usage}");
            std::process::exit(2);
        }
    };
    for f in files(&board, variant) {
        if list {
            println!("{}", f.path);
            continue;
        }
        let dest = Path::new(a[2].as_str()).join(&f.path);
        if let Some(dir) = dest.parent() {
            if let Err(e) = std::fs::create_dir_all(dir) {
                eprintln!("board-cfg: {}: {e}", dir.display());
                std::process::exit(1);
            }
        }
        if let Err(e) = std::fs::write(&dest, f.text.as_bytes()) {
            eprintln!("board-cfg: {}: {e}", dest.display());
            std::process::exit(1);
        }
    }
}
