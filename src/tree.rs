// Copyright (c) 2014 Bart Massey
// [This program is licensed under the "MIT License"]
// See LICENSE.txt in the source distribution for license terms.
//
// Tree model: parse `du` output into `Entry` records, build
// parent/child links (post- or preorder), and emit them in
// the standard duvis format.

use std::cmp::Ordering;
use std::ffi::OsString;
use std::io::{self, BufRead, Write};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::process::exit;

pub const N_INDENT: usize = 2;

pub struct Entry {
    size: u64,
    components: Vec<OsString>,
    depth: u32,
    children: Vec<usize>,
}

impl Entry {
    #[cfg(feature = "gui")]
    pub fn size(&self) -> u64 {
        self.size
    }
    #[cfg(feature = "gui")]
    pub fn depth(&self) -> u32 {
        self.depth
    }
    pub fn components(&self) -> &[OsString] {
        &self.components
    }
    #[cfg(feature = "gui")]
    pub fn children(&self) -> &[usize] {
        &self.children
    }
}

pub struct Duvis {
    entries: Vec<Entry>,
    base_depth: usize,
}

impl Duvis {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            base_depth: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn entry(&self, idx: usize) -> &Entry {
        &self.entries[idx]
    }

    #[cfg(feature = "gui")]
    pub fn base_depth(&self) -> usize {
        self.base_depth
    }

    pub fn set_base_depth(&mut self, d: usize) {
        self.base_depth = d;
    }

    pub fn read_entries<R: BufRead>(&mut self, mut reader: R, zero: bool) -> io::Result<()> {
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
    pub fn sort_entries(&mut self) {
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

    /// Build tree from du's natural post-order output.
    /// Subtree of a direct child at index `i` is `[prev_child_end .. i + 1)`.
    pub fn build_tree_postorder(&mut self, start: usize, end: usize, depth: u32) {
        if start >= end {
            return;
        }
        let parent = end - 1;
        self.entries[parent].depth = depth;
        let parent_components = self.entries[parent].components.len();
        let child_components = parent_components + 1;

        let mut children: Vec<usize> = Vec::new();
        let mut subtree_start = start;
        let mut i = start;
        while i < end - 1 {
            let ic = self.entries[i].components.len();
            if ic == child_components {
                if self.entries[i].components[parent_components - 1]
                    != self.entries[parent].components[parent_components - 1]
                {
                    eprintln!("index {}: unexpected child", i + 1);
                    exit(1);
                }
                children.push(i);
                self.build_tree_postorder(subtree_start, i + 1, depth + 1);
                subtree_start = i + 1;
            } else if ic < child_components {
                eprintln!("index {}: unexpected entry at or above parent level", i + 1);
                exit(1);
            }
            i += 1;
        }
        self.entries[parent].children = children;
    }

    /// Build tree from lex-sorted (preorder-laid-out) input.
    pub fn build_tree_preorder(&mut self, start: usize, end: usize, depth: u32) {
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

        self.entries[start].children = children;
    }

    /// Sort every node's children by descending size, ties broken by leaf
    /// name. Runs as a single pass over all entries; the borrow checker
    /// is appeased by swapping the children list out for the duration of
    /// the sort so the comparator can read sizes from `self.entries`.
    pub fn sort_children_by_size(&mut self) {
        for idx in 0..self.entries.len() {
            let mut children = std::mem::take(&mut self.entries[idx].children);
            children.sort_by(|&a, &b| {
                let ea = &self.entries[a];
                let eb = &self.entries[b];
                eb.size
                    .cmp(&ea.size)
                    .then_with(|| ea.components.last().cmp(&eb.components.last()))
            });
            self.entries[idx].children = children;
        }
    }

    fn indent<W: Write>(out: &mut W, depth: u32) -> io::Result<()> {
        for _ in 0..(N_INDENT * depth as usize) {
            out.write_all(b" ")?;
        }
        Ok(())
    }

    pub fn show_entries<W: Write>(&self, out: &mut W, idx: usize, term: u8) -> io::Result<()> {
        let e = &self.entries[idx];
        if e.depth == 0 {
            out.write_all(e.components[0].as_bytes())?;
            for i in 1..self.base_depth {
                out.write_all(b"/")?;
                out.write_all(e.components[i].as_bytes())?;
            }
            write!(out, " {}", e.size)?;
            out.write_all(&[term])?;
        } else {
            Self::indent(out, e.depth)?;
            out.write_all(e.components.last().unwrap().as_bytes())?;
            write!(out, " {}", e.size)?;
            out.write_all(&[term])?;
        }
        for &c in &e.children {
            self.show_entries(out, c, term)?;
        }
        Ok(())
    }

    pub fn show_entries_raw<W: Write>(&self, out: &mut W, term: u8) -> io::Result<()> {
        for e in &self.entries {
            Self::indent(out, e.depth)?;
            out.write_all(e.components.last().unwrap().as_bytes())?;
            write!(out, " {}", e.size)?;
            out.write_all(&[term])?;
        }
        Ok(())
    }
}
