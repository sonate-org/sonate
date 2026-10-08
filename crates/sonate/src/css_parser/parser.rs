use super::borders::Side;
use super::selectors::{parse_selector_list, SelectorList};
use super::variables::{contains_var, raw_value};
use crate::style::{
    AlignContent, AlignItems, AlignSelf, BoxSizing, CssWideKeyword, Declaration, Directional,
    Display, FlexDirection, FlexWrap, FontFace, JustifyContent, Length, Overflow, OverflowWrap,
    Position,
    Rule, Style, StyleSheet, TextAlign, TextOverflow, TextTransform, WhiteSpace, Widget, WordBreak,
    ZIndex,
};
use cssparser::{
    AtRuleParser, CowRcStr, DeclarationParser, Delimiter, ParseError, Parser, ParserInput, ParserState,
    QualifiedRuleParser, RuleBodyItemParser, RuleBodyParser, StyleSheetParser,
};

/// Parse a CSS string into a StyleSheet
pub fn parse_css(css: &str) -> Result<StyleSheet, String> {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);

    let mut stylesheet = StyleSheet::new();
    let mut css_parser = CssParser::new();

    let rules = StyleSheetParser::new(&mut parser, &mut css_parser);

    for rule in rules {
        match rule {
            Ok(CssRule::Style(rule)) => stylesheet.add_rule(rule),
            Ok(CssRule::FontFace(face)) => stylesheet.font_faces.push(face),
            Err(err) => {
                eprintln!("CSS parsing error: {:?}", err);
            }
        }
    }

    Ok(stylesheet)
}

/// Parse the declarations of an inline `style` attribute, in source order.
pub fn parse_inline_style(css: &str) -> Vec<Declaration> {
    parse_declaration_list(css, true)
}

/// Like [`parse_inline_style`], without reporting errors.
pub fn parse_declarations(css: &str) -> Vec<Declaration> {
    parse_declaration_list(css, false)
}

fn parse_declaration_list(css: &str, report_errors: bool) -> Vec<Declaration> {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    let mut declaration_parser = StyleDeclarationParser::new();

    RuleBodyParser::new(&mut parser, &mut declaration_parser)
        .filter_map(|item| match item {
            Ok(declaration) => Some(declaration),
            Err(err) => {
                if report_errors {
                    eprintln!("Inline style parsing error: {:?}", err);
                }
                None
            }
        })
        .collect()
}

/// A top-level rule.
pub enum CssRule {
    Style(Rule),
    FontFace(FontFace),
}

/// CSS Parser implementation
pub struct CssParser {
    // We can add state here if needed
}

impl CssParser {
    pub fn new() -> Self {
        Self {}
    }
}

impl<'i> QualifiedRuleParser<'i> for CssParser {
    type Prelude = SelectorList;
    type QualifiedRule = CssRule;
    type Error = ();

    fn parse_prelude<'t>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
        // An invalid selector drops the whole rule.
        parse_selector_list(input)
    }

    fn parse_block<'t>(
        &mut self,
        prelude: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::QualifiedRule, ParseError<'i, Self::Error>> {
        let mut declarations = Vec::new();
        let mut declaration_parser = StyleDeclarationParser::new();

        let parser = RuleBodyParser::new(input, &mut declaration_parser);
        for item in parser {
            match item {
                Ok(declaration) => declarations.push(declaration),
                Err(err) => {
                    eprintln!("Declaration parsing error: {:?}", err);
                }
            }
        }

        Ok(CssRule::Style(Rule {
            selectors: prelude,
            declarations,
        }))
    }
}

impl<'i> AtRuleParser<'i> for CssParser {
    type Prelude = ();
    type AtRule = CssRule;
    type Error = ();

