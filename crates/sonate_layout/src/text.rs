//! Breaking text into lines (CSS Text Module Level 3) for measured leaves.
//! <https://www.w3.org/TR/css-text-3/>
//!
//! This module only needs the advance width of text runs, which the caller provides, so the
//! same rules serve Sonate's real fonts and the Ahem font of the test fixtures.

/// `white-space`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WhiteSpace {
    #[default]
    Normal,
    NoWrap,
    Pre,
    PreWrap,
    PreLine,
    BreakSpaces,
}

impl WhiteSpace {
    /// Whether runs of spaces and tabs collapse into one space.
    fn collapses_spaces(self) -> bool {
        matches!(self, WhiteSpace::Normal | WhiteSpace::NoWrap | WhiteSpace::PreLine)
    }

    /// Whether newlines force line breaks (otherwise they are spaces).
    fn preserves_newlines(self) -> bool {
        !matches!(self, WhiteSpace::Normal | WhiteSpace::NoWrap)
    }

    /// Whether lines may wrap.
    fn wraps(self) -> bool {
        !matches!(self, WhiteSpace::NoWrap | WhiteSpace::Pre)
    }

    /// Whether spaces at the end of a line hang (don't count towards its width). Collapsible
    /// spaces there are removed; preserved ones hang only with `pre-wrap`.
    fn hangs_trailing_spaces(self) -> bool {
        self.collapses_spaces() || self == WhiteSpace::PreWrap
    }
}

/// `text-transform`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextTransform {
    #[default]
    None,
    Uppercase,
    Lowercase,
    Capitalize,
}

/// `overflow-wrap` (also `word-wrap`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverflowWrap {
    #[default]
    Normal,
    /// Break inside a word that doesn't fit, without changing the min-content size.
    BreakWord,
    /// Break inside a word that doesn't fit; the min-content size counts these breaks.
    Anywhere,
}

/// `word-break`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WordBreak {
    #[default]
    Normal,
    /// Lines may break between any two letters.
    BreakAll,
    /// Like `normal` for the scripts Sonate handles.
    KeepAll,
}

/// `text-align`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAlign {
    #[default]
    Start,
    End,
    Left,
    Right,
    Center,
    Justify,
}

/// The inherited text properties that affect line breaking and placement.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextStyle {
    pub white_space: WhiteSpace,
    pub text_transform: TextTransform,
    pub overflow_wrap: OverflowWrap,
    pub word_break: WordBreak,
    pub text_align: TextAlign,
    /// Added after every character, in px.
    pub letter_spacing: f64,
    /// Added to every space, in px.
    pub word_spacing: f64,
    /// `line-height` in px; `None` is `normal`.
    pub line_height: Option<f64>,
}

/// Vertical font metrics in px (all positive).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FontMetrics {
    pub ascent: f64,
    pub descent: f64,
    pub line_gap: f64,
}

impl FontMetrics {
    /// The height of a line: `line-height`, or the font's own line spacing for `normal`.
    pub fn line_height(&self, style: &TextStyle) -> f64 {
        style
            .line_height
            .unwrap_or(self.ascent + self.descent + self.line_gap)
    }

    /// Distance from the top of a line to its baseline: the ascent plus half the leading,
    /// floored to whole pixels like Chrome does.
    pub fn baseline(&self, style: &TextStyle) -> f64 {
        self.half_leading(style) + self.ascent
    }

    /// The space above the glyphs in a line: half the leading, floored like in Chrome.
    pub fn half_leading(&self, style: &TextStyle) -> f64 {
        ((self.line_height(style) - self.ascent - self.descent) / 2.0 + 1e-6).floor()
    }

    /// Rounds the ascent and descent to whole pixels, as Chrome does for its line layout.
    pub fn rounded(self) -> Self {
        Self {
            ascent: self.ascent.round(),
            descent: self.descent.round(),
            line_gap: self.line_gap,
        }
    }
}

/// One line of laid-out text.
#[derive(Clone, Debug, PartialEq)]
pub struct TextLine {
    pub text: String,
    /// Advance width including spacing, without spaces that hang at the end.
    pub width: f64,
    /// Whether the line ends a paragraph (at a forced break or the end of the text), which
    /// matters for `text-align: justify`.
    pub ends_paragraph: bool,
    /// The width of preserved spaces that hang at the end (`pre-wrap`).
    pub hanging_width: f64,
}

