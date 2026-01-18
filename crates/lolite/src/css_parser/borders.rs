use super::parser::StyleDeclarationParser;
use crate::style::{BorderStyle, Directional, Length, Style};
use cssparser::{ParseError, Parser};

impl StyleDeclarationParser {
    pub(crate) fn try_parse_line_width<'i, 't>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Option<Length>, ParseError<'i, ()>> {
        // <line-width> = <length> | thin | medium | thick
        if let Ok(len) = input.try_parse(|i| self.parse_length_value(i)) {
            return Ok(Some(len));
        }

        if input.try_parse(|i| i.expect_ident_matching("thin")).is_ok() {
            return Ok(Some(Length::Px(1.0)));
        }
        if input
            .try_parse(|i| i.expect_ident_matching("medium"))
            .is_ok()
        {
            return Ok(Some(Length::Px(3.0)));
        }
        if input
            .try_parse(|i| i.expect_ident_matching("thick"))
            .is_ok()
        {
            return Ok(Some(Length::Px(5.0)));
        }

        Ok(None)
    }

    pub(crate) fn try_parse_line_style<'i, 't>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Option<BorderStyle>, ParseError<'i, ()>> {
        // <line-style> = none | hidden | solid | dotted | dashed | double | groove | ridge | inset | outset
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Some(BorderStyle::None));
        }
        if input
            .try_parse(|i| i.expect_ident_matching("hidden"))
            .is_ok()
        {
            return Ok(Some(BorderStyle::Hidden));
        }
        if input
            .try_parse(|i| i.expect_ident_matching("solid"))
            .is_ok()
        {
            return Ok(Some(BorderStyle::Solid));
        }
        if input
            .try_parse(|i| i.expect_ident_matching("dotted"))
            .is_ok()
        {
            return Ok(Some(BorderStyle::Dotted));
        }
        if input
            .try_parse(|i| i.expect_ident_matching("dashed"))
            .is_ok()
        {
            return Ok(Some(BorderStyle::Dashed));
        }
        if input
            .try_parse(|i| i.expect_ident_matching("double"))
            .is_ok()
        {
            return Ok(Some(BorderStyle::Double));
        }
        if input
            .try_parse(|i| i.expect_ident_matching("groove"))
            .is_ok()
        {
            return Ok(Some(BorderStyle::Groove));
        }
        if input
            .try_parse(|i| i.expect_ident_matching("ridge"))
            .is_ok()
        {
            return Ok(Some(BorderStyle::Ridge));
        }
        if input
            .try_parse(|i| i.expect_ident_matching("inset"))
            .is_ok()
        {
            return Ok(Some(BorderStyle::Inset));
        }
        if input
            .try_parse(|i| i.expect_ident_matching("outset"))
            .is_ok()
        {
            return Ok(Some(BorderStyle::Outset));
        }

        Ok(None)
    }

    pub(crate) fn parse_border_shorthand<'i, 't>(
        &mut self,
        input: &mut Parser<'i, 't>,
        style: &mut Style,
    ) -> Result<(), ParseError<'i, ()>> {
        // border: <line-width> || <line-style> || <color>
        let mut saw_width = false;
        let mut saw_color = false;
        let mut saw_style = false;
        while !input.is_exhausted() {
            if let Some(width) = self.try_parse_line_width(input)? {
                if saw_width {
                    return Err(input.new_error_for_next_token());
                }
                saw_width = true;
                style.border_width = Directional::set_all(Some(width));
                continue;
            }

            if let Ok(color) = input.try_parse(|i| self.parse_color_value(i)) {
                if saw_color {
                    return Err(input.new_error_for_next_token());
                }
                saw_color = true;
                style.border_color = Directional::set_all(Some(color));
                continue;
            }

            if let Some(border_style) = self.try_parse_line_style(input)? {
                if saw_style {
                    return Err(input.new_error_for_next_token());
                }
                saw_style = true;
                style.border_style = Directional::set_all(Some(border_style));
                continue;
            }

            // Unknown token
            return Err(input.new_error_for_next_token());
        }

        Ok(())
    }
}
