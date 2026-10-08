//! Parses the `calc()`, `min()`, `max()` and `clamp()` values that Chrome serializes as computed
//! values, e.g. `calc(50% - 10px)` or `min(100%, 600px)`.

use crate::Calc;

/// A parsed operand: a plain number (a factor) or a length expression.
enum Value {
    Number(f64),
    Length(Calc),
}

pub fn parse_calc(text: &str) -> Result<Calc, String> {
    let mut parser = Parser {
        chars: text.chars().collect(),
        position: 0,
    };
    let value = parser.function_or_operand()?;
    parser.skip_whitespace();
    if parser.position != parser.chars.len() {
        return Err(format!("unexpected input in `{text}`"));
    }
    match value {
        Value::Length(calc) => Ok(calc),
        Value::Number(_) => Err(format!("`{text}` is not a length")),
    }
}

struct Parser {
    chars: Vec<char>,
    position: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.position).copied()
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.position += 1;
        }
    }

    fn expect(&mut self, expected: char) -> Result<(), String> {
        self.skip_whitespace();
        if self.peek() == Some(expected) {
            self.position += 1;
            Ok(())
        } else {
            Err(format!("expected `{expected}` at {}", self.position))
        }
    }

    fn sum(&mut self) -> Result<Value, String> {
        let mut terms = vec![self.product()?];
        loop {
            self.skip_whitespace();
            let negate = match self.peek() {
                Some('+') => false,
                Some('-') => true,
                _ => break,
            };
            self.position += 1;
            let term = self.product()?;
            terms.push(if negate { scale(term, -1.0) } else { term });
        }
        if terms.len() == 1 {
            return Ok(terms.pop().unwrap());
        }
        if terms.iter().all(|term| matches!(term, Value::Number(_))) {
            return Ok(Value::Number(terms.iter().map(number).sum()));
        }
        let terms = terms.into_iter().map(length).collect::<Result<Vec<_>, _>>()?;
        Ok(Value::Length(Calc::Sum(terms)))
    }

    fn product(&mut self) -> Result<Value, String> {
        let mut value = self.function_or_operand()?;
        loop {
            self.skip_whitespace();
            let divide = match self.peek() {
                Some('*') => false,
                Some('/') => true,
                _ => break,
            };
            self.position += 1;
            let other = self.function_or_operand()?;
            value = match (value, other, divide) {
                (Value::Number(a), Value::Number(b), false) => Value::Number(a * b),
                (Value::Number(a), Value::Number(b), true) => Value::Number(a / b),
                (Value::Length(calc), Value::Number(factor), false)
                | (Value::Number(factor), Value::Length(calc), false) => {
                    Value::Length(Calc::Scale(Box::new(calc), factor))
                }
                (Value::Length(calc), Value::Number(divisor), true) => {
                    Value::Length(Calc::Scale(Box::new(calc), 1.0 / divisor))
                }
                _ => return Err("lengths can only be multiplied or divided by numbers".to_owned()),
            };
        }
        Ok(value)
    }

    fn function_or_operand(&mut self) -> Result<Value, String> {
        self.skip_whitespace();
        let name: String = self.chars[self.position..]
            .iter()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect();
        if !name.is_empty() && self.chars.get(self.position + name.len()) == Some(&'(') {
            self.position += name.len();
            self.expect('(')?;
            let arguments = self.arguments()?;
            return match name.as_str() {
                "calc" if arguments.len() == 1 => Ok(arguments.into_iter().next().unwrap()),
                "min" | "max" => {
                    let values = arguments.into_iter().map(length).collect::<Result<Vec<_>, _>>()?;
                    Ok(Value::Length(if name == "min" {
                        Calc::Min(values)
                    } else {
                        Calc::Max(values)
                    }))
                }
                "clamp" if arguments.len() == 3 => {
                    let mut values = arguments.into_iter().map(length);
                    Ok(Value::Length(Calc::Clamp(
                        Box::new(values.next().unwrap()?),
                        Box::new(values.next().unwrap()?),
                        Box::new(values.next().unwrap()?),
                    )))
                }
                _ => Err(format!("unsupported function `{name}`")),
            };
        }
        if self.peek() == Some('(') {
            self.position += 1;
            let value = self.sum()?;
            self.expect(')')?;
            return Ok(value);
        }
        self.number()
    }

    fn arguments(&mut self) -> Result<Vec<Value>, String> {
        let mut arguments = vec![self.sum()?];
        loop {
            self.skip_whitespace();
            match self.peek() {
                Some(',') => {
                    self.position += 1;
                    arguments.push(self.sum()?);
                }
                Some(')') => {
                    self.position += 1;
                    return Ok(arguments);
                }
                _ => return Err(format!("expected `,` or `)` at {}", self.position)),
            }
        }
    }

    fn number(&mut self) -> Result<Value, String> {
        self.skip_whitespace();
        let start = self.position;
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E'))
        {
            // `e` only continues a number in an exponent, not in a unit like `em`.
            if matches!(self.peek(), Some('e' | 'E'))
                && !self.chars.get(self.position + 1).is_some_and(|c| c.is_ascii_digit() || *c == '-' || *c == '+')
            {
                break;
            }
            self.position += 1;
        }
        let text: String = self.chars[start..self.position].iter().collect();
        let value: f64 = text
            .parse()
            .map_err(|_| format!("expected a number at {start}"))?;
        if self.peek() == Some('%') {
            self.position += 1;
            return Ok(Value::Length(Calc::Percent(value)));
        }
        if self.chars[self.position..].starts_with(&['p', 'x']) {
            self.position += 2;
            return Ok(Value::Length(Calc::Px(value)));
        }
        Ok(Value::Number(value))
    }
}

fn scale(value: Value, factor: f64) -> Value {
    match value {
        Value::Number(number) => Value::Number(number * factor),
        Value::Length(calc) => Value::Length(Calc::Scale(Box::new(calc), factor)),
    }
}

fn number(value: &Value) -> f64 {
    match value {
        Value::Number(number) => *number,
        Value::Length(_) => 0.0,
    }
}

fn length(value: Value) -> Result<Calc, String> {
    match value {
        Value::Length(calc) => Ok(calc),
        // A bare 0 is a length.
        Value::Number(0.0) => Ok(Calc::Px(0.0)),
        Value::Number(_) => Err("expected a length".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_chrome_computed_values() {
        let resolve = |text: &str, basis: f64| parse_calc(text).unwrap().resolve(Some(basis)).unwrap();
        assert_eq!(resolve("calc(50% - 10px)", 200.0), 90.0);
        assert_eq!(resolve("calc(-10px + 50%)", 200.0), 90.0);
        assert_eq!(resolve("min(100%, 600px)", 400.0), 400.0);
        assert_eq!(resolve("max(10%, 30px)", 400.0), 40.0);
        assert_eq!(resolve("clamp(10px, 50%, 100px)", 400.0), 100.0);
        assert_eq!(resolve("calc(2 * 25% + 5px / 2)", 100.0), 52.5);
        assert_eq!(resolve("calc(min(50%, 20px) + 1px)", 100.0), 21.0);
        assert_eq!(resolve("calc(1.5e1px + 10%)", 100.0), 25.0);
    }
}
