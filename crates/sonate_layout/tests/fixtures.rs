//! Compares the engine with Chrome on every fixture in `tests/fixtures`.
//!
//! `cargo test -p sonate_layout --test fixtures -- nested/` runs a subset.

use sonate_layout::testing::{engine, harness};
use std::path::Path;
use std::sync::Arc;

fn main() -> std::process::ExitCode {
    let known_failures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/known_failures.txt");
    let suite = harness::Suite::with_layout_fixtures("sonate_layout", known_failures);
    harness::run_main(suite, Arc::new(|_fixture, case| engine::layout_case(case)))
}
