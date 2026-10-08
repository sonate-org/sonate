use super::selectors::Nth;
use crate::css_parser::{parse_css, SelectorList};

fn parse(selector: &str) -> Option<SelectorList> {
    let sheet = parse_css(&format!("{selector} {{ color: red; }}")).ok()?;
    sheet.rules.into_iter().next().map(|rule| rule.selectors)
}

#[test]
fn parses_and_prints_complex_selectors() {
    for (input, printed) in [
        ("div", "div"),
        ("DIV.Card", "div.Card"),
        ("ul  >  li + li ~ p", "ul > li + li ~ p"),
        (".a .b", ".a .b"),
        ("#x.y[data-a='b' i]:hover", "#x.y[data-a=\"b\" i]:hover"),
        ("a, b > c", "a, b > c"),
        (":nth-child(2n+1 of .x)", ":nth-child(2n+1 of .x)"),
        (":not(.a, .b):is(p)", ":not(.a, .b):is(p)"),
    ] {
        assert_eq!(parse(input).map(|s| s.to_string()).as_deref(), Some(printed), "{input}");
    }
}

#[test]
fn rejects_invalid_selectors() {
    for input in [".a::before", ":unknown", ".a >", "[a=]", ".", "a,", "> a", ":nth-child(x)"] {
        assert!(parse(input).is_none(), "{input} should be invalid");
    }
}

#[test]
fn specificity() {
    for (input, expected) in [
        ("*", (0, 0, 0)),
        ("li", (0, 0, 1)),
        ("ul li", (0, 0, 2)),
        (".a", (0, 1, 0)),
        ("#x", (1, 0, 0)),
        ("a[href]:hover", (0, 2, 1)),
        (":not(#x .y)", (1, 1, 0)),
        (":is(.a, #b)", (1, 0, 0)),
        (":where(#x) .a", (0, 1, 0)),
        (":nth-child(2 of .a)", (0, 2, 0)),
    ] {
        let selectors = parse(input).unwrap();
        assert_eq!(selectors.0[0].specificity(), expected, "{input}");
    }
}

#[test]
fn nth_matches() {
    let positions = |nth: Nth| (1..=10).filter(|&p| nth.matches(p)).collect::<Vec<_>>();
    assert_eq!(positions(Nth { a: 2, b: 1 }), [1, 3, 5, 7, 9]);
    assert_eq!(positions(Nth { a: 2, b: 0 }), [2, 4, 6, 8, 10]);
    assert_eq!(positions(Nth { a: -1, b: 3 }), [1, 2, 3]);
    assert_eq!(positions(Nth { a: 0, b: 5 }), [5]);
    assert_eq!(positions(Nth { a: 3, b: -1 }), [2, 5, 8]);
}

#[test]
fn important_and_keywords_parse() {
    let sheet = parse_css(".a { color: red !important; width: inherit; height: unset !important; }").unwrap();
    let declarations = &sheet.rules[0].declarations;
    assert!(declarations[0].important);
    assert!(declarations[0].color.is_some());
    assert_eq!(declarations[1].keyword.as_ref().map(|(p, _)| p.as_str()), Some("width"));
    assert!(!declarations[1].important);
    assert!(declarations[2].important && declarations[2].keyword.is_some());

    // Unknown properties are dropped even with a CSS-wide keyword.
    let sheet = parse_css(".a { not-a-property: inherit; }").unwrap();
    assert!(sheet.rules[0].declarations.is_empty());
}
