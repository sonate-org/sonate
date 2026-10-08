//! Fixture harness shared by this crate's tests and Sonate's integration suite.
//!
//! Fixtures live in `tests/fixtures/**/*.html`; `tools/bless` records Chrome's layout of each
//! fixture in a `.snap.json` next to it. See `tests/fixtures/README.md`.

pub mod ahem;
pub mod calc_parser;
pub mod engine;
pub mod harness;
pub mod snapshot;
