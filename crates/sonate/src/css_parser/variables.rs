//! Custom properties and `var()` (CSS Custom Properties for Cascading Variables Level 1).
//! <https://www.w3.org/TR/css-variables-1/>
//!
//! Custom property values are token text. A declaration that uses `var()` keeps its text until
//! computed-value time, when the references are substituted and the result is parsed.

use cssparser::{ParseError, Parser, ParserInput, Token};
use std::collections::{BTreeMap, HashSet};

/// Consumes the rest of `input` and returns its text without surrounding whitespace.
pub(crate) fn raw_value(input: &mut Parser) -> String {
    let start = input.position();
    while input.next_including_whitespace_and_comments().is_ok() {}
    input.slice_from(start).trim().to_owned()
}

/// Whether the tokens contain a `var()` function, at any depth. Consumes the input.
pub(crate) fn contains_var(input: &mut Parser) -> bool {
    while let Ok(token) = input.next_including_whitespace_and_comments() {
        match token.clone() {
            Token::Function(name) if name.eq_ignore_ascii_case("var") => return true,
            Token::Function(_)
            | Token::ParenthesisBlock
            | Token::SquareBracketBlock
            | Token::CurlyBracketBlock => {
                // A nested block must be consumed completely, or parsing it fails.
                let nested = input.parse_nested_block(|input| {
                    let found = contains_var(input);
                    while input.next_including_whitespace_and_comments().is_ok() {}
                    Ok::<_, ParseError<()>>(found)
                });
                if nested.unwrap_or(false) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// Replaces every `var(--name[, fallback])` in `text`. Returns `None` when a referenced
/// property is missing and has no fallback (the value is invalid at computed-value time).
pub fn substitute_var(text: &str, lookup: &mut dyn FnMut(&str) -> Option<String>) -> Option<String> {
    let mut input = ParserInput::new(text);
    let mut parser = Parser::new(&mut input);
    let mut out = String::new();
    substitute_tokens(&mut parser, lookup, &mut out).ok()?;
    Some(out)
}

fn substitute_tokens(
    input: &mut Parser,
    lookup: &mut dyn FnMut(&str) -> Option<String>,
    out: &mut String,
) -> Result<(), ()> {
    loop {
        let start = input.position();
        let token = match input.next_including_whitespace_and_comments() {
            Ok(token) => token.clone(),
            Err(_) => return Ok(()),
        };
        match token {
            Token::Function(name) if name.eq_ignore_ascii_case("var") => {
                let (name, fallback) = input
                    .parse_nested_block(|input| {
                        input.skip_whitespace();
                        let name = input.expect_ident_cloned()?;
                        if !name.starts_with("--") {
                            return Err(input.new_custom_error::<_, ()>(()));
                        }
                        input.skip_whitespace();
                        let fallback = if input.try_parse(|input| input.expect_comma()).is_ok() {
                            Some(raw_value(input))
                        } else {
                            input.expect_exhausted()?;
                            None
                        };
                        Ok((name.to_string(), fallback))
                    })
                    .map_err(|_| ())?;
                let value = match lookup(&name) {
                    Some(value) => value,
                    None => substitute_var(&fallback.ok_or(())?, lookup).ok_or(())?,
                };
                out.push_str(&value);
            }
            Token::Function(_)
            | Token::ParenthesisBlock
            | Token::SquareBracketBlock
            | Token::CurlyBracketBlock => {
                let closing = match token {
                    Token::SquareBracketBlock => ']',
                    Token::CurlyBracketBlock => '}',
                    _ => ')',
                };
                // The text of the opening token, e.g. `calc(`.
                out.push_str(input.slice_from(start));
                input
                    .parse_nested_block(|input| {
                        substitute_tokens(input, lookup, out).map_err(|_| input.new_custom_error::<_, ()>(()))
                    })
                    .map_err(|_| ())?;
                out.push(closing);
            }
            _ => out.push_str(input.slice_from(start)),
        }
    }
}

/// Substitutes the references between custom properties. Properties in a reference cycle, or
/// that reference a missing property without a fallback, are invalid and dropped.
pub fn resolve_custom_properties(raw: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut resolver = Resolver {
        raw,
        resolved: BTreeMap::new(),
        invalid: HashSet::new(),
        resolving: Vec::new(),
    };
    for name in raw.keys() {
        resolver.resolve(name);
    }
    resolver.resolved
}

struct Resolver<'a> {
    raw: &'a BTreeMap<String, String>,
    resolved: BTreeMap<String, String>,
    invalid: HashSet<String>,
    resolving: Vec<String>,
}

impl Resolver<'_> {
    fn resolve(&mut self, name: &str) -> Option<String> {
        if let Some(value) = self.resolved.get(name) {
            return Some(value.clone());
        }
        if self.invalid.contains(name) {
            return None;
        }
        if let Some(position) = self.resolving.iter().position(|resolving| resolving == name) {
            // Every property in the cycle is invalid.
            for name in &self.resolving[position..] {
                self.invalid.insert(name.clone());
            }
            return None;
        }
        let raw = self.raw.get(name)?;

        self.resolving.push(name.to_owned());
        let value = substitute_var(raw, &mut |reference| self.resolve(reference));
        self.resolving.pop();

        match value {
            Some(value) if !self.invalid.contains(name) => {
                self.resolved.insert(name.to_owned(), value.clone());
                Some(value)
            }
            _ => {
                self.invalid.insert(name.to_owned());
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
        entries
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect()
    }

    #[test]
    fn substitutes_with_fallbacks() {
        let vars = map(&[("--a", "10px"), ("--color", "rgb(1, 2, 3)")]);
        let mut lookup = |name: &str| vars.get(name).cloned();
        assert_eq!(substitute_var("calc(var(--a) * 2)", &mut lookup).as_deref(), Some("calc(10px * 2)"));
        assert_eq!(substitute_var("var(--a) var(--b, 5px)", &mut lookup).as_deref(), Some("10px 5px"));
        assert_eq!(substitute_var("var(--b, var(--color))", &mut lookup).as_deref(), Some("rgb(1, 2, 3)"));
        assert_eq!(substitute_var("var(--b)", &mut lookup), None);
    }

    #[test]
    fn finds_var_in_nested_functions() {
        let declarations = crate::css_parser::parse_inline_style("width: calc(var(--gap) * 3)");
        assert_eq!(
            declarations[0].unparsed.as_ref().map(|(_, text)| text.as_str()),
            Some("calc(var(--gap) * 3)")
        );
    }

    #[test]
    fn resolves_references_and_drops_cycles() {
        let resolved = resolve_custom_properties(&map(&[
            ("--a", "var(--b)"),
            ("--b", "4px"),
            ("--x", "var(--y)"),
            ("--y", "var(--x)"),
            ("--z", "var(--x, 1px)"),
        ]));
        assert_eq!(resolved.get("--a").map(String::as_str), Some("4px"));
        assert!(!resolved.contains_key("--x") && !resolved.contains_key("--y"));
        assert_eq!(resolved.get("--z").map(String::as_str), Some("1px"));
    }
}

