use crate::{
    layout::Size,
    style::{Length, Style},
};
use parking_lot::RwLock;
use skia_safe::{Font, FontMgr, FontStyle};
use std::{collections::HashMap, sync::Arc};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FontSpec {
    pub family: String,
    pub size_px: u32,
}

impl FontSpec {
    pub fn from_style(style: &Style) -> Self {
        let family = style
            .font_family
            .clone()
            .unwrap_or_else(|| "Arial".to_string());

        let size_px = match style.font_size {
            Some(Length::Px(px)) if px > 0.0 => px.round().clamp(1.0, 512.0) as u32,
            _ => 12,
        };

        Self { family, size_px }
    }
}

pub trait TextMeasurer: Send + Sync {
    /// Called at the start of a layout pass.
    ///
    /// Implementations with caches can use this to begin "mark" tracking.
    fn begin_layout_pass(&self) {}

    /// Called at the end of a layout pass.
    ///
    /// Implementations with caches can use this to "sweep" anything not used
    /// during the pass.
    fn end_layout_pass_and_sweep(&self) {}

    /// Size of `text` on one line with its whitespace kept as is (text inputs).
    fn measure_unwrapped(&self, text: &str, font: &FontSpec) -> Size;

    /// Size of `text` with collapsed whitespace, broken into lines no wider than
    /// `max_width_px` (see [`break_lines`]). `f64::INFINITY` gives the max-content size.
    fn measure_wrapped(&self, text: &str, font: &FontSpec, max_width_px: f64) -> Size;

    /// Width of the widest word: the narrowest the text can wrap to (its min-content width).
    fn min_content_width(&self, text: &str, font: &FontSpec) -> f64;

    /// Distance from the top of a line to its baseline.
    fn ascent(&self, font: &FontSpec) -> f64;
}

/// Breaks `text` into lines at whitespace, greedily, so that no line is wider than
/// `max_width` unless it holds a single word. Whitespace is collapsed like in HTML.
/// Returns each line with its width.
pub fn break_lines(text: &str, max_width: f64, measure: &mut dyn FnMut(&str) -> f64) -> Vec<(String, f64)> {
    let space = measure(" ");
    let mut lines: Vec<(String, f64)> = Vec::new();
    for word in text.split_whitespace() {
        let word_width = measure(word);
        match lines.last_mut() {
            Some((line, width)) if *width + space + word_width <= max_width + 1e-6 => {
                line.push(' ');
                line.push_str(word);
                *width += space + word_width;
            }
            _ => lines.push((word.to_owned(), word_width)),
        }
    }
    lines
}

/// Size of the wrapped text for a measure function and line height.
fn wrapped_size(text: &str, max_width: f64, line_height: f64, measure: &mut dyn FnMut(&str) -> f64) -> Size {
    let lines = break_lines(text, max_width, measure);
    Size {
        width: lines.iter().map(|(_, width)| *width).fold(0.0, f64::max),
        height: lines.len() as f64 * line_height,
    }
}

fn widest_word(text: &str, measure: &mut dyn FnMut(&str) -> f64) -> f64 {
    text.split_whitespace().map(measure).fold(0.0, f64::max)
}

#[derive(Clone, Default)]
pub struct SkiaTextMeasurer {
    cache: Arc<RwLock<CacheState>>,
}

#[derive(Default)]
struct CacheState {
    epoch: u64,
    map: HashMap<CacheKey, CacheEntry>,
}

#[derive(Clone, Copy)]
struct CacheEntry {
    size: Size,
    last_used_epoch: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Measurement {
    Unwrapped,
    /// Wrapped at a width in 1/64 px.
    Wrapped(u64),
    MinContent,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct CacheKey {
    text: String,
    family: String,
    size_px: u32,
    measurement: Measurement,
}

impl SkiaTextMeasurer {
    #[allow(unused)]
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(CacheState::default())),
        }
    }

    pub(crate) fn make_font(font: &FontSpec) -> Font {
        let typeface = FontMgr::default()
            .match_family(&font.family)
            .match_style(FontStyle::normal())
            .unwrap_or_else(|| {
                // Fallback typeface.
                FontMgr::default()
                    .legacy_make_typeface(None, FontStyle::normal())
                    .expect("Failed to load any typeface")
            });

        Font::new(typeface, font.size_px as f32)
    }

    /// The height of one line of text.
    pub(crate) fn line_height(font: &Font) -> f64 {
        let (_scale, metrics) = font.metrics();
        ((metrics.descent - metrics.ascent + metrics.leading) as f64).max(0.0)
    }

    fn measure_uncached(text: &str, spec: &FontSpec, measurement: Measurement) -> Size {
        let font = Self::make_font(spec);
        let mut measure = |text: &str| font.measure_str(text, None).0 as f64;
        match measurement {
            Measurement::Unwrapped => Size {
                width: measure(text),
                height: Self::line_height(&font),
            },
            Measurement::Wrapped(max_width) => {
                let max_width = if max_width == u64::MAX {
                    f64::INFINITY
                } else {
                    max_width as f64 / 64.0
                };
                wrapped_size(text, max_width, Self::line_height(&font), &mut measure)
            }
            Measurement::MinContent => Size {
                width: widest_word(text, &mut measure),
                height: 0.0,
            },
        }
    }

    fn measure_cached(&self, text: &str, font: &FontSpec, measurement: Measurement) -> Size {
        let key = CacheKey {
            text: text.to_string(),
            family: font.family.clone(),
            size_px: font.size_px,
            measurement,
        };

        let mut state = self.cache.write();
        let epoch = state.epoch;

        if let Some(entry) = state.map.get_mut(&key) {
            entry.last_used_epoch = epoch;
            return entry.size;
        }

        // Cache miss.
        let size = Self::measure_uncached(text, font, measurement);
        state.map.insert(
            key,
            CacheEntry {
                size,
                last_used_epoch: epoch,
            },
        );
        size
    }
}

