//! Selectors Level 3, plus `:not()`, `:is()`, `:where()` and `:focus-within` from Level 4.
//! <https://www.w3.org/TR/selectors-3/>
//!
//! Pseudo-elements (`::before`) and namespaces are not supported.

use cssparser::{ParseError, Parser, Token};
use std::fmt;

/// A comma-separated list of complex selectors (`a, b > c`).
#[derive(Clone, Debug, PartialEq)]
pub struct SelectorList(pub Vec<ComplexSelector>);

/// Compound selectors joined by combinators, e.g. `.list > li:hover`.
#[derive(Clone, Debug, PartialEq)]
pub struct ComplexSelector {
    /// Left to right; the combinator of the first compound is unused.
    pub compounds: Vec<(Combinator, CompoundSelector)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Combinator {
    /// `a b`
    Descendant,
    /// `a > b`
    Child,
    /// `a + b`
    NextSibling,
    /// `a ~ b`
    SubsequentSibling,
}

/// Simple selectors that all apply to one element, e.g. `div.card#main:hover`.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct CompoundSelector(pub Vec<SimpleSelector>);

#[derive(Clone, Debug, PartialEq)]
pub enum SimpleSelector {
    /// `*`
    Universal,
    /// `div`, matched case-insensitively.
    Type(String),
    /// `#main`
    Id(String),
    /// `.card`
    Class(String),
    /// `[attr]`, `[attr=value]`, …
    Attribute(AttributeSelector),
    PseudoClass(PseudoClass),
}

#[derive(Clone, Debug, PartialEq)]
pub struct AttributeSelector {
    pub name: String,
    pub matcher: Option<(AttributeOperator, String)>,
    /// `[attr=value i]`
    pub case_insensitive: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttributeOperator {
    /// `=`
    Equals,
    /// `~=`: one of the whitespace-separated words.
    Includes,
    /// `|=`: equal, or starts with `value-`.
    DashMatch,
    /// `^=`
    Prefix,
    /// `$=`
    Suffix,
    /// `*=`
    Substring,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PseudoClass {
    Hover,
    Active,
    Focus,
    FocusWithin,
    Root,
    Empty,
    FirstChild,
    LastChild,
    OnlyChild,
    FirstOfType,
    LastOfType,
    OnlyOfType,
    /// `:nth-child(An+B [of S]?)`
    NthChild(Nth, Option<SelectorList>),
    NthLastChild(Nth, Option<SelectorList>),
    NthOfType(Nth),
    NthLastOfType(Nth),
    Not(SelectorList),
    Is(SelectorList),
    Where(SelectorList),
}

/// `An+B`: matches the 1-based positions `A*n + B` for some `n >= 0`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Nth {
    pub a: i32,
    pub b: i32,
}

impl Nth {
    pub fn matches(self, position: i32) -> bool {
        if self.a == 0 {
            return position == self.b;
        }
        let n = position - self.b;
        n % self.a == 0 && n / self.a >= 0
    }
}

/// Specificity as (ids, classes/attributes/pseudo-classes, types).
pub type Specificity = (u32, u32, u32);

fn add(a: Specificity, b: Specificity) -> Specificity {
    (a.0 + b.0, a.1 + b.1, a.2 + b.2)
}

impl SelectorList {
    /// The largest specificity in the list (used by `:is()` and `:not()`).
    pub fn max_specificity(&self) -> Specificity {
        self.0
            .iter()
            .map(ComplexSelector::specificity)
            .max()
            .unwrap_or((0, 0, 0))
    }
}

impl SelectorList {
    /// Whether matching depends on `:hover`, `:active`, `:focus` or `:focus-within`.
    pub fn depends_on_state(&self) -> bool {
        self.0.iter().any(|selector| {
            selector.compounds.iter().any(|(_, compound)| {
                compound.0.iter().any(|simple| match simple {
                    SimpleSelector::PseudoClass(pseudo) => match pseudo {
                        PseudoClass::Hover
                        | PseudoClass::Active
                        | PseudoClass::Focus
                        | PseudoClass::FocusWithin => true,
                        PseudoClass::Not(list)
                        | PseudoClass::Is(list)
                        | PseudoClass::Where(list) => list.depends_on_state(),
                        PseudoClass::NthChild(_, Some(list))
                        | PseudoClass::NthLastChild(_, Some(list)) => list.depends_on_state(),
                        _ => false,
                    },
                    _ => false,
                })
            })
        })
    }
}

impl ComplexSelector {
    pub fn specificity(&self) -> Specificity {
        self.compounds.iter().fold((0, 0, 0), |sum, (_, compound)| {
            add(sum, compound.specificity())
        })
    }
}

impl CompoundSelector {
    pub fn specificity(&self) -> Specificity {
        self.0.iter().fold((0, 0, 0), |sum, simple| {
            add(
                sum,
                match simple {
                    SimpleSelector::Universal => (0, 0, 0),
                    SimpleSelector::Type(_) => (0, 0, 1),
                    SimpleSelector::Id(_) => (1, 0, 0),
                    SimpleSelector::Class(_) | SimpleSelector::Attribute(_) => (0, 1, 0),
                    SimpleSelector::PseudoClass(pseudo) => match pseudo {
                        PseudoClass::Where(_) => (0, 0, 0),
                        PseudoClass::Is(list) | PseudoClass::Not(list) => list.max_specificity(),
                        PseudoClass::NthChild(_, Some(list))
                        | PseudoClass::NthLastChild(_, Some(list)) => {
                            add((0, 1, 0), list.max_specificity())
                        }
                        _ => (0, 1, 0),
                    },
                },
            )
        })
    }
}

/// Parses a selector list, e.g. the prelude of a style rule.
pub fn parse_selector_list<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<SelectorList, ParseError<'i, ()>> {
    let selectors = input.parse_comma_separated(|input| parse_complex_selector(input))?;
    Ok(SelectorList(selectors))
}

fn parse_complex_selector<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<ComplexSelector, ParseError<'i, ()>> {
    input.skip_whitespace();
    let mut compounds = vec![(Combinator::Descendant, parse_compound_selector(input)?)];
    loop {
        // Whitespace is a descendant combinator unless an explicit combinator follows.
        let mut combinator = None;
        let mut explicit = false;
        loop {
            let state = input.state();
            let next = match input.next_including_whitespace() {
                Ok(Token::WhiteSpace(_)) => None,
                Ok(Token::Delim('>')) => Some(Combinator::Child),
                Ok(Token::Delim('+')) => Some(Combinator::NextSibling),
                Ok(Token::Delim('~')) => Some(Combinator::SubsequentSibling),
                _ => {
                    input.reset(&state);
                    break;
                }
            };
            match next {
                Some(_) if explicit => return Err(input.new_custom_error(())),
                Some(next) => {
                    combinator = Some(next);
                    explicit = true;
                }
                None => {
                    combinator.get_or_insert(Combinator::Descendant);
                }
            }
        }
        let Some(combinator) = combinator else {
            break;
        };
        if input.is_exhausted() {
            if explicit {
                // A combinator needs a selector after it.
                return Err(input.new_custom_error(()));
            }
            // Trailing whitespace.
            break;
        }
        compounds.push((combinator, parse_compound_selector(input)?));
    }
    Ok(ComplexSelector { compounds })
}

