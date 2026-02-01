use super::named_colors;
use super::parser::StyleDeclarationParser;
use crate::style::Rgba;
use cssparser::{ParseError, Parser, Token};

impl StyleDeclarationParser {
    fn clamp_u8(value: f32) -> u8 {
        value.clamp(0.0, 255.0).round() as u8
    }

    fn parse_rgb_channel<'i, 't>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<u8, ParseError<'i, ()>> {
        let token = input.next()?;
        match token {
            Token::Number { value, .. } => Ok(Self::clamp_u8(*value as f32)),
            Token::Percentage { unit_value, .. } => {
                Ok(Self::clamp_u8((*unit_value as f32) * 255.0))
            }
            _ => Err(input.new_error_for_next_token()),
        }
    }

    fn parse_rgb_channel_or_none<'i, 't>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<u8, ParseError<'i, ()>> {
        // CSS Color 4 modern rgb() syntax allows `none` components. We can't represent
        // missing channels here, so treat them as 0.
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(0);
        }
        self.parse_rgb_channel(input)
    }

    fn parse_alpha_channel<'i, 't>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<u8, ParseError<'i, ()>> {
        let token = input.next()?;
        let alpha_0_1 = match token {
            Token::Number { value, .. } => *value as f32,
            Token::Percentage { unit_value, .. } => *unit_value as f32,
            _ => return Err(input.new_error_for_next_token()),
        };

        Ok((alpha_0_1.clamp(0.0, 1.0) * 255.0).round() as u8)
    }

    fn parse_hue_value<'i, 't>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<f32, ParseError<'i, ()>> {
        // Modern syntax allows 'none' (missing component). We can't represent
        // missing here, so treat it as 0deg.
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(0.0);
        }

        self.parse_angle_degrees(input)
    }

    fn parse_hsl_percent_or_number<'i, 't>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<f32, ParseError<'i, ()>> {
        let token = input.next()?;
        match token {
            Token::Percentage { unit_value, .. } => Ok((*unit_value as f32) * 100.0),
            Token::Number { value, .. } => Ok(*value as f32),
            Token::Ident(name) if name.eq_ignore_ascii_case("none") => Ok(0.0),
            _ => Err(input.new_error_for_next_token()),
        }
    }

    fn parse_hwb_percent_or_number<'i, 't>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<f32, ParseError<'i, ()>> {
        // HWB W and B allow mixed <percentage>/<number> in modern syntax, and also allow 'none'.
        self.parse_hsl_percent_or_number(input)
    }

    fn parse_alpha_value_u8<'i, 't>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<u8, ParseError<'i, ()>> {
        // <alpha-value> = <number> | <percentage> in Level 4.
        // Modern syntax also allows 'none'.
        let token = input.next()?;
        let alpha_0_1 = match token {
            Token::Number { value, .. } => *value as f32,
            Token::Percentage { unit_value, .. } => *unit_value as f32,
            Token::Ident(name) if name.eq_ignore_ascii_case("none") => 1.0,
            _ => return Err(input.new_error_for_next_token()),
        };
        Ok((alpha_0_1.clamp(0.0, 1.0) * 255.0).round() as u8)
    }

    fn hsl_to_rgb_u8(
        hue_degrees: f32,
        saturation_0_100: f32,
        lightness_0_100: f32,
    ) -> (u8, u8, u8) {
        // Conversion algorithm based on CSS Color 4 §7.1 (sample implementation).
        let hue = hue_degrees;
        let sat = (saturation_0_100.max(0.0).min(100.0)) / 100.0;
        let light = (lightness_0_100.max(0.0).min(100.0)) / 100.0;

        fn f(n: f32, hue: f32, sat: f32, light: f32) -> f32 {
            let k = (n + hue / 30.0) % 12.0;
            let a = sat * light.min(1.0 - light);
            let m = (k - 3.0).min(9.0 - k).min(1.0);
            light - a * (-1.0_f32).max(m)
        }

        let r = Self::clamp_u8(f(0.0, hue, sat, light) * 255.0);
        let g = Self::clamp_u8(f(8.0, hue, sat, light) * 255.0);
        let b = Self::clamp_u8(f(4.0, hue, sat, light) * 255.0);
        (r, g, b)
    }

    fn parse_hsl_color<'i, 't>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Rgba, ParseError<'i, ()>> {
        // Supports both legacy comma-separated and modern space-separated syntax.
        // See CSS Color 4 §7.
        let hue = self.parse_hue_value(input)?;

        // Legacy: hsl(<hue>, <percentage>, <percentage>[, <alpha>]?)
        if input.try_parse(|i| i.expect_comma()).is_ok() {
            let sat = self.parse_percentage(input)?;
            input.expect_comma()?;
            let light = self.parse_percentage(input)?;

            let a = input
                .try_parse(|i| {
                    i.expect_comma()?;
                    self.parse_alpha_value_u8(i)
                })
                .unwrap_or(255);

            // Historical behavior: negative saturation clamped to 0 at parse time.
            let sat = sat.max(0.0);
            let (r, g, b) = Self::hsl_to_rgb_u8(hue, sat, light);
            return Ok(Rgba { r, g, b, a });
        }

        // Modern: hsl(<hue> <sat> <light> [ / <alpha> ]?)
        let sat = self.parse_hsl_percent_or_number(input)?.max(0.0);
        let light = self.parse_hsl_percent_or_number(input)?;

        let a = if input.try_parse(|i| i.expect_delim('/')).is_ok() {
            self.parse_alpha_value_u8(input)?
        } else {
            255
        };

        let (r, g, b) = Self::hsl_to_rgb_u8(hue, sat, light);
        Ok(Rgba { r, g, b, a })
    }

    fn hwb_to_rgb_u8(hue_degrees: f32, white_0_100: f32, black_0_100: f32) -> (u8, u8, u8) {
        // Conversion algorithm based on CSS Color 4 §8.1 (sample implementation).
        // Note: values outside [0,100] are not invalid; we only clamp negative values.
        let hue = hue_degrees;
        let white = (white_0_100.max(0.0)) / 100.0;
        let black = (black_0_100.max(0.0)) / 100.0;

        if white + black >= 1.0 {
            let gray = white / (white + black);
            let v = Self::clamp_u8(gray * 255.0);
            return (v, v, v);
        }

        // Base color is the fully saturated hue at 50% lightness.
        let (base_r, base_g, base_b) = Self::hsl_to_rgb_u8(hue, 100.0, 50.0);
        let mut r = (base_r as f32) / 255.0;
        let mut g = (base_g as f32) / 255.0;
        let mut b = (base_b as f32) / 255.0;

        let factor = 1.0 - white - black;
        r = r * factor + white;
        g = g * factor + white;
        b = b * factor + white;

        (
            Self::clamp_u8(r * 255.0),
            Self::clamp_u8(g * 255.0),
            Self::clamp_u8(b * 255.0),
        )
    }

    fn parse_hwb_color<'i, 't>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Rgba, ParseError<'i, ()>> {
        // CSS Color 4 §8: hwb() does NOT support legacy comma-separated syntax.
        // Using commas inside hwb() is an error.
        let hue = self.parse_hue_value(input)?;

        if input.try_parse(|i| i.expect_comma()).is_ok() {
            return Err(input.new_error_for_next_token());
        }

        let white = self.parse_hwb_percent_or_number(input)?;
        let black = self.parse_hwb_percent_or_number(input)?;

        let a = if input.try_parse(|i| i.expect_delim('/')).is_ok() {
            self.parse_alpha_value_u8(input)?
        } else {
            255
        };

        let (r, g, b) = Self::hwb_to_rgb_u8(hue, white, black);
        Ok(Rgba { r, g, b, a })
    }

    pub(crate) fn parse_color_value<'i, 't>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Rgba, ParseError<'i, ()>> {
        let token = input.next()?;
        match token {
            Token::Ident(name) => named_colors::named_color(name.as_ref())
                .ok_or_else(|| input.new_error_for_next_token()),
            Token::Function(name) => {
                let func = name.as_ref();
                if func.eq_ignore_ascii_case("rgb") {
                    input.parse_nested_block(|input| {
                        // Legacy: rgb(<c>, <c>, <c>)
                        // Modern: rgb(<c> <c> <c> [ / <alpha> ]?)
                        let r = self.parse_rgb_channel_or_none(input)?;

                        if input.try_parse(|i| i.expect_comma()).is_ok() {
                            let g = self.parse_rgb_channel(input)?;
                            input.expect_comma()?;
                            let b = self.parse_rgb_channel(input)?;
                            return Ok(Rgba { r, g, b, a: 255 });
                        }

                        let g = self.parse_rgb_channel_or_none(input)?;
                        let b = self.parse_rgb_channel_or_none(input)?;
                        let a = if input.try_parse(|i| i.expect_delim('/')).is_ok() {
                            self.parse_alpha_value_u8(input)?
                        } else {
                            255
                        };

                        Ok(Rgba { r, g, b, a })
                    })
                } else if func.eq_ignore_ascii_case("rgba") {
                    input.parse_nested_block(|input| {
                        // Legacy: rgba(<c>, <c>, <c>, <alpha>)
                        // Modern: rgba(<c> <c> <c> [ / <alpha> ]?)
                        // (Modern syntax is identical to rgb(); keep `rgba()` accepted for web compatibility.)
                        let r = self.parse_rgb_channel_or_none(input)?;

                        if input.try_parse(|i| i.expect_comma()).is_ok() {
                            let g = self.parse_rgb_channel(input)?;
                            input.expect_comma()?;
                            let b = self.parse_rgb_channel(input)?;
                            input.expect_comma()?;
                            let a = self.parse_alpha_channel(input)?;
                            return Ok(Rgba { r, g, b, a });
                        }

                        let g = self.parse_rgb_channel_or_none(input)?;
                        let b = self.parse_rgb_channel_or_none(input)?;
                        let a = if input.try_parse(|i| i.expect_delim('/')).is_ok() {
                            self.parse_alpha_value_u8(input)?
                        } else {
                            255
                        };

                        Ok(Rgba { r, g, b, a })
                    })
                } else if func.eq_ignore_ascii_case("hsl") || func.eq_ignore_ascii_case("hsla") {
                    input.parse_nested_block(|input| self.parse_hsl_color(input))
                } else if func.eq_ignore_ascii_case("hwb") {
                    input.parse_nested_block(|input| self.parse_hwb_color(input))
                } else {
                    Err(input.new_error_for_next_token())
                }
            }
            Token::Hash(hex) | Token::IDHash(hex) => {
                // Parse hex colors like #ff0000
                parse_hex_color(&hex).map_err(|_| input.new_error_for_next_token())
            }
            _ => Err(input.new_error_for_next_token()),
        }
    }
}

