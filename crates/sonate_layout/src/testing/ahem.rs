//! Text measurement with the metrics of the Ahem test font, which the fixtures use in Chrome.
//!
//! Every glyph (including the space) is a 1em square, the line height is 1em (`line-height: 1`)
//! and the baseline is 0.8em below the top of the line. Lines break at spaces only.

use crate::{AvailableSpace, MeasureInput, MeasureOutput, Size};

/// Measures `text` (whitespace already collapsed) at `font_size` px.
///
/// Mirrors how Chrome sizes the anonymous block around the text: an unknown width is the
/// fit-content width, and the text never wraps narrower than its longest word.
pub fn measure_text(text: &str, font_size: f64, input: MeasureInput) -> MeasureOutput {
    let em = font_size;
    let words: Vec<f64> = text
        .split_whitespace()
        .map(|word| word.chars().count() as f64 * em)
        .collect();
    if words.is_empty() {
        return MeasureOutput {
            size: input.known.map(|size| size.unwrap_or(0.0)),
            first_baseline: None,
        };
    }

    let min_content = words.iter().copied().fold(0.0, f64::max);
    let max_content = words.iter().sum::<f64>() + em * (words.len() - 1) as f64;

    let width = input.known.width.unwrap_or(match input.available.width {
        AvailableSpace::Definite(space) => space.min(max_content).max(min_content),
        AvailableSpace::MinContent => min_content,
        AvailableSpace::MaxContent => max_content,
    });
    let line_count = count_lines(&words, width.clamp(min_content, max_content), em);
    let height = input.known.height.unwrap_or(line_count as f64 * em);

    MeasureOutput {
        size: Size::new(width, height),
        first_baseline: Some(0.8 * em),
    }
}

/// Greedy line breaking; a trailing space hangs and does not count towards the line width.
fn count_lines(words: &[f64], width: f64, space: f64) -> usize {
    let mut lines = 1;
    let mut line_width = words[0];
    for &word in &words[1..] {
        if line_width + space + word <= width + 1e-6 {
            line_width += space + word;
        } else {
            lines += 1;
            line_width = word;
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measure(text: &str, known_width: Option<f64>, available: AvailableSpace) -> Size<f64> {
        measure_text(
            text,
            10.0,
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
