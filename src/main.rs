// Copyright (c) 2014 Bart Massey
// [This program is licensed under the "MIT License"]
// See LICENSE.txt in the source distribution for license terms.
//
// duvis: ASCII (and GTK4) visualization of du(1) output.

use clap::Parser;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, stdin};
use std::process::exit;
use std::sync::atomic::{AtomicUsize, Ordering as AOrd};

#[cfg(feature = "gui")]
mod gui;
mod tree;

use tree::Duvis;

const IO_BUFFER_LENGTH: usize = 1024 * 1024;

#[derive(Parser, Debug)]
#[command(
    name = "duvis",
    about = "ASCII visualization of du(1) disk usage information"
)]
struct Args {
    /// Ingest preorder-laid-out input (lex-sort entries before building)
    #[arg(short = 'p')]
    preorder: bool,

    /// Output to xdu-style GUI
    #[cfg(feature = "gui")]
    #[arg(short = 'g')]
    gui: bool,

    /// GUI font size in points (used with -g)
    #[cfg(feature = "gui")]
    #[arg(long = "font-size", value_name = "PT", default_value_t = 10.0)]
    font_size: f64,

    /// Raw output: emit entries in current array order, indented by depth
    #[arg(short = 'r')]
    raw: bool,

    /// Input lines are NUL-terminated (use with `du -0`)
    #[arg(short = '0')]
    zero: bool,

    /// Do not sort children by size at display time; preserve build order
    #[arg(long)]
    unsorted: bool,

    /// NUL-terminate output records instead of newline (cf. find -print0)
    #[arg(long = "print0")]
    print0: bool,

    /// du output file (defaults to stdin)
    file: Option<String>,
}

fn status(msg: &str) {
    static PASS: AtomicUsize = AtomicUsize::new(1);
    let p = PASS.fetch_add(1, AOrd::SeqCst);
    eprintln!("({}) {}", p, msg);
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    let reader: Box<dyn BufRead> = match &args.file {
        Some(path) => {
            eprintln!("open {}", path);
            Box::new(BufReader::with_capacity(
                IO_BUFFER_LENGTH,
                File::open(path)?,
            ))
        }
        None => Box::new(BufReader::with_capacity(IO_BUFFER_LENGTH, stdin().lock())),
    };

    let mut duvis = Duvis::new();

    status("Parsing du file.");
    duvis.read_entries(reader, args.zero)?;

    if duvis.is_empty() {
        return Ok(());
    }

    let root_idx: usize;
    if args.preorder {
        status("Sorting entries.");
        duvis.sort_entries();
        if duvis.entry(0).components().is_empty() {
            eprintln!("Mysterious zero-length entry in table.");
            exit(1);
        }
        status("Building tree (preorder).");
        root_idx = 0;
        duvis.set_base_depth(duvis.entry(0).components().len());
        let n = duvis.len();
        duvis.build_tree_preorder(0, n, 0);
    } else {
        status("Building tree (postorder).");
        let n = duvis.len();
        root_idx = n - 1;
        duvis.set_base_depth(duvis.entry(root_idx).components().len());
        duvis.build_tree_postorder(0, n, 0);
    }

    if !args.unsorted && !args.raw {
        status("Sorting children by size.");
        duvis.sort_children_by_size();
    }

    #[cfg(feature = "gui")]
    if args.gui {
        status("Launching GUI.");
        let code = gui::run(duvis, root_idx, args.font_size);
        exit(code);
    }

    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    let term: u8 = if args.print0 { 0 } else { b'\n' };

    if args.raw {
        status("Emitting entries.");
        duvis.show_entries_raw(&mut out, term)?;
    } else {
        status("Emitting tree.");
        duvis.show_entries(&mut out, root_idx, term)?;
    }

    Ok(())
}
