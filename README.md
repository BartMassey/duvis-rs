# `duvis` - a `du` visualizer
Copyright © 2014 Bart Massey

## Rationale

I constructed `duvis` to take the place of the standard
`xdu(1)` for visualizing `du(1)` disk usage output. There
are a couple of reasons for replacing `xdu`:

1. In 2014 `xdu` is just too slow. I'm not sure when it
   would have completed on the 5.7M lines of `du` output for
   one of my (smaller) machines, but a half-hour didn't seem
   to do it. The core algorithms used in `xdu` are quite
   inefficient, and the use of storage is not good.

2. It's neat that `xdu` is an X Window System visualization.
   Sadly, though, I often would really prefer ASCII art for
   portability: I don't need the graphics, and being able to
   work with the output in my text editor is rather sweet.

3. The visualization `xdu` provides isn't very well matched
   to my normal task: finding things to archive or delete
   from large systems.

The standard `duvis` visualization is produced quickly, is ASCII, 
and works acceptably well for its target use case.

## Usage

As with `xdu`, you invoke `duvis` on the output of `du`;
currently the `du` output is read from standard input, so
either a pipe or a file is fine. The `du` output must be
complete, in the sense that every prefix of every path in
the file has an entry (with the exception of the common
prefix that was given to `du`); both relative and absolute
paths work.

The output of `duvis` is the paths that were input, with
only the last component shown except at the root, indented
according to nesting depth, and sorted at each level by
decreasing size, with ties broken alphabetically.

See `duvis(1)` or `duvis --help` for the full set of
command-line options.

## Dependencies

The graphical frontend (`-g`) needs GTK4 and Cairo
development headers. On Debian/Ubuntu:

```
sudo apt install libgtk-4-dev libcairo2-dev
```

`GTK` is the backend utilized by `Cairo` to draw all graphics.

The GUI is gated behind a default-on `gui` Cargo feature.
To build without GTK at all:

```
cargo build --release --no-default-features
```

This produces a CLI-only binary; the `-g` flag is removed
from the help output and from the parser.

## History

The original `duvis` was written in C by Bart Massey
starting sometime before 2014. Upstream repo is
<https://github.com/BartMassey/duvis>.

Andrew Graham picked it up as a student project in
mid-2014. His main lasting addition was a GTK-based
graphical frontend, broken out into its own translation unit
in 2016. Upstream repo is
<https://github.com/andeh575/duvis>.

This repository is a 2026 Rust port. The port follows the
design of the original `duvis-bart` code — direct ingestion
of `du(1)`'s natural post-order output, with the size-sort
done at display time so the `-p` and default output are
identical on well-formed input. The GTK graphical frontend
has been ported to GTK4/Cairo as a fairly literal
translation of Andrew Graham's original.

## Acknowledgements

Thanks to Andrew Graham for his work on the original C
implementation, in particular the GTK graphical frontend.

## License

This program is licensed under the "MIT License".  Please
see the file `LICENSE.txt` in the source distribution of
this software for license terms.
