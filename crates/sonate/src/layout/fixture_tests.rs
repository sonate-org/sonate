//! Runs the `sonate_layout` fixtures through Sonate's own HTML loader, CSS parser, cascade and
//! layout, and compares the boxes with Chrome's snapshots.
//!
//! `FIXTURE_FILTER=nested/ cargo test -p sonate layout_fixtures` runs a subset.

use super::test_html::load_layout_fixture;
use sonate_layout::testing::harness::{self, Suite};
use sonate_layout::testing::snapshot::{Rect4, SnapshotCase};
use std::path::Path;

#[test]
fn layout_fixtures() {
    let known_failures =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/layout/fixture_known_failures.txt");
    let suite = Suite::with_layout_fixtures("sonate", known_failures);
    let filter = std::env::var("FIXTURE_FILTER").ok();

    let engine = |fixture: &harness::Fixture, case: &SnapshotCase| Ok(layout_case(&fixture.html, case));
    match harness::run_all(&suite, &engine, filter.as_deref()) {
        Ok(summary) => println!("{summary}"),
        Err(failures) => panic!("{failures}"),
    }
}

fn layout_case(html: &str, case: &SnapshotCase) -> Vec<Rect4> {
    let (ctx, ids) = load_layout_fixture(html, &case.overrides);
    ids.iter()
        .map(|&id| {
            let node = ctx.document.get_node(id).expect("fixture node");
            let node = node.borrow();
            let parent = node
                .parent
                .and_then(|parent| ctx.document.get_node(parent))
                .map(|parent| parent.borrow().layout.bounds)
                .unwrap_or_default();
            let bounds = node.layout.bounds;
            [
                bounds.x - parent.x,
                bounds.y - parent.y,
                bounds.width,
                bounds.height,
            ]
        })
        .collect()
}