/// Parse a hex color string into Rgba
fn parse_hex_color(hex: &str) -> Result<Rgba, &'static str> {
    let hex = hex.trim_start_matches('#');

    match hex.len() {
        3 => {
            // #rgb -> #rrggbb
            let r =
                u8::from_str_radix(&hex[0..1].repeat(2), 16).map_err(|_| "Invalid hex digit")?;
            let g =
                u8::from_str_radix(&hex[1..2].repeat(2), 16).map_err(|_| "Invalid hex digit")?;
            let b =
                u8::from_str_radix(&hex[2..3].repeat(2), 16).map_err(|_| "Invalid hex digit")?;
            Ok(Rgba { r, g, b, a: 255 })
        }
        6 => {
            // #rrggbb
            let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| "Invalid hex digit")?;
            let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| "Invalid hex digit")?;
            let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| "Invalid hex digit")?;
            Ok(Rgba { r, g, b, a: 255 })
        }
        8 => {
            // #rrggbbaa
            let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| "Invalid hex digit")?;
            let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| "Invalid hex digit")?;
            let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| "Invalid hex digit")?;
            let a = u8::from_str_radix(&hex[6..8], 16).map_err(|_| "Invalid hex digit")?;
            Ok(Rgba { r, g, b, a })
        }
        _ => Err("Invalid hex color length"),
    }
}
