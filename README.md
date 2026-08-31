# OdyEdit

Edits the `changes.ini` files that KotOR mod installers read.

A `changes.ini` tells a patcher which game files to modify and how. Writing one
by hand means remembering an undocumented key vocabulary and counting `AddRow0`,
`AddRow1`, `AddRow2` by eye. This program shows the same file as a set of forms.

It is a rewrite of [ChangeEdit](https://github.com/OpenKotOR/ChangeEdit), which
stopped at Delphi 7 and Windows. Its author called it "a textbook example of an
unplanned, unstructured hack of an application". The file format it edits
outlived it by twenty years.

## What it edits

| Section | What it does |
| --- | --- |
| Settings | Names the installer and decides what it checks before running |
| Dialog strings | Adds lines to the game's dialog table and gives them tokens |
| Tables | Adds rows and columns to `.2da` tables, or changes cells in them |
| Game files | Changes fields inside items, creatures, dialogue and the rest |
| Scripts | Compiles source scripts and installs the result |
| Files to install | Copies files into the game's folders |
| Soundsets | Points soundset entries at different dialog strings |
| Install variants | Offers more than one version of the mod |

Dialog strings are edited together with the `append.tlk` they point into,
because editing one without the other produces a mod that installs and then
shows blank text.

## Saving does not reformat

Open a file, save it, and you get the same bytes back. Comments, blank lines,
spacing and line endings all survive, and an edit rewrites only the lines it
actually changes. Mod authors keep these files in version control; a save that
reflowed the whole file would bury one real change in a thousand cosmetic ones.

Keys this editor does not recognise are carried through untouched rather than
dropped, so a file written by a newer patcher survives a round trip.

Two things are not left alone, because a patcher would otherwise read them
wrongly. A row identifier is lifted above the cells it applies to, and a
`2DAMEMORY` capture is pushed below them. Both are cases where the original
order makes the change silently do nothing.

## Build

```sh
cargo build --release
```

Needs Rust 1.77 or newer. The binary lands in `target/release/odyedit`, and
takes an optional filename:

```sh
odyedit path/to/changes.ini
```

## Depends on

- [`kotor-formats`](https://github.com/arrenkaetris/kotor-formats) for the game
  file formats — GFF, 2DA, TLK, SSF — and for byte-exact Latin-1 text.
- `eframe` and `egui` for the window, matching
  [OdyPatcher](https://github.com/oldrepublicwizard/OdyPatcher), the patcher that
  reads what this writes.

The INI layer is this repository's own. Off-the-shelf INI crates lose key order,
drop comments and normalise values, which is exactly what an editor for these
files must not do.

## Tests

```sh
cargo test
```

The round-trip suite runs over 84 real instruction files, taken from
OdyPatcher's own test suite, and checks three things of each: that opening and
saving changes nothing, that writing every section back twice gives the same
file as writing it once, and that a write never loses what the editor read.

## Status

Early. Every section has a working panel. `HACKList`, which patches compiled
scripts byte by byte, has no panel yet — files carrying one still load, save and
keep it intact.

## License

MIT. See [LICENSE](LICENSE).
