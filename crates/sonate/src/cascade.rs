//! The cascade (CSS Cascading and Inheritance Level 4): computes every node's style from the
//! matching rules, its `style` attribute and the inherited values of its parent.
//! <https://www.w3.org/TR/css-cascade-4/>

use crate::css_parser::{parse_inline_style, Specificity};
use crate::layout::{Document, LayoutContext, Node};
use crate::style::{CssWideKeyword, Declaration, Length, Style, StyleSheet};
use crate::style_matching::{Element, ElementStates};
use std::{cell::RefCell, rc::Rc, sync::Arc};

/// The initial font size, `medium`, which is 16px like in browsers.
pub const DEFAULT_FONT_SIZE_PX: f64 = 16.0;

/// Order of precedence, lowest first: `!important` beats normal declarations, the `style`
/// attribute beats rules, then specificity, then source order.
type CascadeKey = (bool, bool, Specificity, usize, usize);

/// Recomputes the style of every node and stores it in `node.layout.style`.
pub fn compute_styles(document: &Document, style_sheet: &StyleSheet, states: &ElementStates) {
    compute_subtree(
        document,
        style_sheet,
        states,
        &document.root_node(),
        &Style::default(),
    );
}

fn compute_subtree(
    document: &Document,
    style_sheet: &StyleSheet,
    states: &ElementStates,
    node: &Rc<RefCell<Node>>,
    parent_style: &Style,
) {
    let style = compute_style(document, style_sheet, states, node, parent_style);
    let children = {
        let mut node = node.borrow_mut();
        if style.widget.is_some_and(|widget| widget.is_text_input()) {
            LayoutContext::prepare_text_input_node(&mut node);
        }
        node.layout.style = Arc::new(style.clone());
        node.children.clone()
    };
    for child in &children {
        compute_subtree(document, style_sheet, states, child, &style);
    }
}

fn compute_style(
    document: &Document,
    style_sheet: &StyleSheet,
    states: &ElementStates,
    node: &Rc<RefCell<Node>>,
    parent: &Style,
) -> Style {
    let mut style = Style::default();
    inherit(&mut style, parent);
    if !Element::is_element(&node.borrow()) {
        return style;
    }

    let element = Element::new(document, node.clone());
    let mut declarations: Vec<(CascadeKey, &Declaration)> = Vec::new();
    for (rule_index, rule) in style_sheet.rules.iter().enumerate() {
        if let Some(specificity) = rule.selectors.match_specificity(&element, states) {
            for (index, declaration) in rule.declarations.iter().enumerate() {
                declarations.push((
                    (declaration.important, false, specificity, rule_index, index),
                    declaration,
                ));
            }
        }
    }
    let inline = node
        .borrow()
        .attributes
        .get("style")
        .map(|css| parse_inline_style(css))
        .unwrap_or_default();
    for (index, declaration) in inline.iter().enumerate() {
        declarations.push((
            (declaration.important, true, (0, 0, 0), 0, index),
            declaration,
        ));
    }

    declarations.sort_by_key(|(key, _)| *key);
    for (_, declaration) in declarations {
        match &declaration.keyword {
            Some((property, keyword)) => apply_keyword(&mut style, parent, property, *keyword),
            None => style.merge(&declaration.style),
        }
    }

    compute_font_size(&mut style, parent);
    style
}

/// Inherited properties take their parent's computed value unless the cascade sets them.
fn inherit(style: &mut Style, parent: &Style) {
    for property in INHERITED {
        copy_property(style, parent, property);
    }
}

const INHERITED: &[&str] = &["color", "font-family", "font-size"];

pub fn is_inherited(property: &str) -> bool {
    INHERITED.contains(&property)
}

fn apply_keyword(style: &mut Style, parent: &Style, property: &str, keyword: CssWideKeyword) {
    let inherit = match keyword {
        CssWideKeyword::Inherit => true,
        CssWideKeyword::Initial => false,
        CssWideKeyword::Unset => is_inherited(property),
    };
    if inherit {
        copy_property(style, parent, property);
    } else {
        copy_property(style, &Style::default(), property);
    }
}

