//! Font and text values (CSS Fonts Level 3, CSS Text Level 3) and `@font-face`.

use super::parser::StyleDeclarationParser;
use crate::style::{FontFace, FontSource, FontStyle, FontWeight, Length, LineHeight, Style};
use cssparser::{ParseError, Parser, Token};

const GENERIC_FAMILIES: &[&str] = &[
    "serif",
    "sans-serif",
    "monospace",
    "cursive",
    "fantasy",
    "system-ui",
    "ui-serif",
    "ui-sans-serif",
    "ui-monospace",
    "ui-rounded",
    "math",
    "emoji",
];

impl StyleDeclarationParser {
    /// A length, a percentage of the parent's font size, or a size keyword.
    pub(crate) fn parse_font_size<'i, 't>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Length, ParseError<'i, ()>> {
        if let Ok(keyword) = input.try_parse(|input| input.expect_ident_cloned()) {
            return Ok(match keyword.to_ascii_lowercase().as_str() {
                "xx-small" => Length::Px(9.0),
                "x-small" => Length::Px(10.0),
                "small" => Length::Px(13.0),
                "medium" => Length::Px(16.0),
                "large" => Length::Px(18.0),
                "x-large" => Length::Px(24.0),
                "xx-large" => Length::Px(32.0),
                "xxx-large" => Length::Px(48.0),
                "larger" => Length::Percent(120.0),
                "smaller" => Length::Percent(100.0 / 1.2),
                _ => return Err(input.new_custom_error(())),
            });
        }
        self.parse_length_value(input)
    }

    /// A comma-separated family list, serialized like browsers do: generic families and
    /// identifier names bare, other names quoted.
    pub(crate) fn parse_font_family<'i, 't>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<String, ParseError<'i, ()>> {
        let families = input.parse_comma_separated(|input| {
            if let Ok(name) = input.try_parse(|input| input.expect_string_cloned()) {
                return Ok(serialize_family(&name, true));
            }
            let mut words = vec![input.expect_ident_cloned()?.to_string()];
            while let Ok(word) = input.try_parse(|input| input.expect_ident_cloned()) {
                words.push(word.to_string());
            }
            Ok(serialize_family(&words.join(" "), false))
        })?;
        Ok(families.join(", "))
    }

    pub(crate) fn parse_font_weight<'i, 't>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<FontWeight, ParseError<'i, ()>> {
        if let Ok(number) = input.try_parse(|input| input.expect_number()) {
            if (1.0..=1000.0).contains(&number) {
                return Ok(FontWeight::Number(number.round() as u16));
            }
            return Err(input.new_custom_error(()));
        }
        let keyword = input.expect_ident_cloned()?;
        Ok(match keyword.to_ascii_lowercase().as_str() {
            "normal" => FontWeight::Number(400),
            "bold" => FontWeight::Number(700),
            "bolder" => FontWeight::Bolder,
            "lighter" => FontWeight::Lighter,
            _ => return Err(input.new_custom_error(())),
        })
    }

    pub(crate) fn parse_font_style<'i, 't>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<FontStyle, ParseError<'i, ()>> {
        let keyword = input.expect_ident_cloned()?;
        let style = match keyword.to_ascii_lowercase().as_str() {
            "normal" => FontStyle::Normal,
            "italic" => FontStyle::Italic,
            "oblique" => FontStyle::Oblique,
            _ => return Err(input.new_custom_error(())),
        };
        if style == FontStyle::Oblique {
            // An optional angle, which Sonate doesn't use.
            let _ = input.try_parse(|input| match input.next()? {
                Token::Dimension { .. } => Ok(()),
                _ => Err(input.new_custom_error::<(), ()>(())),
            });
        }
        Ok(style)
    }

    /// `normal`, a number (a multiple of the font size), a length or a percentage.
    pub(crate) fn parse_line_height<'i, 't>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<LineHeight, ParseError<'i, ()>> {
        if input
            .try_parse(|input| input.expect_ident_matching("normal"))
            .is_ok()
        {
            return Ok(LineHeight::Normal);
        }
        if let Ok(number) = input.try_parse(|input| input.expect_number()) {
            return Ok(LineHeight::Number(f64::from(number).max(0.0)));
        }
        Ok(LineHeight::Length(self.parse_length_value(input)?))
    }

    /// `letter-spacing` / `word-spacing`: `normal` or a length.
    pub(crate) fn parse_spacing<'i, 't>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Length, ParseError<'i, ()>> {
        if input
            .try_parse(|input| input.expect_ident_matching("normal"))
            .is_ok()
        {
            // `Auto` stands for `normal`.
            return Ok(Length::Auto);
        }
        self.parse_length_value(input)
    }

    /// `font: [ <style> || <variant> || <weight> || <stretch> ]? <size> [ / <line-height> ]?
    /// <family>`. Parts that are left out are reset.
    pub(crate) fn parse_font_shorthand<'i, 't>(
        &mut self,
        input: &mut Parser<'i, 't>,
        style: &mut Style,
    ) -> Result<(), ParseError<'i, ()>> {
        let mut font_style = None;
        let mut weight = None;
        for _ in 0..4 {
            let state = input.state();
            match input.next()?.clone() {
                Token::Ident(ident) => match ident.to_ascii_lowercase().as_str() {
                    "normal" => {}
                    "italic" if font_style.is_none() => font_style = Some(FontStyle::Italic),
                    "oblique" if font_style.is_none() => font_style = Some(FontStyle::Oblique),
                    "bold" if weight.is_none() => weight = Some(FontWeight::Number(700)),
                    "bolder" if weight.is_none() => weight = Some(FontWeight::Bolder),
                    "lighter" if weight.is_none() => weight = Some(FontWeight::Lighter),
                    // Variants and stretches don't change Sonate's rendering.
                    "small-caps" | "ultra-condensed" | "extra-condensed" | "condensed"
                    | "semi-condensed" | "semi-expanded" | "expanded" | "extra-expanded"
                    | "ultra-expanded" => {}
                    _ => {
                        input.reset(&state);
                        break;
                    }
                },
                Token::Number { value, .. } if weight.is_none() && (1.0..=1000.0).contains(&value) => {
                    weight = Some(FontWeight::Number(value.round() as u16));
                }
                _ => {
                    input.reset(&state);
                    break;
                }
            }
        }

        style.font_size = Some(self.parse_font_size(input)?);
        style.line_height = Some(
            if input.try_parse(|input| input.expect_delim('/')).is_ok() {
                self.parse_line_height(input)?
            } else {
                LineHeight::Normal
            },
        );
        style.font_family = Some(self.parse_font_family(input)?);
        style.font_style = Some(font_style.unwrap_or_default());
        style.font_weight = Some(weight.unwrap_or(FontWeight::Number(400)));
        Ok(())
    }

    /// The descriptors of a `@font-face` rule.
    pub(crate) fn parse_font_face_descriptor<'i, 't>(
        &mut self,
        name: &str,
        input: &mut Parser<'i, 't>,
        face: &mut FontFace,
    ) -> Result<(), ParseError<'i, ()>> {
        match name.to_ascii_lowercase().as_str() {
            "font-family" => {
                let family = self.parse_font_family(input)?;
                face.family = family.trim_matches('"').to_owned();
            }
            "src" => {
                face.sources = input.parse_comma_separated(|input| {
                    let source = match input.next()?.clone() {
                        Token::UnquotedUrl(url) => FontSource::Url(url.to_string()),
                        Token::Function(name) if name.eq_ignore_ascii_case("url") => {
                            input.parse_nested_block(|input| {
                                Ok(FontSource::Url(input.expect_string_cloned()?.to_string()))
                            })?
                        }
                        Token::Function(name) if name.eq_ignore_ascii_case("local") => {
                            input.parse_nested_block(|input| {
                                let family = self.parse_font_family(input)?;
                                Ok(FontSource::Local(family.trim_matches('"').to_owned()))
                            })?
                        }
                        _ => return Err(input.new_custom_error(())),
                    };
                    // `format()` and `tech()` hints are ignored.
                    while input.next().is_ok() {}
                    Ok(source)
                })?;
            }
            "font-weight" => {
                let FontWeight::Number(weight) = self.parse_font_weight(input)? else {
                    return Err(input.new_custom_error(()));
                };
                face.weight = weight;
                // The upper end of a weight range.
                let _ = input.try_parse(|input| input.expect_number());
            }
            "font-style" => face.italic = self.parse_font_style(input)? != FontStyle::Normal,
            // Other descriptors (unicode-range, font-display, …) are ignored.
            _ => while input.next().is_ok() {},
        }
        Ok(())
    }
}

/// Serializes a family name like browsers do in computed values.
fn serialize_family(name: &str, quoted: bool) -> String {
    let is_identifier = !name.is_empty()
        && !name.starts_with(|c: char| c.is_ascii_digit())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    let generic = GENERIC_FAMILIES.contains(&name.to_ascii_lowercase().as_str());
    if generic && !quoted {
        name.to_ascii_lowercase()
    } else if is_identifier && !generic {
        name.to_owned()
    } else {
        format!("\"{}\"", name.replace('"', "\\\""))
    }
}