/// Measures the advance width of a run of text.
pub type MeasureRun<'a> = dyn FnMut(&str) -> f64 + 'a;

/// A piece of a paragraph: a word (or a letter, with `word-break: break-all`) or spaces.
#[derive(Clone, Debug)]
struct Piece {
    text: String,
    width: f64,
    is_space: bool,
    /// Whether a line may break after this piece.
    break_after: bool,
}

/// Applies `text-transform` and the white-space processing of `white-space`, and splits the
/// text into paragraphs at forced breaks.
fn paragraphs(text: &str, style: &TextStyle) -> Vec<String> {
    let text = transform(text, style.text_transform);
    let mut result = Vec::new();
    let sources: Vec<&str> = if style.white_space.preserves_newlines() {
        text.split('\n').collect()
    } else {
        vec![&text[..]]
    };
    for source in sources {
        if style.white_space.collapses_spaces() {
            let collapsed: Vec<&str> = source.split_whitespace().collect();
            result.push(collapsed.join(" "));
        } else {
            result.push(source.replace('\t', " "));
        }
    }
    // Collapsible whitespace alone doesn't make a line.
    if style.white_space.collapses_spaces() && !style.white_space.preserves_newlines() {
        result.retain(|paragraph| !paragraph.is_empty());
    }
    result
}

fn transform(text: &str, transform: TextTransform) -> String {
    match transform {
        TextTransform::None => text.to_owned(),
        TextTransform::Uppercase => text.to_uppercase(),
        TextTransform::Lowercase => text.to_lowercase(),
        TextTransform::Capitalize => {
            let mut capitalize = true;
            text.chars()
                .flat_map(|c| {
                    let upper = capitalize && c.is_alphanumeric();
                    capitalize = c.is_whitespace();
                    if upper {
                        c.to_uppercase().collect::<Vec<_>>()
                    } else {
                        vec![c]
                    }
                })
                .collect()
        }
    }
}

/// The advance width of a run including `letter-spacing` and `word-spacing`.
fn run_width(text: &str, style: &TextStyle, measure: &mut MeasureRun) -> f64 {
    if text.is_empty() {
        return 0.0;
    }
    let characters = text.chars().count() as f64;
    let spaces = text.chars().filter(|&c| c == ' ').count() as f64;
    measure(text) + style.letter_spacing * characters + style.word_spacing * spaces
}

/// Splits a paragraph into pieces: words and space runs, with every space separate for
/// `break-spaces` and every letter separate for `word-break: break-all`. Lines may break after
/// spaces and, with `break-all`, between letters.
fn pieces(paragraph: &str, style: &TextStyle, measure: &mut MeasureRun) -> Vec<Piece> {
    let wraps = style.white_space.wraps();
    let split_spaces = style.white_space == WhiteSpace::BreakSpaces;
    let split_letters = style.word_break == WordBreak::BreakAll;
    let mut pieces: Vec<Piece> = Vec::new();
    let mut current = String::new();
    let mut current_is_space = false;
    let mut flush = |current: &mut String, is_space: bool, pieces: &mut Vec<Piece>| {
        if !current.is_empty() {
            pieces.push(Piece {
                width: run_width(current, style, measure),
                text: std::mem::take(current),
                is_space,
                break_after: wraps && (is_space || split_letters),
            });
        }
    };
    for c in paragraph.chars() {
        let is_space = c == ' ';
        let split = if is_space { split_spaces } else { split_letters };
        if is_space != current_is_space || split {
            flush(&mut current, current_is_space, &mut pieces);
            current_is_space = is_space;
        }
        current.push(c);
    }
    flush(&mut current, current_is_space, &mut pieces);
    // A letter followed by spaces breaks after the spaces, not between them.
    for index in 0..pieces.len().saturating_sub(1) {
        if pieces[index + 1].is_space && !pieces[index].is_space {
            pieces[index].break_after = false;
        }
    }
    pieces
}

/// A line being built.
struct LineBuilder<'s> {
    style: &'s TextStyle,
    pieces: Vec<Piece>,
}

