use crate::css_parser::parse_css;
use crate::style::{Display, FlexDirection, Length, Style, Widget};

#[test]
fn test_parse_simple_css_document() {
    let css = r#"
        .container {
            display: flex;
            flex-direction: column;
            justify-content: center;
            align-items: stretch;
            background-color: #f0f0f0;
            width: 300px;
            height: 200px;
            padding: 20px;
            margin: 10px;
        }

        .box {
            background-color: red;
            width: 100px;
            height: 50px;
            margin: 5px;
            flex-grow: 1;
            flex-shrink: 0;
        }

        button {
            background-color: #007bff;
            border-width: 2px;
            border-color: #0056b3;
            padding: 8px;
            margin: 4px;
        }
    "#;

    let stylesheet = parse_css(css).expect("Failed to parse CSS");

    // Should have 3 rules
    assert_eq!(stylesheet.rules.len(), 3);

    // Test first rule (.container)
    let container_rule = &stylesheet.rules[0];
    assert_eq!(container_rule.selectors.to_string(), ".container");
    assert!(!container_rule.declarations.is_empty());

    // Check that we have multiple declarations for the container
    let mut found_display = false;
    let mut found_background = false;
    let mut found_width = false;

    for declaration in &container_rule.declarations {
        if declaration.display == Some(Display::Flex) {
            found_display = true;
        }
        if declaration.background_color.is_some() {
            found_background = true;
        }
        if declaration.width.is_some() {
            found_width = true;
        }
    }

    assert!(found_display, "Should have found display: flex");
    assert!(found_background, "Should have found background-color");
    assert!(found_width, "Should have found width");

    // Test second rule (.box)
    let box_rule = &stylesheet.rules[1];
    assert_eq!(box_rule.selectors.to_string(), ".box");
    assert!(!box_rule.declarations.is_empty());

    // Test third rule (button)
    let button_rule = &stylesheet.rules[2];
    assert_eq!(button_rule.selectors.to_string(), "button");
    assert!(!button_rule.declarations.is_empty());
}

#[test]
fn test_parse_flex_properties() {
    let css = r#"
        .flex-container {
            display: flex;
            flex-direction: row;
            justify-content: space-between;
            align-items: center;
            flex-wrap: wrap;
            gap: 15px;
        }
    "#;

    let stylesheet = parse_css(css).expect("Failed to parse CSS");
    assert_eq!(stylesheet.rules.len(), 1);

    let rule = &stylesheet.rules[0];
    assert_eq!(rule.selectors.to_string(), ".flex-container");

    // Verify we can parse all the flex properties
    assert!(!rule.declarations.is_empty());
}

#[test]
fn test_parse_invalid_css_gracefully() {
    let css = r#"
        .valid {
            display: flex;
            background-color: red;
        }

        .invalid {
            unknown-property: some-value;
            display: invalid-value;
        }

        .also-valid {
            width: 100px;
        }
    "#;

    // Should not panic, even with invalid properties
    let result = parse_css(css);
    assert!(result.is_ok());

    let stylesheet = result.unwrap();
    // Should still parse the valid rules
    assert!(!stylesheet.rules.is_empty());
}

#[test]
fn test_complex_selectors() {
    let css = r#"
        .main-container {
            display: flex;
            justify-content: space-evenly;
            align-content: space-around;
        }

        div {
            background-color: blue;
        }

        .sidebar {
            flex-basis: 200px;
            align-self: flex-end;
        }
    "#;

    let stylesheet = parse_css(css).expect("Failed to parse CSS");
    assert_eq!(stylesheet.rules.len(), 3);

    assert_eq!(stylesheet.rules[0].selectors.to_string(), ".main-container");
    assert_eq!(stylesheet.rules[1].selectors.to_string(), "div");
    assert_eq!(stylesheet.rules[2].selectors.to_string(), ".sidebar");
}

#[test]
fn test_empty_css() {
    let css = "";
    let stylesheet = parse_css(css).expect("Failed to parse empty CSS");
    assert_eq!(stylesheet.rules.len(), 0);
}

#[test]
fn test_whitespace_only_css() {
    let css = "   \n\t  \n  ";
    let stylesheet = parse_css(css).expect("Failed to parse whitespace CSS");
    assert_eq!(stylesheet.rules.len(), 0);
}

