# `duvis` - a `du` visualizer
Copyright © 2014 Bart Massey

## History

The original `duvis` was written in C by Bart Massey
starting 2014-03-30. Upstream:
<https://github.com/BartMassey/duvis>.

Andrew Graham picked it up as a student project in mid-2014
(first commit 2014-05-21). His main lasting addition was a
GTK-based graphical frontend, broken out into its own
translation unit in December 2016. Upstream:
<https://github.com/andeh575/duvis>.

This repository is a Rust port begun in 2026. The port
follows the design of the `duvis-bart` line — direct
ingestion of `du(1)`'s natural post-order output, with the
size-sort done at display time so the `-p` and default
output are identical on well-formed input. The graphical
frontend has not yet been ported; `-g` is recognized but
stubbed.

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

## Options

1. -p    Output in preorder format
2. -g    Output to `xdu` style graphical user interface

## Dependencies

In order to properly display any graphical portion of `duvis`
it it necessary to install the following packages:

1. GTK+-3.0: libgtk-3-dev
2. Cairo: cairo2-dev

`GTK` is the backend utilized by `Cairo` to draw all graphics.

## Acknowledgements

Thanks to Andrew Graham for his work on the original C
implementation, in particular the GTK graphical frontend.

## License

This program is licensed under the "MIT License".  Please
see the file `COPYING` in the source distribution of this
software for license terms.