impl TextMeasurer for SkiaTextMeasurer {
    fn begin_layout_pass(&self) {
        let mut state = self.cache.write();
        state.epoch = state.epoch.wrapping_add(1);
    }

    fn end_layout_pass_and_sweep(&self) {
        let mut state = self.cache.write();
        let epoch = state.epoch;
        state.map.retain(|_, entry| entry.last_used_epoch == epoch);
    }

    fn measure_unwrapped(&self, text: &str, font: &FontSpec) -> Size {
        self.measure_cached(text, font, Measurement::Unwrapped)
    }

    fn measure_wrapped(&self, text: &str, font: &FontSpec, max_width_px: f64) -> Size {
        let max_width = if max_width_px.is_finite() {
            (max_width_px.max(0.0) * 64.0).round() as u64
        } else {
            u64::MAX
        };
        self.measure_cached(text, font, Measurement::Wrapped(max_width))
    }

    fn min_content_width(&self, text: &str, font: &FontSpec) -> f64 {
        self.measure_cached(text, font, Measurement::MinContent).width
    }

    fn ascent(&self, font: &FontSpec) -> f64 {
        let (_scale, metrics) = Self::make_font(font).metrics();
        -metrics.ascent as f64
    }
}

/// Deterministic metrics for unit tests, matching the Ahem font that the layout fixtures use in
/// Chrome: every glyph (including the space) is a 1em square, lines are 1em tall and the
/// baseline is 0.8em below the top of the line.
#[cfg(test)]
#[derive(Clone, Default)]
#[allow(unused)]
pub struct TestTextMeasurer;

#[cfg(test)]
impl TestTextMeasurer {
    fn em(font: &FontSpec) -> f64 {
        font.size_px as f64
    }
}

#[cfg(test)]
impl TextMeasurer for TestTextMeasurer {
    fn measure_unwrapped(&self, text: &str, font: &FontSpec) -> Size {
        Size {
            width: text.chars().count() as f64 * Self::em(font),
            height: Self::em(font),
        }
    }

    fn measure_wrapped(&self, text: &str, font: &FontSpec, max_width_px: f64) -> Size {
        let em = Self::em(font);
        wrapped_size(text, max_width_px, em, &mut |text| text.chars().count() as f64 * em)
    }

    fn min_content_width(&self, text: &str, font: &FontSpec) -> f64 {
        let em = Self::em(font);
        widest_word(text, &mut |text| text.chars().count() as f64 * em)
    }

    fn ascent(&self, font: &FontSpec) -> f64 {
        0.8 * Self::em(font)
    }
}

pub fn default_text_measurer() -> Arc<dyn TextMeasurer> {
    #[cfg(test)]
    {
        Arc::new(TestTextMeasurer::default())
    }

    #[cfg(not(test))]
    {
        Arc::new(SkiaTextMeasurer::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn font() -> FontSpec {
        FontSpec {
            family: "Ahem".to_owned(),
            size_px: 10,
        }
    }

    #[test]
    fn wraps_greedily_at_spaces() {
        let measurer = TestTextMeasurer;
        let size = measurer.measure_wrapped("aa bbbb  c", &font(), 70.0);
        assert_eq!((size.width, size.height), (70.0, 20.0));
        assert_eq!(measurer.measure_wrapped(" aa bbbb ", &font(), f64::INFINITY).width, 70.0);
        assert_eq!(measurer.min_content_width("aa bbbb", &font()), 40.0);
    }

    #[test]
    fn long_words_get_their_own_line() {
        let mut measure = |text: &str| text.len() as f64 * 10.0;
        let lines = break_lines("a bbbbbb c", 30.0, &mut measure);
        let lines: Vec<&str> = lines.iter().map(|(line, _)| line.as_str()).collect();
        assert_eq!(lines, ["a", "bbbbbb", "c"]);
    }
}
