use crate::css_parser::parse_css;
use crate::style::{BorderStyle, Length, Selector};

#[test]
fn test_parse_border_shorthand_width_and_color() {
    let css = r#"
        .btn {
            border: 2px solid #0056b3;
        }
    "#;

    let stylesheet = parse_css(css).expect("Failed to parse CSS");
    assert_eq!(stylesheet.rules.len(), 1);

    let rule = &stylesheet.rules[0];
    assert_eq!(rule.selector, Selector::Class("btn".to_string()));

    let mut found_width = false;
    let mut found_color = false;
    let mut found_style = false;

    for declaration in &rule.declarations {
        if let Some(width) = declaration.border_width.top {
            // 2px
            found_width = matches!(
                width,
                crate::style::Length::Px(v) if (v - 2.0).abs() < f64::EPSILON
            );
        }
        if let Some(c) = declaration.border_color.top {
            found_color = c.r == 0x00 && c.g == 0x56 && c.b == 0xB3 && c.a == 0xFF;
        }
        if let Some(style) = declaration.border_style.top {
            found_style = style == BorderStyle::Solid;
        }
    }

    assert!(found_width, "Expected border width from shorthand");
    assert!(found_color, "Expected border color from shorthand");
    assert!(found_style, "Expected border style from shorthand");
}

#[test]
fn test_parse_border_shorthand_none() {
    let css = r#"
        .noborder {
            border: none;
        }
    "#;

    let stylesheet = parse_css(css).expect("Failed to parse CSS");
    assert_eq!(stylesheet.rules.len(), 1);

    let rule = &stylesheet.rules[0];
    assert_eq!(rule.selector, Selector::Class("noborder".to_string()));

    // Also ensure we did parse the style keyword.
    let mut saw_none_style = false;
    for declaration in &rule.declarations {
        if matches!(declaration.border_style.top, Some(BorderStyle::None)) {
            saw_none_style = true;
        }
        assert!(
            declaration.border_width.top.is_none()
                && declaration.border_width.right.is_none()
                && declaration.border_width.bottom.is_none()
                && declaration.border_width.left.is_none(),
            "Expected border: none to not force border_width"
        );
    }
    assert!(saw_none_style, "Expected border: none to set border_style");
}

#[test]
fn test_parse_border_shorthand_rejects_duplicate_width() {
    let css = r#"
        .dup {
            width: 10px;
            border: 1px 2px solid red;
        }
    "#;

    let stylesheet = parse_css(css).expect("Failed to parse CSS");
    assert_eq!(stylesheet.rules.len(), 1);

    let rule = &stylesheet.rules[0];
    assert_eq!(rule.selector, Selector::Class("dup".to_string()));

    let mut saw_width_decl = false;
    let mut saw_any_border_field = false;

    for declaration in &rule.declarations {
        if matches!(declaration.width, Some(Length::Px(v)) if (v - 10.0).abs() < f64::EPSILON) {
            saw_width_decl = true;
        }
        if declaration.border_width.top.is_some()
            || declaration.border_width.right.is_some()
            || declaration.border_width.bottom.is_some()
            || declaration.border_width.left.is_some()
            || declaration.border_color.top.is_some()
            || declaration.border_color.right.is_some()
            || declaration.border_color.bottom.is_some()
            || declaration.border_color.left.is_some()
            || declaration.border_style.top.is_some()
            || declaration.border_style.right.is_some()
            || declaration.border_style.bottom.is_some()
            || declaration.border_style.left.is_some()
        {
            saw_any_border_field = true;
        }
    }

    assert!(
        saw_width_decl,
        "Expected the valid width declaration to parse"
    );
    assert!(
        !saw_any_border_field,
        "Expected invalid border shorthand (duplicate width) to be rejected"
    );
}

#[test]
fn test_parse_border_shorthand_rejects_duplicate_color() {
    let css = r#"
        .dup {
            width: 10px;
            border: 1px solid red blue;
        }
    "#;

    let stylesheet = parse_css(css).expect("Failed to parse CSS");
    assert_eq!(stylesheet.rules.len(), 1);

    let rule = &stylesheet.rules[0];
    assert_eq!(rule.selector, Selector::Class("dup".to_string()));

    let mut saw_width_decl = false;
    let mut saw_any_border_field = false;

    for declaration in &rule.declarations {
        if matches!(declaration.width, Some(Length::Px(v)) if (v - 10.0).abs() < f64::EPSILON) {
            saw_width_decl = true;
        }
        if declaration.border_width.top.is_some()
            || declaration.border_width.right.is_some()
            || declaration.border_width.bottom.is_some()
            || declaration.border_width.left.is_some()
            || declaration.border_color.top.is_some()
            || declaration.border_color.right.is_some()
            || declaration.border_color.bottom.is_some()
            || declaration.border_color.left.is_some()
            || declaration.border_style.top.is_some()
            || declaration.border_style.right.is_some()
            || declaration.border_style.bottom.is_some()
            || declaration.border_style.left.is_some()
        {
            saw_any_border_field = true;
        }
    }

    assert!(
        saw_width_decl,
        "Expected the valid width declaration to parse"
    );
    assert!(
        !saw_any_border_field,
        "Expected invalid border shorthand (duplicate color) to be rejected"
    );
}

