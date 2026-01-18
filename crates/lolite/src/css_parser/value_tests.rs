use crate::css_parser::parse_css;
use crate::style::{BoxSizing, Length, Selector};

#[test]
fn test_parse_lengths() {
    let css = r#"
        .length-test {
            width: 100px;
            height: 50em;
            margin: 10px 20px 30px 40px;
            padding: 5%;
        }

        .auto-test {
            width: auto;
            height: auto;
        }
    "#;

    let stylesheet = parse_css(css).expect("Failed to parse CSS");
    assert_eq!(stylesheet.rules.len(), 2);

    // All rules should parse successfully
    for rule in &stylesheet.rules {
        assert!(!rule.declarations.is_empty());
    }
}

#[test]
fn test_parse_box_sizing() {
    let css = r#"
        .a { box-sizing: border-box; }
        .b { box-sizing: content-box; }
    "#;

    let stylesheet = parse_css(css).expect("Failed to parse CSS");
    assert_eq!(stylesheet.rules.len(), 2);

    let a = &stylesheet.rules[0];
    assert_eq!(a.selector, Selector::Class("a".to_string()));
    assert!(a
        .declarations
        .iter()
        .any(|d| d.box_sizing == Some(BoxSizing::BorderBox)));

    let b = &stylesheet.rules[1];
    assert_eq!(b.selector, Selector::Class("b".to_string()));
    assert!(b
        .declarations
        .iter()
        .any(|d| d.box_sizing == Some(BoxSizing::ContentBox)));
}

#[test]
fn test_border_radius_parsing() {
    let css = r#"
        .simple-radius {
            border-radius: 10px;
        }

        .complex-radius {
            border-radius: 5px 10px 15px 20px;
        }

        .mixed-units {
            border-radius: 1em 50% 2px;
        }
    "#;

    let stylesheet = parse_css(css).expect("Failed to parse border-radius CSS");
    assert_eq!(stylesheet.rules.len(), 3);

    // All rules should parse successfully
    for rule in &stylesheet.rules {
        assert!(!rule.declarations.is_empty());
        // Check that at least one declaration has border_radius
        let has_border_radius = rule
            .declarations
            .iter()
            .any(|decl| decl.border_radius.is_some());
        assert!(has_border_radius, "Rule should have border-radius property");
    }
}

#[test]
fn test_parse_margin_sides() {
    let css = r#"
        .m {
            margin-left: 20px;
            margin-top: 10px;
            margin-right: 5px;
            margin-bottom: 15px;
        }

        .auto_left {
            margin-left: auto;
        }
    "#;

    let stylesheet = parse_css(css).expect("Failed to parse CSS");
    assert_eq!(stylesheet.rules.len(), 2);

    let m_rule = &stylesheet.rules[0];
    assert_eq!(m_rule.selector, Selector::Class("m".to_string()));
    assert!(m_rule
        .declarations
        .iter()
        .any(|d| matches!(d.margin.left, Some(Length::Px(20.0)))));
    assert!(m_rule
        .declarations
        .iter()
        .any(|d| matches!(d.margin.top, Some(Length::Px(10.0)))));
    assert!(m_rule
        .declarations
        .iter()
        .any(|d| matches!(d.margin.right, Some(Length::Px(5.0)))));
    assert!(m_rule
        .declarations
        .iter()
        .any(|d| matches!(d.margin.bottom, Some(Length::Px(15.0)))));

    let auto_rule = &stylesheet.rules[1];
    assert_eq!(auto_rule.selector, Selector::Class("auto_left".to_string()));
    assert!(auto_rule
        .declarations
        .iter()
        .any(|d| matches!(d.margin.left, Some(Length::Auto))));
}

#[test]
fn test_parse_padding_sides() {
    let css = r#"
        .p {
            padding-left: 20px;
            padding-top: 10px;
            padding-right: 5px;
            padding-bottom: 15px;
        }

        .auto_left {
            padding-left: auto;
        }
    "#;

    let stylesheet = parse_css(css).expect("Failed to parse CSS");
    assert_eq!(stylesheet.rules.len(), 2);

    let p_rule = &stylesheet.rules[0];
    assert_eq!(p_rule.selector, Selector::Class("p".to_string()));
    assert!(p_rule
        .declarations
        .iter()
        .any(|d| matches!(d.padding.left, Some(Length::Px(20.0)))));
    assert!(p_rule
        .declarations
        .iter()
        .any(|d| matches!(d.padding.top, Some(Length::Px(10.0)))));
    assert!(p_rule
        .declarations
        .iter()
        .any(|d| matches!(d.padding.right, Some(Length::Px(5.0)))));
    assert!(p_rule
        .declarations
        .iter()
        .any(|d| matches!(d.padding.bottom, Some(Length::Px(15.0)))));

    let auto_rule = &stylesheet.rules[1];
    assert_eq!(auto_rule.selector, Selector::Class("auto_left".to_string()));
    assert!(auto_rule
        .declarations
        .iter()
        .any(|d| matches!(d.padding.left, Some(Length::Auto))));
}
