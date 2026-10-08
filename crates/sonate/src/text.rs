//! Fonts and text measurement. Line breaking itself is shared with the layout engine
//! (`sonate_layout::text`), so measuring, painting and the tests agree.

use crate::{
    layout::Size,
    style::{FontFace, FontSource, FontStyle, FontWeight, Length, LineHeight, Style},
};
use parking_lot::RwLock;
use skia_safe::{font_style, Font, FontMgr, Typeface};
use sonate_layout::text::{self as text_layout, FontMetrics, TextStyle};
use std::{cell::RefCell, collections::HashMap, sync::Arc};

/// The font family list used when no `font-family` is set.
const DEFAULT_FAMILY: &str = "Arial, sans-serif";

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FontSpec {
    /// A family list as written in CSS, e.g. `"Fira Sans", sans-serif`.
    pub family: String,
    /// The size in 1/64 px.
    size_64: u32,
    pub weight: u16,
    pub italic: bool,
}

impl FontSpec {
    pub fn new(family: &str, size_px: f64, weight: u16, italic: bool) -> Self {
        Self {
            family: family.to_owned(),
            size_64: (size_px.clamp(0.0, 4096.0) * 64.0).round() as u32,
            weight,
            italic,
        }
    }

    pub fn from_style(style: &Style) -> Self {
        Self::new(
            style.font_family.as_deref().unwrap_or(DEFAULT_FAMILY),
            font_size_px(style),
            match style.font_weight {
                Some(FontWeight::Number(weight)) => weight,
                _ => 400,
            },
            matches!(style.font_style, Some(FontStyle::Italic | FontStyle::Oblique)),
        )
    }

    pub fn size_px(&self) -> f64 {
        f64::from(self.size_64) / 64.0
    }

    /// The family names in order, without quotes.
    pub fn families(&self) -> Vec<String> {
        let mut families = Vec::new();
        let mut current = String::new();
        let mut quote = None;
        for c in self.family.chars() {
            match (c, quote) {
                ('"' | '\'', None) => quote = Some(c),
                (c, Some(q)) if c == q => quote = None,
                (',', None) => families.push(std::mem::take(&mut current)),
                (c, _) => current.push(c),
            }
        }
        families.push(current);
        families
            .into_iter()
            .map(|family| family.trim().to_owned())
            .filter(|family| !family.is_empty())
            .collect()
    }
}

/// The computed font size in px.
pub fn font_size_px(style: &Style) -> f64 {
    match style.font_size {
        Some(Length::Px(px)) if px > 0.0 => px,
        _ => crate::cascade::DEFAULT_FONT_SIZE_PX,
    }
}

