//! The cascade (CSS Cascading and Inheritance Level 4): computes every node's style from the
//! matching rules, its `style` attribute and the inherited values of its parent.
//! <https://www.w3.org/TR/css-cascade-4/>

use crate::css_parser::{
    parse_declarations, parse_inline_style, resolve_custom_properties, substitute_var, Specificity,
};
use crate::layout::{Document, LayoutContext, Node};
use crate::style::{
    CssWideKeyword, Declaration, FontWeight, Length, LineHeight, Style, StyleSheet,
};
use crate::style_matching::{Element, ElementStates};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc, sync::Arc};

/// The initial font size, `medium`, which is 16px like in browsers.
pub const DEFAULT_FONT_SIZE_PX: f64 = 16.0;

/// Order of precedence, lowest first: `!important` beats normal declarations, the `style`
/// attribute beats rules, then specificity, then source order.
type CascadeKey = (bool, bool, Specificity, usize, usize);

/// What relative lengths are relative to.
#[derive(Clone, Copy, Debug)]
pub struct LengthContext {
    /// The element's font size, for `em`.
    pub font_size: f64,
    /// The root element's font size, for `rem`.
    pub root_font_size: f64,
    /// For `vw`, `vh`, `vmin` and `vmax`.
    pub viewport: (f64, f64),
}

impl LengthContext {
    fn absolute(&self, length: &Length) -> Option<f64> {
        let (width, height) = self.viewport;
        Some(match length {
            Length::Px(px) => *px,
            Length::Em(value) => value * self.font_size,
            Length::Rem(value) => value * self.root_font_size,
            Length::Vw(value) => value * width / 100.0,
            Length::Vh(value) => value * height / 100.0,
            Length::Vmin(value) => value * width.min(height) / 100.0,
            Length::Vmax(value) => value * width.max(height) / 100.0,
            _ => return None,
        })
    }

    /// The computed value: everything but percentages becomes px. A math expression stays one
    /// only if it contains percentages.
    pub fn compute(&self, length: &Length) -> Length {
        match length {
            Length::Calc(node) => {
                let node = node.map(&|leaf| self.compute(leaf));
                if node.has_percentage() {
                    Length::Calc(Arc::new(node))
                } else {
                    let px = node.evaluate(&|leaf| match leaf {
                        Length::Px(px) => Some(*px),
                        _ => None,
                    });
                    px.map_or(Length::Auto, Length::Px)
                }
            }
            other => self
                .absolute(other)
                .map_or_else(|| other.clone(), Length::Px),
        }
    }
}

/// Everything the cascade needs besides the node.
struct Context<'a> {
    document: &'a Document,
    style_sheet: &'a StyleSheet,
    states: &'a ElementStates,
    viewport: (f64, f64),
}

/// Recomputes the style of every node and stores it in `node.layout.style`. `viewport` is the
/// viewport size for the `vw`/`vh` units.
pub fn compute_styles(
    document: &Document,
    style_sheet: &StyleSheet,
    states: &ElementStates,
    viewport: (f64, f64),
) {
    let context = Context {
        document,
        style_sheet,
        states,
        viewport,
    };
    compute_subtree(&context, &document.root_node(), &Style::default(), None);
}

/// `root_font_size` is `None` for the root element.
fn compute_subtree(
    context: &Context,
    node: &Rc<RefCell<Node>>,
    parent_style: &Style,
    root_font_size: Option<f64>,
) {
    let style = compute_style(context, node, parent_style, root_font_size);
    let root_font_size = root_font_size.or_else(|| Some(font_size_px(&style)));
    let children = {
        let mut node = node.borrow_mut();
        if style.widget.is_some_and(|widget| widget.is_text_input()) {
            LayoutContext::prepare_text_input_node(&mut node);
        }
        node.layout.style = Arc::new(style.clone());
        node.children.clone()
    };
    for child in &children {
        compute_subtree(context, child, &style, root_font_size);
    }
}