#[test]
fn test_single_rule_css() {
    let css = ".single { display: flex; }";
    let stylesheet = parse_css(css).expect("Failed to parse single rule CSS");
    assert_eq!(stylesheet.rules.len(), 1);
    assert_eq!(stylesheet.rules[0].selectors.to_string(), ".single");
}

#[test]
fn test_parse_text_input_widget_property() {
    let css = r#"
        input {
            -sonate-widget: text-input;
        }
    "#;

    let stylesheet = parse_css(css).expect("Failed to parse text input widget CSS");
    assert_eq!(stylesheet.rules.len(), 1);

    let declarations = &stylesheet.rules[0].declarations;
    assert!(declarations
        .iter()
        .any(|declaration| declaration.widget == Some(Widget::TextInput)));
}

#[test]
fn test_parse_none_widget_property() {
    let css = r#"
        input {
            -sonate-widget: none;
        }
    "#;

    let stylesheet = parse_css(css).expect("Failed to parse none widget CSS");
    assert_eq!(stylesheet.rules.len(), 1);

    let declarations = &stylesheet.rules[0].declarations;
    assert!(declarations
        .iter()
        .any(|declaration| declaration.widget == Some(Widget::None)));
}

#[test]
fn test_parse_inline_style() {
    let declarations = crate::css_parser::parse_inline_style("width: 50px; flex-direction: column");
    let mut style = Style::default();
    for declaration in &declarations {
        style.merge(declaration);
    }

    assert_eq!(style.width, Some(Length::Px(50.0)));
    assert!(matches!(style.flex_direction, Some(FlexDirection::Column)));
}

#[test]
fn test_parse_flex_shorthand() {
    let parse = |value: &str| {
        let mut style = Style::default();
        for declaration in crate::css_parser::parse_inline_style(&format!("flex: {value}")) {
            style.merge(&declaration);
        }
        (style.flex_grow, style.flex_shrink, style.flex_basis)
    };

    assert_eq!(
        parse("1"),
        (Some(1.0), Some(1.0), Some(Length::Percent(0.0)))
    );
    assert_eq!(
        parse("2 3"),
        (Some(2.0), Some(3.0), Some(Length::Percent(0.0)))
    );
    assert_eq!(
        parse("1 1 50px"),
        (Some(1.0), Some(1.0), Some(Length::Px(50.0)))
    );
    assert_eq!(
        parse("100px"),
        (Some(1.0), Some(1.0), Some(Length::Px(100.0)))
    );
    assert_eq!(parse("auto"), (Some(1.0), Some(1.0), Some(Length::Auto)));
    assert_eq!(parse("none"), (Some(0.0), Some(0.0), Some(Length::Auto)));
}

#[test]
fn test_parse_layout_properties() {
    let mut style = Style::default();
    let css = "display: none; position: absolute; inset: 1px 2px; left: 5%; min-width: 10px; \
               max-width: none; max-height: 50px; flex-flow: column wrap; gap: 4px 8px; font-size: 20px";
    for declaration in crate::css_parser::parse_inline_style(css) {
        style.merge(&declaration);
    }

    assert_eq!(style.display, Some(Display::None));
    assert_eq!(style.position, Some(crate::style::Position::Absolute));
    assert_eq!(style.inset.top, Some(Length::Px(1.0)));
    assert_eq!(style.inset.right, Some(Length::Px(2.0)));
    assert!(matches!(style.inset.left, Some(Length::Percent(p)) if (p - 5.0).abs() < 1e-4));
    assert_eq!(style.min_width, Some(Length::Px(10.0)));
    assert_eq!(style.max_width, Some(Length::Auto));
    assert_eq!(style.max_height, Some(Length::Px(50.0)));
    assert!(matches!(style.flex_direction, Some(FlexDirection::Column)));
    assert!(matches!(
        style.flex_wrap,
        Some(crate::style::FlexWrap::Wrap)
    ));
    assert_eq!(
        (style.row_gap, style.column_gap),
        (Some(Length::Px(4.0)), Some(Length::Px(8.0)))
    );
    assert_eq!(style.font_size, Some(Length::Px(20.0)));
}
