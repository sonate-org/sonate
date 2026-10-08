//! Text measurement with the metrics of the Ahem test font, which the fixtures use in Chrome.
//!
//! Every glyph (including the space) is a 1em square, the ascent is 0.8em and the descent 0.2em,
//! so the normal line height is 1em. Line breaking follows `crate::text`.

use crate::text::{self, FontMetrics, TextStyle};
use crate::{MeasureInput, MeasureOutput};

/// The Ahem font's metrics at `font_size`.
pub fn ahem_metrics(font_size: f64) -> FontMetrics {
    FontMetrics {
        ascent: 0.8 * font_size,
        descent: 0.2 * font_size,
        line_gap: 0.0,
    }
    .rounded()
}

/// Measures `text` at `font_size` px with the Ahem font.
pub fn measure_text(text: &str, font_size: f64, style: &TextStyle, input: MeasureInput) -> MeasureOutput {
    text::measure_text(text, style, ahem_metrics(font_size), input, &mut |run| {
        run.chars().count() as f64 * font_size
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{AvailableSpace, Size};

    fn measure(text: &str, known_width: Option<f64>, available: AvailableSpace) -> Size<f64> {
        measure_text(
            text,
            10.0,
            &TextStyle::default(),
            MeasureInput {
                known: Size::new(known_width, None),
                available: Size::new(available, AvailableSpace::MaxContent),
            },
        )
        .size
    }

    #[test]
    fn intrinsic_sizes() {
        assert_eq!(measure("aa bbbb", None, AvailableSpace::MaxContent), Size::new(70.0, 10.0));
        assert_eq!(measure("aa bbbb", None, AvailableSpace::MinContent), Size::new(40.0, 20.0));
    }

    #[test]
    fn fit_content_in_definite_space() {
        assert_eq!(measure("aa bbbb", None, AvailableSpace::Definite(55.0)), Size::new(55.0, 20.0));
        assert_eq!(measure("aa bbbb", None, AvailableSpace::Definite(10.0)), Size::new(40.0, 20.0));
    }

    #[test]
    fn never_wraps_narrower_than_the_longest_word() {
        // At 40px "a b" fits on one line, so Chrome shows two lines even when the box is 20px.
        assert_eq!(measure("a b cccc", Some(20.0), AvailableSpace::MaxContent), Size::new(20.0, 20.0));
    }
}