fn parse_compound_selector<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<CompoundSelector, ParseError<'i, ()>> {
    let mut simple = Vec::new();
    loop {
        let state = input.state();
        let token = match input.next_including_whitespace() {
            Ok(token) => token.clone(),
            Err(_) => break,
        };
        match token {
            Token::Ident(name) if simple.is_empty() => {
                simple.push(SimpleSelector::Type(name.to_ascii_lowercase()));
            }
            Token::Delim('*') if simple.is_empty() => simple.push(SimpleSelector::Universal),
            Token::IDHash(id) => simple.push(SimpleSelector::Id(id.to_string())),
            Token::Delim('.') => {
                let class = match input.next_including_whitespace()? {
                    Token::Ident(class) => class.to_string(),
                    _ => return Err(input.new_custom_error(())),
                };
                simple.push(SimpleSelector::Class(class));
            }
            Token::SquareBracketBlock => {
                let attribute = input.parse_nested_block(|input| parse_attribute(input))?;
                simple.push(SimpleSelector::Attribute(attribute));
            }
            Token::Colon => simple.push(SimpleSelector::PseudoClass(parse_pseudo_class(input)?)),
            _ => {
                input.reset(&state);
                break;
            }
        }
    }
    if simple.is_empty() {
        return Err(input.new_custom_error(()));
    }
    Ok(CompoundSelector(simple))
}