impl LineBuilder<'_> {
    /// The width that counts for fitting: without spaces that hang at the end.
    fn width(&self) -> f64 {
        let hang = self.style.white_space.hangs_trailing_spaces();
        let end = if hang {
            self.pieces
                .iter()
                .rposition(|piece| !piece.is_space)
                .map_or(0, |index| index + 1)
        } else {
            self.pieces.len()
        };
        self.pieces[..end].iter().map(|piece| piece.width).sum()
    }

    fn fits(&self, piece: &Piece, max_width: f64) -> bool {
        let extra = if piece.is_space && self.style.white_space.hangs_trailing_spaces() {
            0.0
        } else {
            self.pending_spaces() + piece.width
        };
        self.width() + extra <= max_width + 1e-6
    }

    /// Spaces at the end that would stop hanging once a word follows.
    fn pending_spaces(&self) -> f64 {
        if !self.style.white_space.hangs_trailing_spaces() {
            return 0.0;
        }
        self.pieces
            .iter()
            .rev()
            .take_while(|piece| piece.is_space)
            .map(|piece| piece.width)
            .sum()
    }

    fn take_line(&mut self, ends_paragraph: bool) -> TextLine {
        let width = self.width();
        let hanging_width = self.pending_spaces();
        let mut pieces = std::mem::take(&mut self.pieces);
        if self.style.white_space.collapses_spaces() {
            while pieces.last().is_some_and(|piece| piece.is_space) {
                pieces.pop();
            }
        }
        TextLine {
            text: pieces.iter().map(|piece| piece.text.as_str()).collect(),
            width,
            ends_paragraph,
            hanging_width: if self.style.white_space.collapses_spaces() {
                0.0
            } else {
                hanging_width
            },
        }
    }
}

/// Breaks `text` into lines no wider than `max_width` where the white-space rules allow it.
/// Use `f64::INFINITY` for the max-content layout.
pub fn break_lines(text: &str, style: &TextStyle, max_width: f64, measure: &mut MeasureRun) -> Vec<TextLine> {
    let mut lines = Vec::new();
    let wraps = style.white_space.wraps();
    for paragraph in paragraphs(text, style) {
        let mut line = LineBuilder {
            style,
            pieces: Vec::new(),
        };
        for piece in pieces(&paragraph, style, measure) {
            if wraps && !line.fits(&piece, max_width) {
                // Break at the last opportunity on the line.
                if let Some(index) = line.pieces.iter().rposition(|piece| piece.break_after) {
                    let rest = line.pieces.split_off(index + 1);
                    lines.push(line.take_line(false));
                    line.pieces = rest;
                }
            }
            let overflows = wraps && !line.fits(&piece, max_width);
            if overflows && !piece.is_space && style.overflow_wrap != OverflowWrap::Normal {
                // A word that can't fit otherwise breaks between letters.
                for c in piece.text.chars() {
                    let c = c.to_string();
                    let letter = Piece {
                        width: run_width(&c, style, measure),
                        text: c,
                        is_space: false,
                        break_after: false,
                    };
                    if !line.fits(&letter, max_width) && !line.pieces.is_empty() {
                        lines.push(line.take_line(false));
                    }
                    line.pieces.push(letter);
                }
                if let Some(last) = line.pieces.last_mut() {
                    last.break_after = piece.break_after;
                }
                continue;
            }
            line.pieces.push(piece);
        }
        lines.push(line.take_line(true));
    }
    lines
}

/// The narrowest width the text can be laid out in without overflowing (its min-content
/// width): the widest piece between break opportunities.
pub fn min_content_width(text: &str, style: &TextStyle, measure: &mut MeasureRun) -> f64 {
    if !style.white_space.wraps() {
        return max_content_width(text, style, measure);
    }
    // Breaks inside words only count for `anywhere`; `break-word` breaks them only when
    // nothing else fits, which doesn't change the min-content width.
    let anywhere = style.overflow_wrap == OverflowWrap::Anywhere;
    let without_emergency_breaks = TextStyle {
        overflow_wrap: OverflowWrap::Normal,
        ..*style
    };
    break_lines(text, &without_emergency_breaks, 0.0, measure)
        .iter()
        .map(|line| {
            if anywhere {
                line.text
                    .chars()
                    .filter(|&c| c != ' ')
                    .map(|c| run_width(&c.to_string(), style, measure))
                    .fold(0.0, f64::max)
            } else {
                line.width
            }
        })
        .fold(0.0, f64::max)
}