    fn parse_prelude<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
        // Only `@font-face` is supported.
        if name.eq_ignore_ascii_case("font-face") {
            Ok(())
        } else {
            Err(input.new_custom_error(()))
        }
    }

    fn parse_block<'t>(
        &mut self,
        _prelude: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::AtRule, ParseError<'i, Self::Error>> {
        let mut parser = FontFaceParser {
            face: FontFace {
                weight: 400,
                ..FontFace::default()
            },
            values: StyleDeclarationParser::new(),
        };
        for item in RuleBodyParser::new(input, &mut parser) {
            if let Err(err) = item {
                eprintln!("@font-face parsing error: {:?}", err);
            }
        }
        Ok(CssRule::FontFace(parser.face))
    }
}

/// The descriptors of a `@font-face` rule.
struct FontFaceParser {
    face: FontFace,
    values: StyleDeclarationParser,
}

impl<'i> DeclarationParser<'i> for FontFaceParser {
    type Declaration = ();
    type Error = ();

    fn parse_value<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
        _declaration_start: &ParserState,
    ) -> Result<(), ParseError<'i, ()>> {
        self.values
            .parse_font_face_descriptor(&name, input, &mut self.face)
    }
}

impl<'i> AtRuleParser<'i> for FontFaceParser {
    type Prelude = ();
    type AtRule = ();
    type Error = ();
}

impl<'i> QualifiedRuleParser<'i> for FontFaceParser {
    type Prelude = ();
    type QualifiedRule = ();
    type Error = ();
}

impl<'i> RuleBodyItemParser<'i, (), ()> for FontFaceParser {
    fn parse_qualified(&self) -> bool {
        false
    }

    fn parse_declarations(&self) -> bool {
        true
    }
}

/// Declaration parser for style properties
pub struct StyleDeclarationParser {
    // State can be added here if needed
}

impl StyleDeclarationParser {
    pub fn new() -> Self {
        Self {}
    }
}

impl<'i> DeclarationParser<'i> for StyleDeclarationParser {
    type Declaration = Declaration;
    type Error = ();

    fn parse_value<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
        _declaration_start: &ParserState,
    ) -> Result<Self::Declaration, ParseError<'i, Self::Error>> {
        let is_custom = name.starts_with("--");
        // Parse the value up to `!important`, so value parsers can require the whole input.
        let mut declaration = input.parse_until_before(Delimiter::Bang, |input| {
            let mut declaration = Declaration {
                style: Style::default(),
                important: false,
                keyword: None,
                custom: None,
                unparsed: None,
            };
            let keyword = input.try_parse(|input| {
                let ident = input.expect_ident_cloned()?;
                input.expect_exhausted()?;
                match ident.to_ascii_lowercase().as_str() {
                    "inherit" => Ok(CssWideKeyword::Inherit),
                    "initial" => Ok(CssWideKeyword::Initial),
                    // `revert` has no user-agent styles to go back to.
                    "unset" | "revert" => Ok(CssWideKeyword::Unset),
                    _ => Err(input.new_custom_error::<(), ()>(())),
                }
            });
            if !is_custom && !crate::cascade::is_known_property(&name) {
                return Err(input.new_custom_error(()));
            }

            if let Ok(keyword) = keyword {
                declaration.keyword = Some((name.to_string(), keyword));
            } else if is_custom {
                declaration.custom = Some((name.to_string(), raw_value(input)));
            } else {
                let start = input.state();
                let uses_var = contains_var(input);
                input.reset(&start);
                if uses_var {
                    declaration.unparsed = Some((name.to_string(), raw_value(input)));
                } else {
                    declaration.style = self.parse_property(name.clone(), input)?;
                }
            }
            Ok(declaration)
        })?;
        declaration.important = input.try_parse(cssparser::parse_important).is_ok();
        Ok(declaration)
    }
}

