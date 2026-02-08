use crate::layout::test_html::load_html_test_example;
use crate::style::Length;
use crate::style::Selector;

#[test]
fn tag_selectors_apply_to_elements() {
    const HTML: &str = r#"
<style>
  INPUT { border-width: 2px; }
</style>
<div id="example">
  <input id="x" />
</div>
"#;

    let (ctx, nodes_by_id) = load_html_test_example(HTML, "example");
    let x = nodes_by_id.get("x").copied().expect("missing node x");

    assert!(
        ctx.style_sheet
            .rules
            .iter()
            .any(|r| r.selector == Selector::Tag("input".to_owned())),
        "expected a tag rule for input"
    );

    let node = ctx.document.get_node(x).expect("node not found");
    assert_eq!(
        node.borrow().attributes.get("tag").map(|s| s.as_str()),
        Some("input")
    );
    let style = node.borrow().layout.style.clone();

    assert_eq!(style.border_width.top, Some(Length::Px(2.0)));
    assert_eq!(style.border_width.right, Some(Length::Px(2.0)));
    assert_eq!(style.border_width.bottom, Some(Length::Px(2.0)));
    assert_eq!(style.border_width.left, Some(Length::Px(2.0)));
}