fn parse_attribute<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<AttributeSelector, ParseError<'i, ()>> {
    let name = input.expect_ident()?.to_ascii_lowercase();
    if input.is_exhausted() {
        return Ok(AttributeSelector {
            name,
            matcher: None,
            case_insensitive: false,
        });
    }

    let operator = match input.next()?.clone() {
        Token::Delim('=') => AttributeOperator::Equals,
        Token::IncludeMatch => AttributeOperator::Includes,
        Token::DashMatch => AttributeOperator::DashMatch,
        Token::PrefixMatch => AttributeOperator::Prefix,
        Token::SuffixMatch => AttributeOperator::Suffix,
        Token::SubstringMatch => AttributeOperator::Substring,
        _ => return Err(input.new_custom_error(())),
    };
    let value = match input.next()?.clone() {
        Token::Ident(value) | Token::QuotedString(value) => value.to_string(),
        _ => return Err(input.new_custom_error(())),
    };
    let case_insensitive = match input.try_parse(|input| input.expect_ident_cloned()) {
        Ok(flag) if flag.eq_ignore_ascii_case("i") => true,
        Ok(flag) if flag.eq_ignore_ascii_case("s") => false,
        Ok(_) => return Err(input.new_custom_error(())),
        Err(_) => false,
    };
    input.expect_exhausted()?;

    Ok(AttributeSelector {
        name,
        matcher: Some((operator, value)),
        case_insensitive,
    })
}

fn parse_pseudo_class<'i>(input: &mut Parser<'i, '_>) -> Result<PseudoClass, ParseError<'i, ()>> {
    match input.next_including_whitespace()?.clone() {
        Token::Ident(name) => Ok(match name.to_ascii_lowercase().as_str() {
            "hover" => PseudoClass::Hover,
            "active" => PseudoClass::Active,
            "focus" => PseudoClass::Focus,
            "focus-within" => PseudoClass::FocusWithin,
            "root" => PseudoClass::Root,
            "empty" => PseudoClass::Empty,
            "first-child" => PseudoClass::FirstChild,
            "last-child" => PseudoClass::LastChild,
            "only-child" => PseudoClass::OnlyChild,
            "first-of-type" => PseudoClass::FirstOfType,
            "last-of-type" => PseudoClass::LastOfType,
            "only-of-type" => PseudoClass::OnlyOfType,
            _ => return Err(input.new_custom_error(())),
        }),
        Token::Function(name) => {
            let name = name.to_ascii_lowercase();
            input.parse_nested_block(|input| {
                Ok(match name.as_str() {
                    "nth-child" => {
                        let (nth, of) = parse_nth_with_selector(input)?;
                        PseudoClass::NthChild(nth, of)
                    }
                    "nth-last-child" => {
                        let (nth, of) = parse_nth_with_selector(input)?;
                        PseudoClass::NthLastChild(nth, of)
                    }
                    "nth-of-type" => PseudoClass::NthOfType(parse_nth(input)?),
                    "nth-last-of-type" => PseudoClass::NthLastOfType(parse_nth(input)?),
                    "not" => PseudoClass::Not(parse_selector_list(input)?),
                    "is" | "matches" => PseudoClass::Is(parse_selector_list(input)?),
                    "where" => PseudoClass::Where(parse_selector_list(input)?),
                    _ => return Err(input.new_custom_error(())),
                })
            })
        }
        _ => Err(input.new_custom_error(())),
    }
}

