//! Tests of the public API that the HTML fixtures can't express.

use sonate_layout::{
    compute_layout, AvailableSpace, Dimension, FlexDirection, LayoutTree, LengthPercentage,
    MeasureInput, MeasureOutput, NodeId, Size, Style,
};

fn viewport() -> Size<AvailableSpace> {
    Size::new(AvailableSpace::Definite(800.0), AvailableSpace::Definite(600.0))
}

/// Text-like leaves: 10px per character, wrapping only at the available width.
fn measure_chars(input: MeasureInput) -> MeasureOutput {
    let width = match input.available.width {
        AvailableSpace::Definite(width) => width.min(100.0),
        AvailableSpace::MinContent => 30.0,
        AvailableSpace::MaxContent => 100.0,
    };
    let width = input.known.width.unwrap_or(width);
    MeasureOutput {
        size: Size::new(width, input.known.height.unwrap_or((100.0 / width.max(1.0)).ceil() * 10.0)),
        first_baseline: Some(8.0),
    }
}

/// Alternating rows and columns, each level holding a measured leaf and the next level.
fn nested_tree(tree: &mut LayoutTree, depth: usize) -> NodeId {
    let mut child = None;
    for level in 0..depth {
        let leaf = tree.push(Style::default(), &[], true);
        let mut children = vec![leaf];
        children.extend(child);
        let style = Style {
            flex_direction: if level % 2 == 0 {
                FlexDirection::Row
            } else {
                FlexDirection::Column
            },
            padding: sonate_layout::Edges::all(LengthPercentage::Px(1.0)),
            flex_grow: 1.0,
            ..Style::default()
        };
        child = Some(tree.push(style, &children, false));
    }
    child.unwrap()
}

fn measure_calls(depth: usize) -> usize {
    let mut tree = LayoutTree::new();
    let root = nested_tree(&mut tree, depth);
    let mut calls = 0;
    compute_layout(&mut tree, root, viewport(), &mut |_, input| {
        calls += 1;
        measure_chars(input)
    });
    calls
}

#[test]
fn deep_nesting_does_not_remeasure_exponentially() {
    // A parent measures each child several times (min-content, max-content, at its flexed
    // size) before laying it out. Without effective caching the work multiplies at every
    // level; with it, doubling the depth roughly doubles the work.
    let calls: Vec<usize> = [10, 20, 40, 80].iter().map(|&depth| measure_calls(depth)).collect();
    for pair in calls.windows(2) {
        assert!(pair[1] <= pair[0] * 6, "measure calls per depth 10/20/40/80: {calls:?}");
    }
    assert!(calls[3] < 80 * 40, "measure calls per depth 10/20/40/80: {calls:?}");
}

#[test]
fn row_with_fixed_and_growing_items() {
    let mut tree = LayoutTree::new();
    let fixed = tree.push(
        Style {
            size: Size::new(Dimension::Px(100.0), Dimension::Px(50.0)),
            ..Style::default()
        },
        &[],
        false,
    );
    let growing = tree.push(
        Style {
            flex_grow: 1.0,
            ..Style::default()
        },
        &[],
        false,
    );
    let root = tree.push(
        Style {
            size: Size::new(Dimension::Px(400.0), Dimension::Px(200.0)),
            ..Style::default()
        },
        &[fixed, growing],
        false,
    );
    compute_layout(&mut tree, root, viewport(), &mut |_, input| measure_chars(input));

    assert_eq!(tree.layout(root).size, Size::new(400.0, 200.0));
    assert_eq!(tree.layout(fixed).size, Size::new(100.0, 50.0));
    assert_eq!(tree.layout(growing).location.x, 100.0);
    assert_eq!(tree.layout(growing).size, Size::new(300.0, 200.0));
}

#[test]
fn root_without_size_fills_the_viewport() {
    let mut tree = LayoutTree::new();
    let root = tree.push(Style::default(), &[], false);
    compute_layout(&mut tree, root, viewport(), &mut |_, input| measure_chars(input));
    assert_eq!(tree.layout(root).size, Size::new(800.0, 600.0));
}