fn compute_style(
    context: &Context,
    node: &Rc<RefCell<Node>>,
    parent: &Style,
    root_font_size: Option<f64>,
) -> Style {
    let mut style = Style::default();
    inherit(&mut style, parent);
    if !Element::is_element(&node.borrow()) {
        return style;
    }

    let element = Element::new(context.document, node.clone());
    let mut declarations: Vec<(CascadeKey, &Declaration)> = Vec::new();
    for (rule_index, rule) in context.style_sheet.rules.iter().enumerate() {
        if let Some(specificity) = rule.selectors.match_specificity(&element, context.states) {
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
    let custom = cascade_custom_properties(&mut style, parent, &declarations);
    for (_, declaration) in declarations {
        match (&declaration.keyword, &declaration.unparsed) {
            _ if declaration.custom.is_some() => {}
            (Some((property, _)), _) if property.starts_with("--") => {}
            (Some((property, keyword)), _) => apply_keyword(&mut style, parent, property, *keyword),
            (None, Some((property, text))) => apply_unparsed(&mut style, parent, property, text, &custom),
            (None, None) => style.merge(&declaration.style),
        }
    }

    // Computed values: font-size first, since `em` in other properties refers to it.
    let mut lengths = LengthContext {
        font_size: font_size_px(parent),
        root_font_size: root_font_size.unwrap_or(DEFAULT_FONT_SIZE_PX),
        viewport: context.viewport,
    };
    compute_font_size(&mut style, &lengths);
    lengths.font_size = font_size_px(&style);
    if root_font_size.is_none() {
        // On the root element, `rem` in other properties is its own font size.
        lengths.root_font_size = lengths.font_size;
    }
    compute_lengths(&mut style, &lengths);
    compute_font_weight(&mut style, parent);
    style.line_height = match style.line_height.take() {
        Some(LineHeight::Length(Length::Percent(percent))) => {
            Some(LineHeight::Length(Length::Px(percent / 100.0 * lengths.font_size)))
        }
        Some(LineHeight::Length(length)) => Some(LineHeight::Length(lengths.compute(&length))),
        other => other,
    };
    style
}

/// `bolder` and `lighter` are relative to the parent's weight (CSS Fonts §2.2).
fn compute_font_weight(style: &mut Style, parent: &Style) {
    let parent_weight = match parent.font_weight {
        Some(FontWeight::Number(weight)) => weight,
        _ => 400,
    };
    style.font_weight = match style.font_weight {
        Some(FontWeight::Bolder) => Some(FontWeight::Number(match parent_weight {
            0..350 => 400,
            350..550 => 700,
            _ => 900,
        })),
        Some(FontWeight::Lighter) => Some(FontWeight::Number(match parent_weight {
            0..550 => 100,
            550..750 => 400,
            _ => 700,
        })),
        other => other,
    };
}

fn font_size_px(style: &Style) -> f64 {
    match style.font_size {
        Some(Length::Px(px)) => px,
        _ => DEFAULT_FONT_SIZE_PX,
    }
}

/// Resolves every length to its computed value (see [`LengthContext::compute`]).
fn compute_lengths(style: &mut Style, context: &LengthContext) {
    let compute = |length: &mut Option<Length>| {
        if let Some(value) = length {
            *value = context.compute(value);
        }
    };
    for length in [
        &mut style.letter_spacing,
        &mut style.word_spacing,
        &mut style.width,
        &mut style.height,
        &mut style.min_width,
        &mut style.min_height,
        &mut style.max_width,
        &mut style.max_height,
        &mut style.flex_basis,
        &mut style.row_gap,
        &mut style.column_gap,
    ] {
        compute(length);
    }
    for edges in [
        &mut style.margin,
        &mut style.padding,
        &mut style.inset,
        &mut style.border_width,
    ] {
        for length in [
            &mut edges.top,
            &mut edges.right,
            &mut edges.bottom,
            &mut edges.left,
        ] {
            compute(length);
        }
    }
    let radius = &mut style.border_radius;
    for corner in [
        &mut radius.top_left,
        &mut radius.top_right,
        &mut radius.bottom_right,
        &mut radius.bottom_left,
    ]
    .into_iter()
    .flatten()
    {
        corner.x = context.compute(&corner.x);
        corner.y = context.compute(&corner.y);
    }
}

/// Custom properties cascade before everything else, since `var()` uses their final values.
/// They inherit, and references between them are substituted here.
fn cascade_custom_properties(
    style: &mut Style,
    parent: &Style,
    declarations: &[(CascadeKey, &Declaration)],
) -> BTreeMap<String, String> {
    let inherited = parent.custom_properties.as_deref().cloned().unwrap_or_default();
    let mut custom = inherited.clone();
    let mut changed = false;
    for (_, declaration) in declarations {
        if let Some((name, value)) = &declaration.custom {
            custom.insert(name.clone(), value.clone());
            changed = true;
        } else if let Some((name, keyword)) = &declaration.keyword {
            if name.starts_with("--") {
                changed = true;
                match (keyword, inherited.get(name)) {
                    (CssWideKeyword::Initial, _) | (_, None) => {
                        custom.remove(name);
                    }
                    (_, Some(value)) => {
                        custom.insert(name.clone(), value.clone());
                    }
                }
            }
        }
    }
    if changed {
        custom = resolve_custom_properties(&custom);
        style.custom_properties = (!custom.is_empty()).then(|| Arc::new(custom.clone()));
    }
    custom
}

/// Substitutes `var()` in a declaration and applies it. A value that is invalid after
/// substitution makes the property behave as `unset`.
fn apply_unparsed(
    style: &mut Style,
    parent: &Style,
    property: &str,
    text: &str,
    custom: &BTreeMap<String, String>,
) {
    let declarations = substitute_var(text, &mut |name| custom.get(name).cloned())
        .map(|value| parse_declarations(&format!("{property}: {value}")))
        .filter(|declarations| !declarations.is_empty());
    match declarations {
        Some(declarations) => {
            for declaration in declarations {
                match &declaration.keyword {
                    Some((property, keyword)) => apply_keyword(style, parent, property, *keyword),
                    None => style.merge(&declaration.style),
                }
            }
        }
        None => apply_keyword(style, parent, property, CssWideKeyword::Unset),
    }
}

/// Inherited properties take their parent's computed value unless the cascade sets them.
fn inherit(style: &mut Style, parent: &Style) {
    for property in INHERITED {
        copy_property(style, parent, property);
    }
    style.custom_properties = parent.custom_properties.clone();
}

const INHERITED: &[&str] = &[
    "color",
    "font-family",
    "font-size",
    "font-weight",
    "font-style",
    "line-height",
    "text-align",
    "white-space",
    "text-transform",
    "overflow-wrap",
    "word-break",
    "letter-spacing",
    "word-spacing",
];

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

/// `em` and `%` font sizes are relative to the parent's font size (`context.font_size`); the
/// computed value is px.
fn compute_font_size(style: &mut Style, context: &LengthContext) {
    let parent_px = context.font_size;
    let percent_of_parent = |leaf: &Length| match leaf {
        Length::Percent(percent) => Length::Px(percent / 100.0 * parent_px),
        other => other.clone(),
    };
    style.font_size = match style.font_size.take() {
        None | Some(Length::Auto) => None,
        Some(Length::Calc(node)) => {
            let node = Length::Calc(Arc::new(node.map(&percent_of_parent)));
            match context.compute(&node) {
                Length::Px(px) => Some(Length::Px(px.max(0.0))),
                _ => None,
            }
        }
        Some(length) => match context.compute(&percent_of_parent(&length)) {
            Length::Px(px) => Some(Length::Px(px.max(0.0))),
            _ => None,
        },
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
        "font-weight" => copy!(font_weight),
        "font-style" => copy!(font_style),
        "font" => copy!(font_family, font_size, font_weight, font_style, line_height),
        "line-height" => copy!(line_height),
        "text-align" => copy!(text_align),
        "white-space" => copy!(white_space),
        "text-transform" => copy!(text_transform),
        "overflow-wrap" | "word-wrap" => copy!(overflow_wrap),
        "word-break" => copy!(word_break),
        "letter-spacing" => copy!(letter_spacing),
        "word-spacing" => copy!(word_spacing),
        "text-overflow" => copy!(text_overflow),
        _ => false,
    }
}
