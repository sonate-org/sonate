//! 2D transforms (CSS Transforms Level 1) as affine matrices.

use crate::layout::Rect;
use crate::style::{Length, Style, TransformFunction};

/// `[a, b, c, d, e, f]`: x' = a·x + c·y + e, y' = b·x + d·y + f.
pub type Affine = [f64; 6];

/// The transform that applies `inner` first, then `outer`.
pub fn compose(outer: Affine, inner: Affine) -> Affine {
    let [a1, b1, c1, d1, e1, f1] = outer;
    let [a2, b2, c2, d2, e2, f2] = inner;
    [
        a1 * a2 + c1 * b2,
        b1 * a2 + d1 * b2,
        a1 * c2 + c1 * d2,
        b1 * c2 + d1 * d2,
        a1 * e2 + c1 * f2 + e1,
        b1 * e2 + d1 * f2 + f1,
    ]
}

pub fn apply(m: Affine, x: f64, y: f64) -> (f64, f64) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

/// `None` for transforms that collapse to a line or point.
pub fn invert(m: Affine) -> Option<Affine> {
    let [a, b, c, d, e, f] = m;
    let det = a * d - b * c;
    if det.abs() < 1e-12 {
        return None;
    }
    Some([
        d / det,
        -b / det,
        -c / det,
        a / det,
        (c * f - d * e) / det,
        (b * e - a * f) / det,
    ])
}

/// Resolves a length against `basis`, the size percentages refer to.
fn resolve(length: &Length, basis: f64) -> f64 {
    match length {
        Length::Percent(percent) => percent / 100.0 * basis,
        Length::Calc(node) => node
            .evaluate(&|leaf| match leaf {
                Length::Percent(percent) => Some(percent / 100.0 * basis),
                other => Some(other.to_px()),
            })
            .unwrap_or(0.0),
        other => other.to_px(),
    }
}

/// `transform-origin` in px from the border box's top left corner.
pub fn transform_origin(style: &Style, width: f64, height: f64) -> (f64, f64) {
    match &style.transform_origin {
        Some((x, y)) => (resolve(x, width), resolve(y, height)),
        None => (width / 2.0, height / 2.0),
    }
}

/// The `transform` functions as one matrix for a border box of `width` × `height`, without the
/// origin: what browsers report as the computed `matrix(…)`. `None` for `transform: none`.
pub fn transform_matrix(style: &Style, width: f64, height: f64) -> Option<Affine> {
    let functions = style
        .transform
        .as_ref()
        .filter(|functions| !functions.is_empty())?;
    let mut matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    for function in functions.iter() {
        let next = match function {
            TransformFunction::Matrix(values) => *values,
            TransformFunction::Translate(x, y) => {
                [1.0, 0.0, 0.0, 1.0, resolve(x, width), resolve(y, height)]
            }
            TransformFunction::Scale(x, y) => [*x, 0.0, 0.0, *y, 0.0, 0.0],
            TransformFunction::Rotate(degrees) => {
                let (sin, cos) = sin_cos_degrees(*degrees);
                [cos, sin, -sin, cos, 0.0, 0.0]
            }
            TransformFunction::Skew(x, y) => [
                1.0,
                y.to_radians().tan(),
                x.to_radians().tan(),
                1.0,
                0.0,
                0.0,
            ],
        };
        matrix = compose(matrix, next);
    }
    Some(matrix)
}

/// Sine and cosine, exact for multiples of 90 degrees like in browsers.
fn sin_cos_degrees(degrees: f64) -> (f64, f64) {
    let quarter_turns = degrees / 90.0;
    if quarter_turns.fract() == 0.0 {
        return match quarter_turns.rem_euclid(4.0) as u8 {
            0 => (0.0, 1.0),
            1 => (1.0, 0.0),
            2 => (0.0, -1.0),
            _ => (-1.0, 0.0),
        };
    }
    degrees.to_radians().sin_cos()
}

/// The element's transform in its parent's coordinates: the `transform` functions applied
/// around `transform-origin` (50% 50% by default) of its border box.
pub fn element_transform(style: &Style, bounds: Rect) -> Option<Affine> {
    let matrix = transform_matrix(style, bounds.width, bounds.height)?;
    let (origin_x, origin_y) = transform_origin(style, bounds.width, bounds.height);
    let (origin_x, origin_y) = (bounds.x + origin_x, bounds.y + origin_y);
    let around_origin = compose([1.0, 0.0, 0.0, 1.0, origin_x, origin_y], matrix);
    Some(compose(
        around_origin,
        [1.0, 0.0, 0.0, 1.0, -origin_x, -origin_y],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
    }

    #[test]
    fn rotates_around_the_center() {
        let style = crate::css_parser::parse_declarations("transform: rotate(90deg)")
            .remove(0)
            .style;
        let m = element_transform(&style, Rect::new(0.0, 0.0, 100.0, 50.0)).unwrap();
        // The top-left corner turns around the center (50, 25).
        assert!(close(apply(m, 0.0, 0.0), (75.0, -25.0)));
        assert!(close(apply(invert(m).unwrap(), 75.0, -25.0), (0.0, 0.0)));
    }

    #[test]
    fn functions_apply_right_to_left() {
        let style = crate::css_parser::parse_declarations(
            "transform: translate(10px, 50%) scale(2); transform-origin: 0 0",
        );
        let mut combined = Style::default();
        for declaration in style {
            combined.merge(&declaration);
        }
        let m = element_transform(&combined, Rect::new(0.0, 0.0, 100.0, 40.0)).unwrap();
        assert!(close(apply(m, 5.0, 5.0), (20.0, 30.0)));
    }
}
