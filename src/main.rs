// Copyright (c) 2014 Bart Massey
// [This program is licensed under the "MIT License"]
// See COPYING in the source distribution for license terms.
//
// Rust port of duvis.c. ASCII xdu replacement. Graphics
// (-g) intentionally omitted from this port.

use clap::Parser;
use std::cmp::Ordering;
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write, stdin};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::process::exit;
use std::sync::atomic::{AtomicUsize, Ordering as AOrd};

const N_INDENT: usize = 2;
const IO_BUFFER_LENGTH: usize = 1024 * 1024;

#[derive(Parser, Debug)]
#[command(
    name = "duvis",
    about = "ASCII visualization of du(1) disk usage information"
)]
struct Args {
    /// Build the tree in preorder (sort children by size desc, then name asc)
    #[arg(short = 'p')]
    preorder: bool,

    /// Output to xdu-style GUI (not implemented in this Rust port yet)
    #[arg(short = 'g')]
    gui: bool,

    /// Raw output: emit entries in current array order, indented by depth
    #[arg(short = 'r')]
    raw: bool,

    /// Input lines are NUL-terminated (use with `du -0`)
    #[arg(short = '0')]
    zero: bool,

    /// du output file (defaults to stdin)
    file: Option<String>,
}

struct Entry {
    size: u64,
    components: Vec<OsString>,
    depth: u32,
    children: Vec<usize>,
}

struct Duvis {
    entries: Vec<Entry>,
    base_depth: usize,
}