/// The text properties of a computed style, for line breaking and alignment.
pub fn text_style(style: &Style) -> TextStyle {
    let font_size = font_size_px(style);
    let spacing = |length: &Option<Length>| length.as_ref().map_or(0.0, Length::to_px);
    TextStyle {
        white_space: style.white_space.unwrap_or_default(),
        text_transform: style.text_transform.unwrap_or_default(),
        overflow_wrap: style.overflow_wrap.unwrap_or_default(),
        word_break: style.word_break.unwrap_or_default(),
        text_align: style.text_align.unwrap_or_default(),
        letter_spacing: spacing(&style.letter_spacing),
        word_spacing: spacing(&style.word_spacing),
        line_height: match &style.line_height {
            None | Some(LineHeight::Normal) => None,
            Some(LineHeight::Number(factor)) => Some(factor * font_size),
            Some(LineHeight::Length(length)) => Some(length.to_px()),
        },
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

    /// The advance width of a run of text, without letter or word spacing.
    fn advance(&self, text: &str, font: &FontSpec) -> f64;

    /// The font's vertical metrics.
    fn metrics(&self, font: &FontSpec) -> FontMetrics;

    /// Size of `text` on one line with its whitespace kept (used by the legacy engine).
    fn measure_unwrapped(&self, text: &str, font: &FontSpec) -> Size {
        let metrics = self.metrics(font);
        Size {
            width: self.advance(text, font),
            height: metrics.line_height(&TextStyle::default()),
        }
    }

    /// Size of `text` wrapped at `max_width_px` (used by the legacy engine).
    fn measure_wrapped(&self, text: &str, font: &FontSpec, max_width_px: f64) -> Size {
        let style = TextStyle::default();
        let lines =
            text_layout::break_lines(text, &style, max_width_px, &mut |run| self.advance(run, font));
        Size {
            width: lines.iter().map(|line| line.width).fold(0.0, f64::max),
            height: lines.len() as f64 * self.metrics(font).line_height(&style),
        }
    }
}

/// A font registered by `@font-face`.
struct RegisteredFace {
    /// Lowercase family name.
    family: String,
    weight: u16,
    italic: bool,
    source: LoadedSource,
}

enum LoadedSource {
    Data(Arc<Vec<u8>>),
    /// An installed font under another name.
    Local(String),
}

static FONT_FACES: std::sync::RwLock<Vec<RegisteredFace>> = std::sync::RwLock::new(Vec::new());

/// Loads the fonts of `@font-face` rules. Each face uses its first source that loads; file
/// paths resolve against the working directory.
pub fn register_font_faces(faces: &[FontFace]) {
    for face in faces {
        let source = face.sources.iter().find_map(|source| match source {
            FontSource::Url(url) => {
                let path = url.strip_prefix("file://").unwrap_or(url);
                match std::fs::read(path) {
                    Ok(data) => Some(LoadedSource::Data(Arc::new(data))),
                    Err(err) => {
                        eprintln!("@font-face {}: can't read {path}: {err}", face.family);
                        None
                    }
                }
            }
            FontSource::Local(name) => {
                let style = font_style::FontStyle::normal();
                FontMgr::default()
                    .match_family_style(name, style)
                    .map(|_| LoadedSource::Local(name.clone()))
            }
        });
        if let Some(source) = source {
            FONT_FACES.write().unwrap().push(RegisteredFace {
                family: face.family.to_lowercase(),
                weight: face.weight,
                italic: face.italic,
                source,
            });
        }
    }
}

thread_local! {
    /// Typefaces made from registered font data, per thread (keyed by the data's address).
    static TYPEFACES: RefCell<HashMap<usize, Typeface>> = RefCell::new(HashMap::new());
}

/// Installed fonts to try for a generic family where the platform doesn't map it.
fn generic_candidates(family: &str) -> &'static [&'static str] {
    match family.to_ascii_lowercase().as_str() {
        "sans-serif" | "system-ui" | "ui-sans-serif" | "ui-rounded" => &[
            "Arial",
            "Helvetica",
            "Segoe UI",
            "DejaVu Sans",
            "Liberation Sans",
        ],
        "serif" | "ui-serif" => &["Times New Roman", "Times", "DejaVu Serif", "Liberation Serif"],
        "monospace" | "ui-monospace" => &["Menlo", "Consolas", "Courier New", "DejaVu Sans Mono"],
        "cursive" => &["Comic Sans MS", "Apple Chancery"],
        "fantasy" => &["Impact", "Papyrus"],
        _ => &[],
    }
}

/// A registered or installed typeface for one family name.
fn find_typeface(
    family: &str,
    style: font_style::FontStyle,
    weight: u16,
    italic: bool,
) -> Option<Typeface> {
    let lower = family.to_lowercase();
    let registered = {
        let faces = FONT_FACES.read().unwrap();
        // The closest weight among faces of the family, preferring the right style.
        faces
            .iter()
            .filter(|face| face.family == lower)
            .min_by_key(|face| (face.italic != italic, face.weight.abs_diff(weight)))
            .map(|face| match &face.source {
                LoadedSource::Data(data) => Ok(data.clone()),
                LoadedSource::Local(name) => Err(name.clone()),
            })
    };
    match registered {
        Some(Ok(data)) => {
            let key = Arc::as_ptr(&data) as usize;
            return TYPEFACES.with(|typefaces| {
                let mut typefaces = typefaces.borrow_mut();
                if let Some(typeface) = typefaces.get(&key) {
                    return Some(typeface.clone());
                }
                let typeface = FontMgr::default().new_from_data(&data, None)?;
                typefaces.insert(key, typeface.clone());
                Some(typeface)
            });
        }
        Some(Err(local)) => return FontMgr::default().match_family_style(&local, style),
        None => {}
    }

    FontMgr::default()
        .match_family_style(family, style)
        .or_else(|| {
            generic_candidates(family)
                .iter()
                .find_map(|candidate| FontMgr::default().match_family_style(candidate, style))
        })
}

/// Measures with Skia and the system's fonts.
#[derive(Clone, Default)]
pub struct SkiaTextMeasurer {
    cache: Arc<RwLock<CacheState>>,
}

#[derive(Default)]
struct CacheState {
    epoch: u64,
    advances: HashMap<(String, FontSpec), (f64, u64)>,
}