/// The width of the text without wrapping (its max-content width).
pub fn max_content_width(text: &str, style: &TextStyle, measure: &mut MeasureRun) -> f64 {
    break_lines(text, style, f64::INFINITY, measure)
        .iter()
        .map(|line| line.width)
        .fold(0.0, f64::max)
}

/// Where a line starts within a box of `width` and how wide it is drawn, for `text-align`.
/// A justified line (not the last of its paragraph) spans the whole box.
pub fn align_line(line: &TextLine, align: TextAlign, width: f64) -> (f64, f64) {
    let free = width - line.width;
    match align {
        TextAlign::Start | TextAlign::Left => (0.0, line.width),
        TextAlign::Justify if !line.ends_paragraph && line.text.contains(' ') => (0.0, width.max(line.width)),
        TextAlign::Justify => (0.0, line.width),
        TextAlign::End | TextAlign::Right => (free, line.width),
        TextAlign::Center => (free / 2.0, line.width),
    }
}

/// Measures text for a measured leaf the way Chrome sizes the anonymous block around an
/// element's text: an unknown width is the fit-content width (between the min- and
/// max-content widths), and the text never wraps narrower than its min-content width.
pub fn measure_text(
    text: &str,
    style: &TextStyle,
    metrics: FontMetrics,
    input: crate::MeasureInput,
    run: &mut MeasureRun,
) -> crate::MeasureOutput {
    use crate::AvailableSpace;

    let max_content = max_content_width(text, style, run);
    let min_content = min_content_width(text, style, run).min(max_content);
    let width = input.known.width.unwrap_or(match input.available.width {
        AvailableSpace::Definite(space) => space.min(max_content).max(min_content),
        AvailableSpace::MinContent => min_content,
        AvailableSpace::MaxContent => max_content,
    });
    let lines = break_lines(text, style, width.max(min_content), run);
    let height = input
        .known
        .height
        .unwrap_or(lines.len() as f64 * metrics.line_height(style));
    crate::MeasureOutput {
        size: crate::Size::new(width, height),
        first_baseline: (!lines.is_empty()).then(|| metrics.baseline(style)),
    }
}