impl Duvis {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            base_depth: 0,
        }
    }

    fn read_entries<R: BufRead>(&mut self, mut reader: R, zero: bool) -> io::Result<()> {
        let terminator: u8 = if zero { 0 } else { b'\n' };
        let mut buf: Vec<u8> = Vec::new();
        let mut line_number: usize = 0;

        loop {
            buf.clear();
            let n = reader.read_until(terminator, &mut buf)?;
            if n == 0 {
                return Ok(());
            }

            if buf.last() == Some(&terminator) {
                buf.pop();
            } else {
                eprintln!("warning: unterminated final path");
            }

            line_number += 1;

            let ws_pos = buf
                .iter()
                .position(|&b| b == b' ' || b == b'\t')
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("line {}: buffer format error", line_number),
                    )
                })?;

            let size_str = std::str::from_utf8(&buf[..ws_pos]).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("line {}: size parse failure", line_number),
                )
            })?;

            let size: u64 = size_str.parse().map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("line {}: size parse failure", line_number),
                )
            })?;

            let path = &buf[ws_pos + 1..];
            let mut components: Vec<OsString> = path
                .split(|&b| b == b'/')
                .map(|s| OsString::from_vec(s.to_vec()))
                .collect();
            if components.last().is_some_and(|s| s.is_empty()) {
                components.pop();
            }

            self.entries.push(Entry {
                size,
                components,
                depth: 0,
                children: Vec::new(),
            });
        }
    }

    /// Sort entries so prefixes precede their extensions; alphabetical within.
    fn sort_entries(&mut self) {
        self.entries.sort_by(|a, b| {
            let n = a.components.len().min(b.components.len());
            for i in 0..n {
                let q = a.components[i].cmp(&b.components[i]);
                if q != Ordering::Equal {
                    return q;
                }
            }
            a.components.len().cmp(&b.components.len())
        });
    }

    /// Build tree using du's natural post-order output; no sort.
    fn build_tree_postorder(&mut self, start: usize, end: usize, depth: u32) {
        if start >= end {
            return;
        }
        let parent = end - 1;
        self.entries[parent].depth = depth;
        let parent_components = self.entries[parent].components.len();
        let child_components = parent_components + 1;

        let mut children: Vec<usize> = Vec::new();
        let mut i = start;
        while i < end - 1 {
            if self.entries[i].components.len() != child_components {
                i += 1;
                continue;
            }
            let mut j = i + 1;
            while j < end - 1 && self.entries[j].components.len() > child_components {
                j += 1;
            }
            children.push(i);
            self.entries[i].depth = depth + 1;
            if j > i + 1 {
                self.build_tree_postorder(i, j, depth + 1);
            }
            i = j;
        }
        self.entries[parent].children = children;
    }

    /// Build tree in preorder; sort each level by descending size, then name.
    fn build_tree_preorder(&mut self, start: usize, end: usize, depth: u32) {
        let offset = depth as usize + self.base_depth;
        if self.entries[start].components.len() != offset {
            eprintln!("index {}: unexpected entry", start + 1);
            exit(1);
        }
        self.entries[start].depth = depth;

        let mut children: Vec<usize> = Vec::new();
        let mut i = start + 1;
        while i < end {
            if self.entries[i].components.len() != offset + 1 {
                eprintln!("index {}: missing entry", i + 1);
                exit(1);
            }
            children.push(i);
            self.entries[i].depth = depth + 1;
            let mut j = i + 1;
            while j < end
                && self.entries[j].components.len() > offset + 1
                && self.entries[i].components[offset] == self.entries[j].components[offset]
            {
                j += 1;
            }
            if j > i + 1 {
                self.build_tree_preorder(i, j, depth + 1);
            }
            i = j;
        }

        children.sort_by(|&a, &b| {
            let ea = &self.entries[a];
            let eb = &self.entries[b];
            eb.size
                .cmp(&ea.size)
                .then_with(|| ea.components.last().cmp(&eb.components.last()))
        });
        self.entries[start].children = children;
    }

    fn indent<W: Write>(out: &mut W, depth: u32) -> io::Result<()> {
        for _ in 0..(N_INDENT * depth as usize) {
            out.write_all(b" ")?;
        }
        Ok(())
    }

    fn show_entries<W: Write>(&self, out: &mut W, idx: usize) -> io::Result<()> {
        let e = &self.entries[idx];
        if e.depth == 0 {
            out.write_all(e.components[0].as_bytes())?;
            for i in 1..self.base_depth {
                out.write_all(b"/")?;
                out.write_all(e.components[i].as_bytes())?;
            }
            writeln!(out, " {}", e.size)?;
        } else {
            Self::indent(out, e.depth)?;
            out.write_all(e.components.last().unwrap().as_bytes())?;
            writeln!(out, " {}", e.size)?;
        }
        for &c in &e.children {
            self.show_entries(out, c)?;
        }
        Ok(())
    }

    fn show_entries_raw<W: Write>(&self, out: &mut W) -> io::Result<()> {
        for e in &self.entries {
            Self::indent(out, e.depth)?;
            out.write_all(e.components.last().unwrap().as_bytes())?;
            writeln!(out, " {}", e.size)?;
        }
        Ok(())
    }
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
            Box::new(BufReader::with_capacity(IO_BUFFER_LENGTH, File::open(path)?))
        }
        None => Box::new(BufReader::with_capacity(IO_BUFFER_LENGTH, stdin().lock())),
    };

    let mut duvis = Duvis::new();

    status("Parsing du file.");
    duvis.read_entries(reader, args.zero)?;

    if duvis.entries.is_empty() {
        return Ok(());
    }

    let root_idx: usize;
    if args.preorder {
        status("Sorting entries.");
        duvis.sort_entries();
        if duvis.entries[0].components.is_empty() {
            eprintln!("Mysterious zero-length entry in table.");
            exit(1);
        }
        status("Building tree (preorder).");
        root_idx = 0;
        duvis.base_depth = duvis.entries[0].components.len();
        let n = duvis.entries.len();
        duvis.build_tree_preorder(0, n, 0);
    } else {
        status("Building tree (postorder).");
        let n = duvis.entries.len();
        root_idx = n - 1;
        duvis.base_depth = duvis.entries[root_idx].components.len();
        duvis.build_tree_postorder(0, n, 0);
    }

    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    if args.gui {
        eprintln!("GUI not implemented in this Rust port yet.");
        exit(1);
    } else if args.raw {
        status("Emitting entries.");
        duvis.show_entries_raw(&mut out)?;
    } else {
        status("Emitting tree.");
        duvis.show_entries(&mut out, root_idx)?;
    }

    Ok(())
}