/// `em` and `%` font sizes are relative to the parent's font size; the computed value is px.
fn compute_font_size(style: &mut Style, parent: &Style) {
    let parent_px = match parent.font_size {
        Some(Length::Px(px)) => px,
        _ => DEFAULT_FONT_SIZE_PX,
    };
    style.font_size = match style.font_size {
        Some(Length::Em(em)) => Some(Length::Px(em * parent_px)),
        Some(Length::Percent(percent)) => Some(Length::Px(percent / 100.0 * parent_px)),
        Some(Length::Auto) => None,
        other => other,
    };
}

pub fn is_known_property(property: &str) -> bool {
    copy_property(&mut Style::default(), &Style::default(), property)
}

/// Copies the fields that `property` (longhand or shorthand) sets from `source` into `target`.
/// Returns `false` for properties Sonate doesn't support.
pub fn copy_property(target: &mut Style, source: &Style, property: &str) -> bool {
    macro_rules! copy {
        ($($($field:ident).+),+) => {{
            $(target.$($field).+ = source.$($field).+.clone();)+
            true
        }};
    }

    match property {
        "-sonate-widget" => copy!(widget),
        "display" => copy!(display),
        "position" => copy!(position),
        "color" => copy!(color),
        "background" | "background-color" => copy!(background_color),
        "border" => copy!(border_width, border_style, border_color),
        "border-top" => copy!(border_width.top, border_style.top, border_color.top),
        "border-right" => copy!(border_width.right, border_style.right, border_color.right),
        "border-bottom" => copy!(
            border_width.bottom,
            border_style.bottom,
            border_color.bottom
        ),
        "border-left" => copy!(border_width.left, border_style.left, border_color.left),
        "border-width" => copy!(border_width),
        "border-top-width" => copy!(border_width.top),
        "border-right-width" => copy!(border_width.right),
        "border-bottom-width" => copy!(border_width.bottom),
        "border-left-width" => copy!(border_width.left),
        "border-style" => copy!(border_style),
        "border-top-style" => copy!(border_style.top),
        "border-right-style" => copy!(border_style.right),
        "border-bottom-style" => copy!(border_style.bottom),
        "border-left-style" => copy!(border_style.left),
        "border-color" => copy!(border_color),
        "border-top-color" => copy!(border_color.top),
        "border-right-color" => copy!(border_color.right),
        "border-bottom-color" => copy!(border_color.bottom),
        "border-left-color" => copy!(border_color.left),
        "border-radius" => copy!(border_radius),
        "border-top-left-radius" => copy!(border_radius.top_left),
        "border-top-right-radius" => copy!(border_radius.top_right),
        "border-bottom-right-radius" => copy!(border_radius.bottom_right),
        "border-bottom-left-radius" => copy!(border_radius.bottom_left),
        "box-sizing" => copy!(box_sizing),
        "overflow" => copy!(overflow),
        "width" => copy!(width),
        "height" => copy!(height),
        "min-width" => copy!(min_width),
        "min-height" => copy!(min_height),
        "max-width" => copy!(max_width),
        "max-height" => copy!(max_height),
        "margin" => copy!(margin),
        "margin-top" => copy!(margin.top),
        "margin-right" => copy!(margin.right),
        "margin-bottom" => copy!(margin.bottom),
        "margin-left" => copy!(margin.left),
        "padding" => copy!(padding),
        "padding-top" => copy!(padding.top),
        "padding-right" => copy!(padding.right),
        "padding-bottom" => copy!(padding.bottom),
        "padding-left" => copy!(padding.left),
        "inset" => copy!(inset),
        "top" => copy!(inset.top),
        "right" => copy!(inset.right),
        "bottom" => copy!(inset.bottom),
        "left" => copy!(inset.left),
        "flex" => copy!(flex_grow, flex_shrink, flex_basis),
        "flex-grow" => copy!(flex_grow),
        "flex-shrink" => copy!(flex_shrink),
        "flex-basis" => copy!(flex_basis),
        "flex-flow" => copy!(flex_direction, flex_wrap),
        "flex-direction" => copy!(flex_direction),
        "flex-wrap" => copy!(flex_wrap),
        "justify-content" => copy!(justify_content),
        "align-items" => copy!(align_items),
        "align-content" => copy!(align_content),
        "align-self" => copy!(align_self),
        "order" => copy!(order),
        "z-index" => copy!(z_index),
        "gap" => copy!(row_gap, column_gap),
        "row-gap" => copy!(row_gap),
        "column-gap" => copy!(column_gap),
        "font-size" => copy!(font_size),
        "font-family" => copy!(font_family),
        _ => false,
    }
}