/// A line's rect, relative to the content's box: where the glyphs go horizontally, and the
/// font's ascent-to-descent area vertically (what browsers report for text ranges).
pub fn line_rects(
    text: &str,
    style: &TextStyle,
    metrics: FontMetrics,
    box_width: f64,
    run: &mut MeasureRun,
) -> Vec<[f64; 4]> {
    let min_content = min_content_width(text, style, run);
    let line_height = metrics.line_height(style);
    let half_leading = metrics.half_leading(style);
    let height = metrics.ascent + metrics.descent;
    let mut rects = Vec::new();
    for (index, line) in break_lines(text, style, box_width.max(min_content), run).iter().enumerate() {
        if line.text.is_empty() {
            continue;
        }
        let (x, width) = align_line(line, style.text_align, box_width);
        let y = index as f64 * line_height + half_leading;
        if width > 0.0 {
            rects.push([x, y, width, height]);
        }
        // Hanging spaces are a separate piece of the line in browsers.
        if line.hanging_width > 0.0 {
            rects.push([x + width, y, line.hanging_width, height]);
        }
    }
    rects
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ahem-like: every character is 10px wide.
    fn ahem(text: &str) -> f64 {
        text.chars().count() as f64 * 10.0
    }

    fn lines(text: &str, style: &TextStyle, width: f64) -> Vec<(String, f64)> {
        break_lines(text, style, width, &mut ahem)
            .into_iter()
            .map(|line| (line.text, line.width))
            .collect()
    }

    fn style(white_space: WhiteSpace) -> TextStyle {
        TextStyle {
            white_space,
            ..TextStyle::default()
        }
    }

    #[test]
    fn normal_collapses_and_wraps() {
        let normal = style(WhiteSpace::Normal);
        assert_eq!(lines("  aa   bbbb \n c ", &normal, 60.0), [("aa".into(), 20.0), ("bbbb c".into(), 60.0)]);
        assert_eq!(lines("aa bbbb c", &normal, 70.0), [("aa bbbb".into(), 70.0), ("c".into(), 10.0)]);
        assert_eq!(min_content_width("aa bbbb", &normal, &mut ahem), 40.0);
        assert_eq!(max_content_width("aa bbbb", &normal, &mut ahem), 70.0);
        assert!(lines("   ", &normal, 100.0).is_empty());
    }

    #[test]
    fn pre_keeps_spaces_and_newlines() {
        let pre = style(WhiteSpace::Pre);
        assert_eq!(lines("a  b\n\nc", &pre, 10.0), [("a  b".into(), 40.0), ("".into(), 0.0), ("c".into(), 10.0)]);
        let pre_wrap = style(WhiteSpace::PreWrap);
        // Trailing spaces hang.
        assert_eq!(lines("aa  bb", &pre_wrap, 30.0), [("aa  ".into(), 20.0), ("bb".into(), 20.0)]);
        let pre_line = style(WhiteSpace::PreLine);
        assert_eq!(lines("a   b\nc", &pre_line, 100.0), [("a b".into(), 30.0), ("c".into(), 10.0)]);
    }

    #[test]
    fn nowrap_never_wraps() {
        assert_eq!(lines("aa bb", &style(WhiteSpace::NoWrap), 10.0), [("aa bb".into(), 50.0)]);
        assert_eq!(min_content_width("aa bb", &style(WhiteSpace::NoWrap), &mut ahem), 50.0);
    }

    #[test]
    fn spacing() {
        let spaced = TextStyle {
            letter_spacing: 2.0,
            word_spacing: 5.0,
            ..TextStyle::default()
        };
        // 5 characters with 2px each, one space with 5px more.
        assert_eq!(lines("ab cd", &spaced, 1000.0), [("ab cd".into(), 50.0 + 10.0 + 5.0)]);
    }

    #[test]
    fn breaking_inside_words() {
        let anywhere = TextStyle {
            overflow_wrap: OverflowWrap::Anywhere,
            ..TextStyle::default()
        };
        assert_eq!(
            lines("abcde f", &anywhere, 30.0),
            [("abc".into(), 30.0), ("de".into(), 20.0), ("f".into(), 10.0)]
        );
        assert_eq!(min_content_width("abcde f", &anywhere, &mut ahem), 10.0);
        let break_word = TextStyle {
            overflow_wrap: OverflowWrap::BreakWord,
            ..TextStyle::default()
        };
        assert_eq!(min_content_width("abcde f", &break_word, &mut ahem), 50.0);
        let break_all = TextStyle {
            word_break: WordBreak::BreakAll,
            ..TextStyle::default()
        };
        assert_eq!(lines("ab cdef", &break_all, 40.0), [("ab c".into(), 40.0), ("def".into(), 30.0)]);
    }

    #[test]
    fn transform_and_alignment() {
        let upper = TextStyle {
            text_transform: TextTransform::Capitalize,
            ..TextStyle::default()
        };
        assert_eq!(lines("hello big world", &upper, 1000.0)[0].0, "Hello Big World");
        let line = TextLine {
            text: "ab cd".into(),
            width: 50.0,
            ends_paragraph: false,
            hanging_width: 0.0,
        };
        assert_eq!(align_line(&line, TextAlign::Center, 100.0), (25.0, 50.0));
        assert_eq!(align_line(&line, TextAlign::Right, 100.0), (50.0, 50.0));
        assert_eq!(align_line(&line, TextAlign::Justify, 100.0), (0.0, 100.0));
    }

    #[test]
    fn line_boxes() {
        let metrics = FontMetrics {
            ascent: 8.0,
            descent: 2.0,
            line_gap: 0.0,
        };
        let tall = TextStyle {
            line_height: Some(20.0),
            ..TextStyle::default()
        };
        assert_eq!(metrics.line_height(&tall), 20.0);
        assert_eq!(metrics.baseline(&tall), 13.0);
        assert_eq!(metrics.baseline(&TextStyle::default()), 8.0);
    }
}