impl SkiaTextMeasurer {
    #[allow(unused)]
    pub fn new() -> Self {
        Self::default()
    }

    /// The font for a spec: the first family in the list that is registered or installed, with
    /// synthetic bold and italic when the typeface lacks them.
    pub(crate) fn make_font(spec: &FontSpec) -> Font {
        let slant = if spec.italic {
            font_style::Slant::Italic
        } else {
            font_style::Slant::Upright
        };
        let style = font_style::FontStyle::new(
            font_style::Weight::from(i32::from(spec.weight)),
            font_style::Width::NORMAL,
            slant,
        );
        let typeface = spec
            .families()
            .iter()
            .find_map(|family| find_typeface(family, style, spec.weight, spec.italic))
            .or_else(|| FontMgr::default().legacy_make_typeface(None, style))
            .expect("Failed to load any typeface");

        let mut font = Font::new(typeface.clone(), spec.size_px() as f32);
        let actual = typeface.font_style();
        if spec.weight >= 600 && *actual.weight() < 600 {
            font.set_embolden(true);
        }
        if spec.italic && actual.slant() == font_style::Slant::Upright {
            font.set_skew_x(-0.25);
        }
        font
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
        state.advances.retain(|_, (_, used)| *used == epoch);
    }

    fn advance(&self, text: &str, font: &FontSpec) -> f64 {
        let key = (text.to_owned(), font.clone());
        let mut state = self.cache.write();
        let epoch = state.epoch;
        if let Some((width, used)) = state.advances.get_mut(&key) {
            *used = epoch;
            return *width;
        }
        let width = Self::make_font(font).measure_str(text, None).0 as f64;
        state.advances.insert(key, (width, epoch));
        width
    }

    fn metrics(&self, font: &FontSpec) -> FontMetrics {
        let (_scale, metrics) = Self::make_font(font).metrics();
        FontMetrics {
            ascent: -metrics.ascent as f64,
            descent: metrics.descent as f64,
            line_gap: metrics.leading as f64,
        }
        .rounded()
    }
}

/// Deterministic metrics for unit tests, matching the Ahem font that the layout fixtures use in
/// Chrome: every glyph (including the space) is a 1em square, the ascent is 0.8em and the
/// descent 0.2em.
#[cfg(test)]
#[derive(Clone, Default)]
#[allow(unused)]
pub struct TestTextMeasurer;

#[cfg(test)]
impl TextMeasurer for TestTextMeasurer {
    fn advance(&self, text: &str, font: &FontSpec) -> f64 {
        text.chars().count() as f64 * font.size_px()
    }

    fn metrics(&self, font: &FontSpec) -> FontMetrics {
        FontMetrics {
            ascent: 0.8 * font.size_px(),
            descent: 0.2 * font.size_px(),
            line_gap: 0.0,
        }
        .rounded()
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

/// The line boxes of a text node, relative to its text box (see `sonate_layout::TextBox`):
/// where each line's glyphs go and how high the font is.
#[allow(unused)]
pub fn line_rects(
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &Style,
    box_width: f64,
) -> Vec<[f64; 4]> {
    let font = FontSpec::from_style(style);
    text_layout::line_rects(
        text,
        &text_style(style),
        measurer.metrics(&font),
        box_width,
        &mut |run| measurer.advance(run, &font),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_lists() {
        let font = FontSpec::new("\"Fira Sans\", Arial , 'a, b', sans-serif", 12.0, 400, false);
        assert_eq!(font.families(), ["Fira Sans", "Arial", "a, b", "sans-serif"]);
        assert_eq!(FontSpec::new("x", 10.5, 400, false).size_px(), 10.5);
    }

    #[test]
    fn font_face_loads_files() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../sonate_layout/tools/bless/Ahem.ttf");
        let sheet = crate::css_parser::parse_css(&format!(
            "@font-face {{ font-family: \"Test Ahem\"; src: url(\"/missing.ttf\"), url(\"{path}\") format(\"truetype\"); }}"
        ))
        .unwrap();
        assert_eq!(sheet.font_faces[0].family, "Test Ahem");
        register_font_faces(&sheet.font_faces);

        // Ahem glyphs are 1em squares, so a real Skia measurement is exact.
        let measurer = SkiaTextMeasurer::new();
        let font = FontSpec::new("\"Test Ahem\", serif", 10.0, 400, false);
        assert_eq!(measurer.advance("Hello", &font), 50.0);
        let metrics = measurer.metrics(&font);
        assert_eq!((metrics.ascent, metrics.descent), (8.0, 2.0));
    }
}