impl StyleDeclarationParser {
    /// Parses the value of `name` into a `Style` where only that property is set.
    fn parse_property<'i, 't>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
    ) -> Result<Style, ParseError<'i, ()>> {
        let mut style = Style::default();

        match name.as_ref() {
            "-sonate-widget" => {
                let ident = input.expect_ident()?;
                style.widget = Some(match ident.as_ref() {
                    "text-input" => Widget::TextInput,
                    "none" => Widget::None,
                    _ => return Err(input.new_error_for_next_token()),
                });
            }
            "display" => {
                let ident = input.expect_ident()?;
                match ident.as_ref() {
                    "flex" => style.display = Some(Display::Flex),
                    "none" => style.display = Some(Display::None),
                    _ => return Err(input.new_error_for_next_token()),
                }
            }
            "color" => {
                style.color = Some(self.parse_color_value(input)?);
            }
            "background" => {
                // only support color for now
                style.background_color = Some(self.parse_color_value(input)?);
            }
            "background-color" => {
                style.background_color = Some(self.parse_color_value(input)?);
            }
            "border-color" => {
                style.border_color = Directional::set_all(Some(self.parse_color_value(input)?));
            }
            "border-top-color" => {
                self.parse_border_side_color(input, &mut style.border_color.top)?;
            }
            "border-right-color" => {
                self.parse_border_side_color(input, &mut style.border_color.right)?;
            }
            "border-bottom-color" => {
                self.parse_border_side_color(input, &mut style.border_color.bottom)?;
            }
            "border-left-color" => {
                self.parse_border_side_color(input, &mut style.border_color.left)?;
            }
            "border-width" => {
                style.border_width = Directional::set_all(Some(self.parse_length_value(input)?));
            }
            "border-top-width" => {
                self.parse_border_side_width(input, &mut style.border_width.top)?;
            }
            "border-right-width" => {
                self.parse_border_side_width(input, &mut style.border_width.right)?;
            }
            "border-bottom-width" => {
                self.parse_border_side_width(input, &mut style.border_width.bottom)?;
            }
            "border-left-width" => {
                self.parse_border_side_width(input, &mut style.border_width.left)?;
            }
            "border-style" => {
                // Parse a single <line-style> value.
                let v = self
                    .try_parse_line_style(input)?
                    .ok_or_else(|| input.new_error_for_next_token())?;
                style.border_style = Directional::set_all(Some(v));
            }
            "border-top-style" => {
                self.parse_border_side_style(input, &mut style.border_style.top)?;
            }
            "border-right-style" => {
                self.parse_border_side_style(input, &mut style.border_style.right)?;
            }
            "border-bottom-style" => {
                self.parse_border_side_style(input, &mut style.border_style.bottom)?;
            }
            "border-left-style" => {
                self.parse_border_side_style(input, &mut style.border_style.left)?;
            }
            "border" => {
                self.parse_border_shorthand(input, &mut style)?;
            }
            "border-top" => {
                self.parse_border_side_shorthand(input, &mut style, Side::Top)?;
            }
            "border-right" => {
                self.parse_border_side_shorthand(input, &mut style, Side::Right)?;
            }
            "border-bottom" => {
                self.parse_border_side_shorthand(input, &mut style, Side::Bottom)?;
            }
            "border-left" => {
                self.parse_border_side_shorthand(input, &mut style, Side::Left)?;
            }
            "border-radius" => {
                self.parse_border_radius_shorthand(input, &mut style)?;
            }
            "border-top-left-radius" => {
                self.parse_border_corner_radius(input, &mut style.border_radius.top_left)?;
            }
            "border-top-right-radius" => {
                self.parse_border_corner_radius(input, &mut style.border_radius.top_right)?;
            }
            "border-bottom-right-radius" => {
                self.parse_border_corner_radius(input, &mut style.border_radius.bottom_right)?;
            }
            "border-bottom-left-radius" => {
                self.parse_border_corner_radius(input, &mut style.border_radius.bottom_left)?;
            }
            "box-sizing" => {
                let ident = input.expect_ident()?;
                style.box_sizing = Some(match ident.as_ref() {
                    "content-box" => BoxSizing::ContentBox,
                    "border-box" => BoxSizing::BorderBox,
                    _ => return Err(input.new_error_for_next_token()),
                });
            }
            "overflow" => {
                let ident = input.expect_ident()?;
                style.overflow = Some(match ident.as_ref() {
                    "visible" => Overflow::Visible,
                    "hidden" => Overflow::Hidden,
                    "scroll" => Overflow::Scroll,
                    "auto" => Overflow::Auto,
                    _ => return Err(input.new_error_for_next_token()),
                });
            }
            "width" => {
                style.width = Some(self.parse_length_value(input)?);
            }
            "height" => {
                style.height = Some(self.parse_length_value(input)?);
            }
            "margin" => {
                // NOTE: We currently implement the physical margin shorthands/sides:
                // - `margin` (shorthand)
                // - `margin-top/right/bottom/left`
                // We do NOT yet implement logical properties like `margin-inline`,
                // `margin-block`, `margin-inline-start/end`, etc.
                let first = self.parse_length_value(input)?;
                let second = input
                    .try_parse(|input| self.parse_length_value(input))
                    .unwrap_or(first.clone());
                let third = input
                    .try_parse(|input| self.parse_length_value(input))
                    .unwrap_or(first.clone());
                let fourth = input
                    .try_parse(|input| self.parse_length_value(input))
                    .unwrap_or(second.clone());

                // Emit per-side declarations so later `margin-left` etc. can override
                // a single side via Style::merge.
                style.margin = Directional {
                    top: Some(first),
                    right: Some(second),
                    bottom: Some(third),
                    left: Some(fourth),
                };
            }
            "margin-top" => {
                style.margin.top = Some(self.parse_length_value(input)?);
            }
            "margin-right" => {
                style.margin.right = Some(self.parse_length_value(input)?);
            }
            "margin-bottom" => {
                style.margin.bottom = Some(self.parse_length_value(input)?);
            }
            "margin-left" => {
                style.margin.left = Some(self.parse_length_value(input)?);
            }
            "padding" => {
                let first = self.parse_length_value(input)?;
                let second = input
                    .try_parse(|input| self.parse_length_value(input))
                    .unwrap_or(first.clone());
                let third = input
                    .try_parse(|input| self.parse_length_value(input))
                    .unwrap_or(first.clone());
                let fourth = input
                    .try_parse(|input| self.parse_length_value(input))
                    .unwrap_or(second.clone());

                // Emit per-side declarations so later `padding-left` etc. can override
                // a single side via Style::merge.
                style.padding = Directional {
                    top: Some(first),
                    right: Some(second),
                    bottom: Some(third),
                    left: Some(fourth),
                };
            }
            "padding-top" => {
                style.padding.top = Some(self.parse_length_value(input)?);
            }
            "padding-right" => {
                style.padding.right = Some(self.parse_length_value(input)?);
            }
            "padding-bottom" => {
                style.padding.bottom = Some(self.parse_length_value(input)?);
            }
            "padding-left" => {
                style.padding.left = Some(self.parse_length_value(input)?);
            }
            "flex" => {
                // `none | auto | [ <grow> <shrink>? || <basis> ]`
                if input
                    .try_parse(|input| input.expect_ident_matching("none"))
                    .is_ok()
                {
                    style.flex_grow = Some(0.0);
                    style.flex_shrink = Some(0.0);
                    style.flex_basis = Some(Length::Auto);
                } else if input
                    .try_parse(|input| input.expect_ident_matching("auto"))
                    .is_ok()
                {
                    style.flex_grow = Some(1.0);
                    style.flex_shrink = Some(1.0);
                    style.flex_basis = Some(Length::Auto);
                } else {
                    let mut grow = None;
                    let mut shrink = None;
                    let mut basis = None;
                    loop {
                        if grow.is_none() {
                            if let Ok(value) = input.try_parse(|input| input.expect_number()) {
                                grow = Some(value as f64);
                                shrink = input
                                    .try_parse(|input| input.expect_number())
                                    .ok()
                                    .map(f64::from);
                                continue;
                            }
                        }
                        if basis.is_none() {
                            if let Ok(value) =
                                input.try_parse(|input| self.parse_length_value(input))
                            {
                                basis = Some(value);
                                continue;
                            }
                        }
                        break;
                    }
                    if grow.is_none() && basis.is_none() {
                        return Err(input.new_error_for_next_token());
                    }
                    style.flex_grow = Some(grow.unwrap_or(1.0));
                    style.flex_shrink = Some(shrink.unwrap_or(1.0));
                    // A flex shorthand with only factors means a flex basis of 0%.
                    style.flex_basis = Some(basis.unwrap_or(Length::Percent(0.0)));
                }
            }
            "flex-flow" => {
                let mut parsed_any = false;
                while let Ok(ident) = input.try_parse(|input| input.expect_ident_cloned()) {
                    match ident.as_ref() {
                        "row" => style.flex_direction = Some(FlexDirection::Row),
                        "row-reverse" => style.flex_direction = Some(FlexDirection::RowReverse),
                        "column" => style.flex_direction = Some(FlexDirection::Column),
                        "column-reverse" => {
                            style.flex_direction = Some(FlexDirection::ColumnReverse)
                        }
                        "nowrap" => style.flex_wrap = Some(FlexWrap::NoWrap),
                        "wrap" => style.flex_wrap = Some(FlexWrap::Wrap),
                        "wrap-reverse" => style.flex_wrap = Some(FlexWrap::WrapReverse),
                        _ => return Err(input.new_error_for_next_token()),
                    }
                    parsed_any = true;
                }
                if !parsed_any {
                    return Err(input.new_error_for_next_token());
                }
            }
            "position" => {
                let ident = input.expect_ident()?;
                style.position = Some(match ident.as_ref() {
                    "static" | "relative" => Position::Relative,
                    "absolute" => Position::Absolute,
                    _ => return Err(input.new_error_for_next_token()),
                });
            }
            "inset" => {
                let first = self.parse_length_value(input)?;
                let second = input
                    .try_parse(|input| self.parse_length_value(input))
                    .unwrap_or_else(|_| first.clone());
                let third = input
                    .try_parse(|input| self.parse_length_value(input))
                    .unwrap_or_else(|_| first.clone());
                let fourth = input
                    .try_parse(|input| self.parse_length_value(input))
                    .unwrap_or_else(|_| second.clone());
                style.inset = Directional {
                    top: Some(first),
                    right: Some(second),
                    bottom: Some(third),
                    left: Some(fourth),
                };
            }
            "top" => {
                style.inset.top = Some(self.parse_length_value(input)?);
            }
            "right" => {
                style.inset.right = Some(self.parse_length_value(input)?);
            }
            "bottom" => {
                style.inset.bottom = Some(self.parse_length_value(input)?);
            }
            "left" => {
                style.inset.left = Some(self.parse_length_value(input)?);
            }
            "min-width" => {
                style.min_width = Some(self.parse_length_value(input)?);
            }
            "min-height" => {
                style.min_height = Some(self.parse_length_value(input)?);
            }
            "max-width" => {
                style.max_width = Some(self.parse_max_size(input)?);
            }
            "max-height" => {
                style.max_height = Some(self.parse_max_size(input)?);
            }
            "font-size" => {
                style.font_size = Some(self.parse_font_size(input)?);
            }
            "font-family" => {
                style.font_family = Some(self.parse_font_family(input)?);
            }
            "font-weight" => {
                style.font_weight = Some(self.parse_font_weight(input)?);
            }
            "font-style" => {
                style.font_style = Some(self.parse_font_style(input)?);
            }
            "font" => self.parse_font_shorthand(input, &mut style)?,
            "line-height" => {
                style.line_height = Some(self.parse_line_height(input)?);
            }
            "text-align" => {
                let ident = input.expect_ident()?;
                style.text_align = Some(match ident.to_ascii_lowercase().as_str() {
                    "start" => TextAlign::Start,
                    "end" => TextAlign::End,
                    "left" => TextAlign::Left,
                    "right" => TextAlign::Right,
                    "center" => TextAlign::Center,
                    "justify" => TextAlign::Justify,
                    _ => return Err(input.new_error_for_next_token()),
                });
            }
            "white-space" => {
                let ident = input.expect_ident()?;
                style.white_space = Some(match ident.to_ascii_lowercase().as_str() {
                    "normal" => WhiteSpace::Normal,
                    "nowrap" => WhiteSpace::NoWrap,
                    "pre" => WhiteSpace::Pre,
                    "pre-wrap" => WhiteSpace::PreWrap,
                    "pre-line" => WhiteSpace::PreLine,
                    "break-spaces" => WhiteSpace::BreakSpaces,
                    _ => return Err(input.new_error_for_next_token()),
                });
            }
            "text-transform" => {
                let ident = input.expect_ident()?;
                style.text_transform = Some(match ident.to_ascii_lowercase().as_str() {
                    "none" => TextTransform::None,
                    "uppercase" => TextTransform::Uppercase,
                    "lowercase" => TextTransform::Lowercase,
                    "capitalize" => TextTransform::Capitalize,
                    _ => return Err(input.new_error_for_next_token()),
                });
            }
            "overflow-wrap" | "word-wrap" => {
                let ident = input.expect_ident()?;
                style.overflow_wrap = Some(match ident.to_ascii_lowercase().as_str() {
                    "normal" => OverflowWrap::Normal,
                    "break-word" => OverflowWrap::BreakWord,
                    "anywhere" => OverflowWrap::Anywhere,
                    _ => return Err(input.new_error_for_next_token()),
                });
            }
            "word-break" => {
                let ident = input.expect_ident()?;
                style.word_break = Some(match ident.to_ascii_lowercase().as_str() {
                    "normal" => WordBreak::Normal,
                    "break-all" => WordBreak::BreakAll,
                    "keep-all" => WordBreak::KeepAll,
                    // Deprecated: like `overflow-wrap: anywhere`.
                    "break-word" => {
                        style.overflow_wrap = Some(OverflowWrap::Anywhere);
                        WordBreak::Normal
                    }
                    _ => return Err(input.new_error_for_next_token()),
                });
            }
            "letter-spacing" => {
                style.letter_spacing = Some(self.parse_spacing(input)?);
            }
            "word-spacing" => {
                style.word_spacing = Some(self.parse_spacing(input)?);
            }
            "text-overflow" => {
                let ident = input.expect_ident()?;
                style.text_overflow = Some(match ident.to_ascii_lowercase().as_str() {
                    "clip" => TextOverflow::Clip,
                    "ellipsis" => TextOverflow::Ellipsis,
                    _ => return Err(input.new_error_for_next_token()),
                });
            }
            "flex-direction" => {
                let ident = input.expect_ident()?;
                style.flex_direction = Some(match ident.as_ref() {
                    "row" => FlexDirection::Row,
                    "row-reverse" => FlexDirection::RowReverse,
                    "column" => FlexDirection::Column,
                    "column-reverse" => FlexDirection::ColumnReverse,
                    _ => return Err(input.new_error_for_next_token()),
                });
            }
            "flex-wrap" => {
                let ident = input.expect_ident()?;
                style.flex_wrap = Some(match ident.as_ref() {
                    "nowrap" => FlexWrap::NoWrap,
                    "wrap" => FlexWrap::Wrap,
                    "wrap-reverse" => FlexWrap::WrapReverse,
                    _ => return Err(input.new_error_for_next_token()),
                });
            }
            "justify-content" => {
                let ident = input.expect_ident()?;
                style.justify_content = Some(match ident.as_ref() {
                    "normal" => JustifyContent::Normal,
                    "start" => JustifyContent::Start,
                    "end" => JustifyContent::End,
                    "stretch" => JustifyContent::Stretch,
                    "flex-start" => JustifyContent::FlexStart,
                    "flex-end" => JustifyContent::FlexEnd,
                    "center" => JustifyContent::Center,
                    "space-between" => JustifyContent::SpaceBetween,
                    "space-around" => JustifyContent::SpaceAround,
                    "space-evenly" => JustifyContent::SpaceEvenly,
                    _ => return Err(input.new_error_for_next_token()),
                });
            }
            "align-items" => {
                let ident = input.expect_ident()?;
                style.align_items = Some(match ident.as_ref() {
                    "stretch" => AlignItems::Stretch,
                    "normal" => AlignItems::Normal,
                    "start" => AlignItems::Start,
                    "end" => AlignItems::End,
                    "flex-start" => AlignItems::FlexStart,
                    "flex-end" => AlignItems::FlexEnd,
                    "center" => AlignItems::Center,
                    "baseline" => AlignItems::Baseline,
                    _ => return Err(input.new_error_for_next_token()),
                });
            }
            "align-content" => {
                let ident = input.expect_ident()?;
                style.align_content = Some(match ident.as_ref() {
                    "stretch" => AlignContent::Stretch,
                    "normal" => AlignContent::Normal,
                    "start" => AlignContent::Start,
                    "end" => AlignContent::End,
                    "flex-start" => AlignContent::FlexStart,
                    "flex-end" => AlignContent::FlexEnd,
                    "center" => AlignContent::Center,
                    "space-between" => AlignContent::SpaceBetween,
                    "space-around" => AlignContent::SpaceAround,
                    "space-evenly" => AlignContent::SpaceEvenly,
                    _ => return Err(input.new_error_for_next_token()),
                });
            }
            "align-self" => {
                let ident = input.expect_ident()?;
                style.align_self = Some(match ident.as_ref() {
                    "auto" => AlignSelf::Auto,
                    "normal" => AlignSelf::Normal,
                    "start" => AlignSelf::Start,
                    "end" => AlignSelf::End,
                    "flex-start" => AlignSelf::FlexStart,
                    "flex-end" => AlignSelf::FlexEnd,
                    "center" => AlignSelf::Center,
                    "baseline" => AlignSelf::Baseline,
                    "stretch" => AlignSelf::Stretch,
                    _ => return Err(input.new_error_for_next_token()),
                });
            }
            "flex-grow" => {
                let value = input.expect_number()?;
                style.flex_grow = Some(value as f64);
            }
            "flex-shrink" => {
                let value = input.expect_number()?;
                style.flex_shrink = Some(value as f64);
            }
            "flex-basis" => {
                style.flex_basis = Some(self.parse_length_value(input)?);
            }
            "order" => {
                let value = input.expect_number()?;
                style.order = Some(value as i32);
            }
            "z-index" => {
                if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
                    style.z_index = Some(ZIndex::Auto);
                } else {
                    let value = input.expect_integer()?;
                    style.z_index = Some(ZIndex::Index(value));
                }
            }
            "gap" => {
                let row_gap = self.parse_length_value(input)?;
                let column_gap = input
                    .try_parse(|input| self.parse_length_value(input))
                    .unwrap_or_else(|_| row_gap.clone());
                style.row_gap = Some(row_gap);
                style.column_gap = Some(column_gap);
            }
            "row-gap" => {
                style.row_gap = Some(self.parse_length_value(input)?);
            }
            "column-gap" => {
                style.column_gap = Some(self.parse_length_value(input)?);
            }
            _ => {
                // Skip unknown properties
                return Err(input.new_error_for_next_token());
            }
        }

        Ok(style)
    }
}

impl StyleDeclarationParser {
    /// `none` or a length; `none` is stored as `Length::Auto`.
    fn parse_max_size<'i, 't>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Length, ParseError<'i, ()>> {
        if input
            .try_parse(|input| input.expect_ident_matching("none"))
            .is_ok()
        {
            return Ok(Length::Auto);
        }
        self.parse_length_value(input)
    }
}

impl<'i> AtRuleParser<'i> for StyleDeclarationParser {
    type Prelude = ();
    type AtRule = Declaration;
    type Error = ();
}

impl<'i> QualifiedRuleParser<'i> for StyleDeclarationParser {
    type Prelude = ();
    type QualifiedRule = Declaration;
    type Error = ();
}

impl<'i> RuleBodyItemParser<'i, Declaration, ()> for StyleDeclarationParser {
    fn parse_qualified(&self) -> bool {
        false
    }

    fn parse_declarations(&self) -> bool {
        true
    }
}