fn parse_nth<'i>(input: &mut Parser<'i, '_>) -> Result<Nth, ParseError<'i, ()>> {
    let (a, b) = cssparser::parse_nth(input)?;
    Ok(Nth { a, b })
}

/// `An+B` optionally followed by `of <selector-list>`.
fn parse_nth_with_selector<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<(Nth, Option<SelectorList>), ParseError<'i, ()>> {
    let nth = parse_nth(input)?;
    if input.is_exhausted() {
        return Ok((nth, None));
    }
    input.expect_ident_matching("of")?;
    Ok((nth, Some(parse_selector_list(input)?)))
}

impl fmt::Display for SelectorList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, selector) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{selector}")?;
        }
        Ok(())
    }
}

impl fmt::Display for ComplexSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, (combinator, compound)) in self.compounds.iter().enumerate() {
            if index > 0 {
                f.write_str(match combinator {
                    Combinator::Descendant => " ",
                    Combinator::Child => " > ",
                    Combinator::NextSibling => " + ",
                    Combinator::SubsequentSibling => " ~ ",
                })?;
            }
            write!(f, "{compound}")?;
        }
        Ok(())
    }
}

impl fmt::Display for CompoundSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for simple in &self.0 {
            match simple {
                SimpleSelector::Universal => f.write_str("*")?,
                SimpleSelector::Type(name) => f.write_str(name)?,
                SimpleSelector::Id(id) => write!(f, "#{id}")?,
                SimpleSelector::Class(class) => write!(f, ".{class}")?,
                SimpleSelector::Attribute(attribute) => {
                    write!(f, "[{}", attribute.name)?;
                    if let Some((operator, value)) = &attribute.matcher {
                        let operator = match operator {
                            AttributeOperator::Equals => "=",
                            AttributeOperator::Includes => "~=",
                            AttributeOperator::DashMatch => "|=",
                            AttributeOperator::Prefix => "^=",
                            AttributeOperator::Suffix => "$=",
                            AttributeOperator::Substring => "*=",
                        };
                        write!(f, "{operator}\"{value}\"")?;
                        if attribute.case_insensitive {
                            f.write_str(" i")?;
                        }
                    }
                    f.write_str("]")?;
                }
                SimpleSelector::PseudoClass(pseudo) => write!(f, ":{pseudo}")?,
            }
        }
        Ok(())
    }
}

impl fmt::Display for PseudoClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let nth = |f: &mut fmt::Formatter<'_>, name: &str, nth: &Nth, of: Option<&SelectorList>| {
            write!(f, "{name}({}n+{}", nth.a, nth.b)?;
            if let Some(of) = of {
                write!(f, " of {of}")?;
            }
            f.write_str(")")
        };
        match self {
            PseudoClass::Hover => f.write_str("hover"),
            PseudoClass::Active => f.write_str("active"),
            PseudoClass::Focus => f.write_str("focus"),
            PseudoClass::FocusWithin => f.write_str("focus-within"),
            PseudoClass::Root => f.write_str("root"),
            PseudoClass::Empty => f.write_str("empty"),
            PseudoClass::FirstChild => f.write_str("first-child"),
            PseudoClass::LastChild => f.write_str("last-child"),
            PseudoClass::OnlyChild => f.write_str("only-child"),
            PseudoClass::FirstOfType => f.write_str("first-of-type"),
            PseudoClass::LastOfType => f.write_str("last-of-type"),
            PseudoClass::OnlyOfType => f.write_str("only-of-type"),
            PseudoClass::NthChild(n, of) => nth(f, "nth-child", n, of.as_ref()),
            PseudoClass::NthLastChild(n, of) => nth(f, "nth-last-child", n, of.as_ref()),
            PseudoClass::NthOfType(n) => nth(f, "nth-of-type", n, None),
            PseudoClass::NthLastOfType(n) => nth(f, "nth-last-of-type", n, None),
            PseudoClass::Not(list) => write!(f, "not({list})"),
            PseudoClass::Is(list) => write!(f, "is({list})"),
            PseudoClass::Where(list) => write!(f, "where({list})"),
        }
    }
}
