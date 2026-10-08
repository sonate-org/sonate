use crate::css_parser::parse_declarations;
use crate::style::{BorderStyle, Length, Rgba, Style, TransformFunction};

/// The style of a declaration list, later declarations winning.
fn style(css: &str) -> Style {
    let mut style = Style::default();
    for declaration in parse_declarations(css) {
        style.merge(&declaration);
    }
    style
}

fn rgb(r: u8, g: u8, b: u8) -> Rgba {
    Rgba { r, g, b, a: 255 }
}

#[test]
fn box_shadow_lists() {
    let shadows = style("box-shadow: 1px 2px red, inset 3px 4px 5px 6px, blue -1px 0px 2px")
        .box_shadow
        .unwrap();
    assert_eq!(shadows.len(), 3);
    assert_eq!(
        (
            &shadows[0].offset_x,
            &shadows[0].offset_y,
            &shadows[0].blur,
            shadows[0].color,
            shadows[0].inset
        ),
        (
            &Length::Px(1.0),
            &Length::Px(2.0),
            &Length::Px(0.0),
            Some(rgb(255, 0, 0)),
            false
        )
    );
    assert!(shadows[1].inset);
    assert_eq!(
        (&shadows[1].spread, shadows[1].color),
        (&Length::Px(6.0), None)
    );
    assert_eq!(
        (&shadows[2].offset_x, shadows[2].color),
        (&Length::Px(-1.0), Some(rgb(0, 0, 255)))
    );

    assert_eq!(
        style("box-shadow: none")
            .box_shadow
            .map(|shadows| shadows.len()),
        Some(0)
    );
}

#[test]
fn invalid_box_shadows_are_dropped() {
    for css in [
        "box-shadow: 1px",
        "box-shadow: 1px 2px -3px",
        "box-shadow: 1px 2px 3px 4px 5px",
        "box-shadow: inset inset 1px 2px",
        "box-shadow: red blue 1px 2px",
    ] {
        assert!(style(css).box_shadow.is_none(), "{css}");
    }
}

#[test]
fn transform_functions() {
    let functions = style("transform: translate(10px) rotate(0.25turn) scale(2, 3) skewX(10grad) matrix(1, 2, 3, 4, 5, 6)")
        .transform
        .unwrap();
    assert_eq!(
        functions.as_slice(),
        &[
            TransformFunction::Translate(Length::Px(10.0), Length::Px(0.0)),
            TransformFunction::Rotate(90.0),
            TransformFunction::Scale(2.0, 3.0),
            TransformFunction::Skew(9.0, 0.0),
            TransformFunction::Matrix([1.0, 2.0, 3.0, 4.0, 5.0, 6.0]),
        ]
    );
    // An unknown function invalidates the whole declaration.
    assert!(style("transform: rotate(10deg) perspective(10px)")
        .transform
        .is_none());
}

#[test]
fn transform_origin_keywords() {
    let origin = |css: &str| style(css).transform_origin.unwrap();
    assert_eq!(
        origin("transform-origin: top left"),
        (Length::Percent(0.0), Length::Percent(0.0))
    );
    assert_eq!(
        origin("transform-origin: top"),
        (Length::Percent(50.0), Length::Percent(0.0))
    );
    assert_eq!(
        origin("transform-origin: right 10px"),
        (Length::Percent(100.0), Length::Px(10.0))
    );
    assert_eq!(
        origin("transform-origin: 5px"),
        (Length::Px(5.0), Length::Percent(50.0))
    );
}

#[test]
fn opacity_numbers_and_percentages() {
    assert_eq!(style("opacity: 0.5").opacity, Some(0.5));
    assert_eq!(style("opacity: 25%").opacity, Some(0.25));
    assert_eq!(style("opacity: 3").opacity, Some(1.0));
}

#[test]
fn border_sides_take_one_to_four_values() {
    let widths = style("border-width: 1px 2px 3px").border_width;
    assert_eq!(
        [widths.top, widths.right, widths.bottom, widths.left],
        [1.0, 2.0, 3.0, 2.0].map(|px| Some(Length::Px(px)))
    );
    let styles = style("border-style: solid dashed").border_style;
    assert_eq!(
        [styles.top, styles.right, styles.bottom, styles.left],
        [
            BorderStyle::Solid,
            BorderStyle::Dashed,
            BorderStyle::Solid,
            BorderStyle::Dashed
        ]
        .map(Some)
    );
    let colors = style("border-color: red green blue black").border_color;
    assert_eq!(
        [colors.top, colors.right, colors.bottom, colors.left],
        [rgb(255, 0, 0), rgb(0, 128, 0), rgb(0, 0, 255), rgb(0, 0, 0)].map(Some)
    );
    assert!(style("border-width: 1px 2px 3px 4px 5px")
        .border_width
        .top
        .is_none());
}

#[test]
fn used_border_widths_follow_the_style() {
    // Without a style there is no border; without a width it is medium.
    assert_eq!(
        style("border-width: 4px").used_border_width().top,
        Length::Px(0.0)
    );
    assert_eq!(
        style("border-style: solid").used_border_width().top,
        Length::Px(3.0)
    );
    assert_eq!(
        style("border: 2px dotted").used_border_width().left,
        Length::Px(2.0)
    );
    assert_eq!(
        style("border: 2px solid; border-right-style: hidden")
            .used_border_width()
            .right,
        Length::Px(0.0)
    );
}

#[test]
fn background_shorthand() {
    assert_eq!(
        style("background: red").background_color,
        Some(rgb(255, 0, 0))
    );
    assert_eq!(
        style("background: none red").background_color,
        Some(rgb(255, 0, 0))
    );
    assert_eq!(
        style("background: red; background: none").background_color,
        Some(Rgba::default())
    );
    assert!(style("background: red blue").background_color.is_none());
}
