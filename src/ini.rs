//! Instruction file reader and writer.
//!
//! This lives in `kotor-formats` now, shared with the patcher that reads the
//! same files. The editor's version was a copy of the patcher's reader with a
//! writer added; keeping two was the same mistake the shared crate exists to
//! prevent, one level down from the formats.
//!
//! Re-exported here so every `crate::ini::…` path keeps working.
//!
//! See [`kotor_formats::ini`] for the documentation and the tests.

pub use kotor_formats::ini::*;