#[test]
fn test_parse_border_shorthand_rejects_duplicate_style() {
    let css = r#"
        .dup {
            width: 10px;
            border: 1px solid dashed red;
        }
    "#;

    let stylesheet = parse_css(css).expect("Failed to parse CSS");
    assert_eq!(stylesheet.rules.len(), 1);

    let rule = &stylesheet.rules[0];
    assert_eq!(rule.selector, Selector::Class("dup".to_string()));

    let mut saw_width_decl = false;
    let mut saw_any_border_field = false;

    for declaration in &rule.declarations {
        if matches!(declaration.width, Some(Length::Px(v)) if (v - 10.0).abs() < f64::EPSILON) {
            saw_width_decl = true;
        }
        if declaration.border_width.top.is_some()
            || declaration.border_width.right.is_some()
            || declaration.border_width.bottom.is_some()
            || declaration.border_width.left.is_some()
            || declaration.border_color.top.is_some()
            || declaration.border_color.right.is_some()
            || declaration.border_color.bottom.is_some()
            || declaration.border_color.left.is_some()
            || declaration.border_style.top.is_some()
            || declaration.border_style.right.is_some()
            || declaration.border_style.bottom.is_some()
            || declaration.border_style.left.is_some()
        {
            saw_any_border_field = true;
        }
    }

    assert!(
        saw_width_decl,
        "Expected the valid width declaration to parse"
    );
    assert!(
        !saw_any_border_field,
        "Expected invalid border shorthand (duplicate style) to be rejected"
    );
}

#[test]
fn test_parse_border_style_property() {
    let css = r#"
        .styled {
            border-style: dashed;
        }
    "#;

    let stylesheet = parse_css(css).expect("Failed to parse CSS");
    assert_eq!(stylesheet.rules.len(), 1);

    let rule = &stylesheet.rules[0];
    assert_eq!(rule.selector, Selector::Class("styled".to_string()));

    let mut saw_dashed = false;
    for declaration in &rule.declarations {
        if matches!(declaration.border_style.top, Some(BorderStyle::Dashed)) {
            saw_dashed = true;
        }
    }

    assert!(saw_dashed, "Expected border-style to parse as dashed");
}

#[test]
fn test_parse_border_side_longhands() {
    let css = r#"
        .sides {
            border-top-color: #ff0000;
            border-right-style: dotted;
            border-bottom-width: thick;
            border-left-width: 2px;
        }
    "#;

    let stylesheet = parse_css(css).expect("Failed to parse CSS");
    assert_eq!(stylesheet.rules.len(), 1);

    let rule = &stylesheet.rules[0];
    assert_eq!(rule.selector, Selector::Class("sides".to_string()));

    let mut saw_top_color = false;
    let mut saw_right_style = false;
    let mut saw_bottom_width = false;
    let mut saw_left_width = false;

    for declaration in &rule.declarations {
        if let Some(c) = declaration.border_color.top {
            saw_top_color = c.r == 0xFF && c.g == 0x00 && c.b == 0x00 && c.a == 0xFF;
        }
        if let Some(s) = declaration.border_style.right {
            saw_right_style = s == BorderStyle::Dotted;
        }
        if let Some(w) = declaration.border_width.bottom {
            saw_bottom_width = matches!(w, Length::Px(v) if (v - 5.0).abs() < f64::EPSILON);
        }
        if let Some(w) = declaration.border_width.left {
            saw_left_width = matches!(w, Length::Px(v) if (v - 2.0).abs() < f64::EPSILON);
        }

        // Ensure we didn't accidentally set other sides for these.
        assert!(declaration.border_color.right.is_none());
        assert!(declaration.border_color.bottom.is_none());
        assert!(declaration.border_color.left.is_none());
        assert!(declaration.border_style.top.is_none());
        assert!(declaration.border_style.bottom.is_none());
        assert!(declaration.border_style.left.is_none());
        assert!(declaration.border_width.top.is_none());
        assert!(declaration.border_width.right.is_none());
    }

    assert!(saw_top_color);
    assert!(saw_right_style);
    assert!(saw_bottom_width);
    assert!(saw_left_width);
}
