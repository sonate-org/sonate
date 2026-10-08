use super::parser::StyleDeclarationParser;
use crate::style::{CalcNode, Length};
use std::sync::Arc;
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
        let token = input.next()?.clone();
        match token {
            Token::Dimension { value, unit, .. } => {
                length_from_unit(f64::from(value), &unit).ok_or_else(|| input.new_custom_error(()))
            }
            // Numbers without units are treated as pixels.
            Token::Number { value, .. } => Ok(Length::Px(f64::from(value))),
            Token::Percentage { unit_value, .. } => Ok(Length::Percent(f64::from(unit_value) * 100.0)),
            Token::Ident(name) if name.eq_ignore_ascii_case("auto") => Ok(Length::Auto),
            Token::Function(name) if is_math_function(&name) => {
                let node = input.parse_nested_block(|input| {
                    let node = math_function(&name, input)?;
                    input.expect_exhausted()?;
                    Ok(node)
                })?;
                Ok(match node {
                    CalcNode::Value(length) => length,
                    node => Length::Calc(Arc::new(node)),
                })
            }
            _ => Err(input.new_custom_error(())),
        }
    }
}

/// `px`, the font-relative and viewport units, and the absolute units (as px).
fn length_from_unit(value: f64, unit: &str) -> Option<Length> {
    Some(match unit.to_ascii_lowercase().as_str() {
        "px" => Length::Px(value),
        "em" => Length::Em(value),
        "rem" => Length::Rem(value),
        "vw" => Length::Vw(value),
        "vh" => Length::Vh(value),
        "vmin" => Length::Vmin(value),
        "vmax" => Length::Vmax(value),
        "in" => Length::Px(value * 96.0),
        "cm" => Length::Px(value * 96.0 / 2.54),
        "mm" => Length::Px(value * 96.0 / 25.4),
        "q" => Length::Px(value * 96.0 / 101.6),
        "pt" => Length::Px(value * 96.0 / 72.0),
        "pc" => Length::Px(value * 16.0),
        _ => return None,
    })
}

fn is_math_function(name: &str) -> bool {
    ["calc", "min", "max", "clamp"]
        .iter()
        .any(|function| name.eq_ignore_ascii_case(function))
}

/// An operand of a math expression: a factor or a length.
enum Operand {
    Number(f64),
    Length(CalcNode),
}

impl Operand {
    fn into_length<'i>(self, input: &Parser<'i, '_>) -> Result<CalcNode, ParseError<'i, ()>> {
        match self {
            Operand::Length(node) => Ok(node),
            // A bare 0 is a length.
            Operand::Number(number) if number == 0.0 => Ok(CalcNode::Value(Length::Px(0.0))),
            Operand::Number(_) => Err(input.new_custom_error(())),
        }
    }
}

/// The arguments of `calc()`, `min()`, `max()` or `clamp()`.
fn math_function<'i>(name: &str, input: &mut Parser<'i, '_>) -> Result<CalcNode, ParseError<'i, ()>> {
    let mut arguments = input.parse_comma_separated(|input| sum(input)?.into_length(input))?;
    match (name.to_ascii_lowercase().as_str(), arguments.len()) {
        ("calc", 1) => Ok(arguments.pop().unwrap()),
        ("min", _) => Ok(CalcNode::Min(arguments)),
        ("max", _) => Ok(CalcNode::Max(arguments)),
        ("clamp", 3) => {
            let max = arguments.pop().unwrap();
            let value = arguments.pop().unwrap();
            let min = arguments.pop().unwrap();
            Ok(CalcNode::Clamp(Box::new(min), Box::new(value), Box::new(max)))
        }
        _ => Err(input.new_custom_error(())),
    }
}

/// `product [ [ + | - ] product ]*`; `+` and `-` must be surrounded by whitespace, which the
/// tokenizer enforces (`-10px` is a single token).
fn sum<'i>(input: &mut Parser<'i, '_>) -> Result<Operand, ParseError<'i, ()>> {
    let mut terms = vec![product(input)?];
    loop {
        let state = input.state();
        let negate = match input.next() {
            Ok(Token::Delim('+')) => false,
            Ok(Token::Delim('-')) => true,
            _ => {
                input.reset(&state);
                break;
            }
        };
        let term = product(input)?;
        terms.push(if negate { scale(term, -1.0) } else { term });
    }
    if terms.len() == 1 {
        return Ok(terms.pop().unwrap());
    }
    if terms.iter().all(|term| matches!(term, Operand::Number(_))) {
        let total = terms
            .iter()
            .map(|term| match term {
                Operand::Number(number) => *number,
                Operand::Length(_) => 0.0,
            })
            .sum();
        return Ok(Operand::Number(total));
    }
    let terms = terms
        .into_iter()
        .map(|term| term.into_length(input))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Operand::Length(CalcNode::Sum(terms)))
}

/// `value [ [ * | / ] value ]*`; lengths can only be multiplied or divided by numbers.
fn product<'i>(input: &mut Parser<'i, '_>) -> Result<Operand, ParseError<'i, ()>> {
    let mut value = operand(input)?;
    loop {
        let state = input.state();
        let divide = match input.next() {
            Ok(Token::Delim('*')) => false,
            Ok(Token::Delim('/')) => true,
            _ => {
                input.reset(&state);
                break;
            }
        };
        let other = operand(input)?;
        value = match (value, other, divide) {
            (Operand::Number(a), Operand::Number(b), false) => Operand::Number(a * b),
            (Operand::Number(a), Operand::Number(b), true) if b != 0.0 => Operand::Number(a / b),
            (Operand::Length(node), Operand::Number(factor), false)
            | (Operand::Number(factor), Operand::Length(node), false) => {
                Operand::Length(CalcNode::Scale(Box::new(node), factor))
            }
            (Operand::Length(node), Operand::Number(divisor), true) if divisor != 0.0 => {
                Operand::Length(CalcNode::Scale(Box::new(node), 1.0 / divisor))
            }
            _ => return Err(input.new_custom_error(())),
        };
    }
    Ok(value)
}

fn operand<'i>(input: &mut Parser<'i, '_>) -> Result<Operand, ParseError<'i, ()>> {
    match input.next()?.clone() {
        Token::Number { value, .. } => Ok(Operand::Number(f64::from(value))),
        Token::Dimension { value, unit, .. } => length_from_unit(f64::from(value), &unit)
            .map(|length| Operand::Length(CalcNode::Value(length)))
            .ok_or_else(|| input.new_custom_error(())),
        Token::Percentage { unit_value, .. } => Ok(Operand::Length(CalcNode::Value(Length::Percent(
            f64::from(unit_value) * 100.0,
        )))),
        Token::ParenthesisBlock => input.parse_nested_block(|input| {
            let value = sum(input)?;
            input.expect_exhausted()?;
            Ok(value)
        }),
        Token::Function(name) if is_math_function(&name) => input.parse_nested_block(|input| {
            let node = math_function(&name, input)?;
            input.expect_exhausted()?;
            Ok(Operand::Length(node))
        }),
        _ => Err(input.new_custom_error(())),
    }
}

fn scale(operand: Operand, factor: f64) -> Operand {
    match operand {
        Operand::Number(number) => Operand::Number(number * factor),
        Operand::Length(node) => Operand::Length(CalcNode::Scale(Box::new(node), factor)),
    }
}
