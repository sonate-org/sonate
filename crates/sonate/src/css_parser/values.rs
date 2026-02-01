use super::parser::StyleDeclarationParser;
use crate::style::Length;
use cssparser::{ParseError, Parser, Token};

impl StyleDeclarationParser {
    fn normalize_hue_degrees(hue_degrees: f32) -> f32 {
        if !hue_degrees.is_finite() {
            return 0.0;
        }
        let mut hue = hue_degrees % 360.0;
        if hue < 0.0 {
            hue += 360.0;
        }
        hue
    }

    /// Parse a CSS `<angle>` value and return degrees normalized to [0, 360).
    ///
    /// Supported forms:
    /// - `<number>` (interpreted as degrees)
    /// - `<angle>` units: `deg`, `rad`, `grad`, `turn`
    pub(crate) fn parse_angle_degrees<'i, 't>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<f32, ParseError<'i, ()>> {
        let token = input.next()?;
        let degrees = match token {
            Token::Number { value, .. } => *value as f32,
            Token::Dimension { value, unit, .. } => match unit.as_ref() {
                "deg" => *value as f32,
                "grad" => (*value as f32) * 0.9,
                "rad" => (*value as f32) * (180.0 / std::f32::consts::PI),
                "turn" => (*value as f32) * 360.0,
                _ => return Err(input.new_error_for_next_token()),
            },
            _ => return Err(input.new_error_for_next_token()),
        };

        Ok(Self::normalize_hue_degrees(degrees))
    }

    pub(crate) fn parse_percentage<'i, 't>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<f32, ParseError<'i, ()>> {
        let token = input.next()?;
        match token {
            Token::Percentage { unit_value, .. } => Ok((*unit_value as f32) * 100.0),
            _ => Err(input.new_error_for_next_token()),
        }
    }

    pub(crate) fn parse_length_value<'i, 't>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Length, ParseError<'i, ()>> {
        let token = input.next()?;
        match token {
            Token::Dimension { value, unit, .. } => match unit.as_ref() {
                "px" => Ok(Length::Px(*value as f64)),
                "em" => Ok(Length::Em(*value as f64)),
                "%" => Ok(Length::Percent(*value as f64)),
                _ => Err(input.new_error_for_next_token()),
            },
            Token::Number { value, .. } => {
                // Numbers without units are treated as pixels
                Ok(Length::Px(*value as f64))
            }
            Token::Percentage { unit_value, .. } => {
                // Handle percentage values
                Ok(Length::Percent(*unit_value as f64 * 100.0))
            }
            Token::Ident(name) => match name.as_ref() {
                "auto" => Ok(Length::Auto),
                _ => Err(input.new_error_for_next_token()),
            },
            _ => Err(input.new_error_for_next_token()),
        }
    }
}
