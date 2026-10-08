//! Values of the visual effect properties: `box-shadow` (CSS Backgrounds Level 3) and
//! `transform` / `transform-origin` (CSS Transforms Level 1, 2D functions).

use super::parser::StyleDeclarationParser;
use crate::style::{BoxShadow, Length, TransformFunction};
use cssparser::{ParseError, Parser, Token};

impl StyleDeclarationParser {
    /// `none | [ inset? && <length>{2,4} && <color>? ]#`
    pub(crate) fn parse_box_shadow<'i, 't>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Vec<BoxShadow>, ParseError<'i, ()>> {
        if input
            .try_parse(|input| input.expect_ident_matching("none"))
            .is_ok()
        {
            return Ok(Vec::new());
        }
        input.parse_comma_separated(|input| {
            let mut inset = false;
            let mut color = None;
            let mut lengths = Vec::new();
            while !input.is_exhausted() {
                if !inset
                    && input
                        .try_parse(|input| input.expect_ident_matching("inset"))
                        .is_ok()
                {
                    inset = true;
                } else if let Some(parsed) = color
                    .is_none()
                    .then(|| input.try_parse(|input| self.parse_color_value(input)).ok())
                    .flatten()
                {
                    color = Some(parsed);
                } else if lengths.len() < 4 {
                    lengths.push(self.parse_length_value(input)?);
                } else {
                    return Err(input.new_custom_error(()));
                }
            }
            // The blur radius can't be negative.
            if lengths.len() < 2 || matches!(lengths.get(2), Some(Length::Px(blur)) if *blur < 0.0)
            {
                return Err(input.new_custom_error(()));
            }
            let mut lengths = lengths.into_iter();
            Ok(BoxShadow {
                offset_x: lengths.next().unwrap(),
                offset_y: lengths.next().unwrap(),
                blur: lengths.next().unwrap_or(Length::Px(0.0)),
                spread: lengths.next().unwrap_or(Length::Px(0.0)),
                color,
                inset,
            })
        })
    }

    /// `none | <transform-function>+`
    pub(crate) fn parse_transform<'i, 't>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Vec<TransformFunction>, ParseError<'i, ()>> {
        if input
            .try_parse(|input| input.expect_ident_matching("none"))
            .is_ok()
        {
            return Ok(Vec::new());
        }
        let mut functions = Vec::new();
        while !input.is_exhausted() {
            let name = input.expect_function()?.to_ascii_lowercase();
            let function = input.parse_nested_block(|input| {
                let mut lengths = |input: &mut Parser<'i, '_>, count: usize| {
                    let mut values = vec![self.parse_length_value(input)?];
                    while values.len() < count
                        && input.try_parse(|input| input.expect_comma()).is_ok()
                    {
                        values.push(self.parse_length_value(input)?);
                    }
                    Ok::<_, ParseError<'i, ()>>(values)
                };
                let function = match name.as_str() {
                    "translate" => {
                        let mut values = lengths(input, 2)?.into_iter();
                        let x = values.next().unwrap();
                        TransformFunction::Translate(x, values.next().unwrap_or(Length::Px(0.0)))
                    }
                    "translatex" => TransformFunction::Translate(
                        self.parse_length_value(input)?,
                        Length::Px(0.0),
                    ),
                    "translatey" => TransformFunction::Translate(
                        Length::Px(0.0),
                        self.parse_length_value(input)?,
                    ),
                    "scale" => {
                        let x = number_or_percentage(input)?;
                        let y = if input.try_parse(|input| input.expect_comma()).is_ok() {
                            number_or_percentage(input)?
                        } else {
                            x
                        };
                        TransformFunction::Scale(x, y)
                    }
                    "scalex" => TransformFunction::Scale(number_or_percentage(input)?, 1.0),
                    "scaley" => TransformFunction::Scale(1.0, number_or_percentage(input)?),
                    "rotate" => TransformFunction::Rotate(self.parse_transform_angle(input)?),
                    "skew" => {
                        let x = self.parse_transform_angle(input)?;
                        let y = if input.try_parse(|input| input.expect_comma()).is_ok() {
                            self.parse_transform_angle(input)?
                        } else {
                            0.0
                        };
                        TransformFunction::Skew(x, y)
                    }
                    "skewx" => TransformFunction::Skew(self.parse_transform_angle(input)?, 0.0),
                    "skewy" => TransformFunction::Skew(0.0, self.parse_transform_angle(input)?),
                    "matrix" => {
                        let mut values = [0.0; 6];
                        for (index, value) in values.iter_mut().enumerate() {
                            if index > 0 {
                                input.expect_comma()?;
                            }
                            *value = f64::from(input.expect_number()?);
                        }
                        TransformFunction::Matrix(values)
                    }
                    _ => return Err(input.new_custom_error(())),
                };
                Ok(function)
            })?;
            functions.push(function);
        }
        Ok(functions)
    }

    /// An angle in degrees; a bare `0` is allowed.
    fn parse_transform_angle<'i, 't>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<f64, ParseError<'i, ()>> {
        match input.next()?.clone() {
            Token::Number { value: 0.0, .. } => Ok(0.0),
            Token::Dimension { value, unit, .. } => Ok(f64::from(value)
                * match unit.to_ascii_lowercase().as_str() {
                    "deg" => 1.0,
                    "grad" => 0.9,
                    "rad" => 180.0 / std::f64::consts::PI,
                    "turn" => 360.0,
                    _ => return Err(input.new_custom_error(())),
                }),
            _ => Err(input.new_custom_error(())),
        }
    }

    /// One or two positions (`left`, `center`, `bottom`, lengths, percentages); a third value
    /// (z) is ignored.
    pub(crate) fn parse_transform_origin<'i, 't>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<(Length, Length), ParseError<'i, ()>> {
        let mut values = Vec::new();
        while !input.is_exhausted() && values.len() < 3 {
            if let Ok(keyword) = input.try_parse(|input| input.expect_ident_cloned()) {
                let (value, axis) = match keyword.to_ascii_lowercase().as_str() {
                    "left" => (Length::Percent(0.0), Some('x')),
                    "right" => (Length::Percent(100.0), Some('x')),
                    "top" => (Length::Percent(0.0), Some('y')),
                    "bottom" => (Length::Percent(100.0), Some('y')),
                    "center" => (Length::Percent(50.0), None),
                    _ => return Err(input.new_custom_error(())),
                };
                values.push((value, axis));
            } else {
                values.push((self.parse_length_value(input)?, None));
            }
        }
        let center = Length::Percent(50.0);
        Ok(match values.as_slice() {
            [(value, Some('y'))] => (center, value.clone()),
            [(value, _)] => (value.clone(), center),
            [(first, first_axis), (second, second_axis), ..] => {
                // Keywords may come in either order (`top left`).
                if *first_axis == Some('y') || *second_axis == Some('x') {
                    (second.clone(), first.clone())
                } else {
                    (first.clone(), second.clone())
                }
            }
            [] => return Err(input.new_custom_error(())),
        })
    }
}

fn number_or_percentage<'i>(input: &mut Parser<'i, '_>) -> Result<f64, ParseError<'i, ()>> {
    match input.next()?.clone() {
        Token::Number { value, .. } => Ok(f64::from(value)),
        Token::Percentage { unit_value, .. } => Ok(f64::from(unit_value)),
        _ => Err(input.new_custom_error(())),
    }
}
