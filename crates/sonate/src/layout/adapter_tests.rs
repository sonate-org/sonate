//! Behaviour of the `sonate_layout` adapter that the HTML fixtures can't cover.

use super::test_html::load_html_test_example;
use super::*;

#[test]
fn whitespace_only_text_does_not_become_a_flex_item() {
    let mut ctx = LayoutContext::new();
    ctx.style_sheet = crate::css_parser::parse_css(".row { column-gap: 10px; }").unwrap();
    let root = ctx.document.root_id();
    let row = ctx.document.create_node(Id::from_u64(1), None);
    let space = ctx
        .document
        .create_node(Id::from_u64(2), Some("   ".to_owned()));
    let text = ctx
        .document
        .create_node(Id::from_u64(3), Some("Ab".to_owned()));
    ctx.document
        .set_attribute(row, "class".to_owned(), "row".to_owned());
    ctx.document.set_parent(root, row).unwrap();
    ctx.document.set_parent(row, space).unwrap();
    ctx.document.set_parent(row, text).unwrap();

    ctx.layout();

    // Without the whitespace item there is no gap before the text.
    let bounds = ctx.document.get_node(text).unwrap().borrow().layout.bounds;
    assert_eq!(bounds.x, 0.0);
}

#[test]
fn display_none_subtrees_are_not_rendered() {
    const HTML: &str = r#"
<div id="example" style="width: 100px; height: 100px">
  <div id="hidden" style="display: none"><div id="inner">Hidden text</div></div>
  <div id="shown">Shown</div>
</div>
"#;
    let (mut ctx, ids) = load_html_test_example(HTML, "example");
    ctx.layout();

    let snapshot = build_render_snapshot(ctx.document.root_node());
    assert!(snapshot.root.find_node_by_id(ids["hidden"]).is_none());
    assert!(snapshot.root.find_node_by_id(ids["inner"]).is_none());
    assert!(snapshot.root.find_node_by_id(ids["shown"]).is_some());
}

#[test]
fn nested_scroll_offsets_add_up_and_are_clamped() {
    const HTML: &str = r#"
<div id="example" style="width: 100px; height: 100px; overflow: scroll; flex-direction: column">
  <div id="inner" style="height: 200px; flex-shrink: 0; overflow: scroll; flex-direction: column">
    <div id="content" style="height: 300px; flex-shrink: 0"></div>
  </div>
</div>
"#;
    let (mut ctx, ids) = load_html_test_example(HTML, "example");
    ctx.scroll_state
        .insert(ids["example"], ScrollState::new(0.0, 30.0));
    ctx.scroll_state
        .insert(ids["inner"], ScrollState::new(0.0, 500.0));
    ctx.layout();

    let y = |name: &str| {
        ctx.document
            .get_node(ids[name])
            .unwrap()
            .borrow()
            .layout
            .bounds
            .y
    };
    assert_eq!(y("inner"), -30.0);
    // The inner offset is clamped to its overflow (300 - 200).
    assert_eq!(ctx.scroll_state[&ids["inner"]].scroll_y.get(), 100.0);
    assert_eq!(y("content"), -130.0);
}
