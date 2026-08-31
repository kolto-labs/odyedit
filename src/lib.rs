//! Editor for the `changes.ini` files that drive KotOR mod installers.
//!
//! A `changes.ini` tells a patcher which game files to modify and how. Writing
//! one by hand means remembering an undocumented key vocabulary and counting
//! `AddRow0`, `AddRow1`, `AddRow2` by eye. This program presents the same file
//! as a set of forms.
//!
//! The layers, bottom up:
//!
//! - [`ini`] reads and writes the file itself, keeping untouched lines exactly
//!   as they arrived.
//! - [`model`] gives the key vocabulary names and types — settings, and the
//!   seven kinds of instruction list.
//! - `ui` (behind the `gui` feature) is the window.
//!
//! The game file formats the editor has to open to offer sensible choices —
//! reading `spells.2da` to list its columns, reading `dialog.tlk` to show what
//! a `StrRef` points at — come from `kotor-formats`.

pub mod ini;
pub mod model;

#[cfg(feature = "gui")]
pub mod ui;
