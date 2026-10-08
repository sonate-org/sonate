use crate::css_parser::SelectorList;
use sonate_macros::MergeProperties;
use std::sync::Arc;

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Clone, Default, Debug, PartialEq)]
#[allow(unused)]
pub enum Length {
    #[default]
    Auto,
    Px(f64),
    Em(f64),
    Rem(f64),
    Vw(f64),
    Vh(f64),
    Vmin(f64),
    Vmax(f64),
    Percent(f64),
    /// `calc()`, `min()`, `max()` or `clamp()`. After the cascade only px and % remain in it.
    Calc(Arc<CalcNode>),
}

impl Length {
    /// The value in px. Relative units are resolved by the cascade, so after it this is exact
    /// for everything but percentages, which count as 0.
    pub fn to_px(&self) -> f64 {
        match self {
            Length::Px(value) => *value,
            Length::Calc(calc) => calc.evaluate(&|length| match length {
                Length::Px(px) => Some(*px),
                _ => Some(0.0),
            })
            .unwrap_or(0.0),
            _ => 0.0,
        }
    }
}

/// A math expression over lengths.
#[derive(Clone, Debug, PartialEq)]
pub enum CalcNode {
    /// A length that isn't itself `Calc`.
    Value(Length),
    Sum(Vec<CalcNode>),
    /// Multiplication by a number (also used for subtraction and division).
    Scale(Box<CalcNode>, f64),
    Min(Vec<CalcNode>),
    Max(Vec<CalcNode>),
    /// `clamp(min, value, max)`
    Clamp(Box<CalcNode>, Box<CalcNode>, Box<CalcNode>),
}

impl CalcNode {
    /// Evaluates the expression with `leaf` resolving each length; `None` if a leaf can't be
    /// resolved.
    pub fn evaluate(&self, leaf: &dyn Fn(&Length) -> Option<f64>) -> Option<f64> {
        Some(match self {
            CalcNode::Value(length) => leaf(length)?,
            CalcNode::Sum(terms) => {
                let mut sum = 0.0;
                for term in terms {
                    sum += term.evaluate(leaf)?;
                }
                sum
            }
            CalcNode::Scale(value, factor) => value.evaluate(leaf)? * factor,
            CalcNode::Min(values) => values
                .iter()
                .map(|value| value.evaluate(leaf))
                .collect::<Option<Vec<_>>>()?
                .into_iter()
                .fold(f64::INFINITY, f64::min),
            CalcNode::Max(values) => values
                .iter()
                .map(|value| value.evaluate(leaf))
                .collect::<Option<Vec<_>>>()?
                .into_iter()
                .fold(f64::NEG_INFINITY, f64::max),
            CalcNode::Clamp(min, value, max) => value
                .evaluate(leaf)?
                .min(max.evaluate(leaf)?)
                .max(min.evaluate(leaf)?),
        })
    }

    /// Replaces every leaf.
    pub fn map(&self, leaf: &dyn Fn(&Length) -> Length) -> CalcNode {
        let all = |values: &[CalcNode]| values.iter().map(|value| value.map(leaf)).collect();
        match self {
            CalcNode::Value(length) => CalcNode::Value(leaf(length)),
            CalcNode::Sum(terms) => CalcNode::Sum(all(terms)),
            CalcNode::Scale(value, factor) => CalcNode::Scale(Box::new(value.map(leaf)), *factor),
            CalcNode::Min(values) => CalcNode::Min(all(values)),
            CalcNode::Max(values) => CalcNode::Max(all(values)),
            CalcNode::Clamp(min, value, max) => CalcNode::Clamp(
                Box::new(min.map(leaf)),
                Box::new(value.map(leaf)),
                Box::new(max.map(leaf)),
            ),
        }
    }

    /// Whether any leaf is a percentage.
    pub fn has_percentage(&self) -> bool {
        match self {
            CalcNode::Value(length) => matches!(length, Length::Percent(_)),
            CalcNode::Scale(value, _) => value.has_percentage(),
            CalcNode::Sum(values) | CalcNode::Min(values) | CalcNode::Max(values) => {
                values.iter().any(CalcNode::has_percentage)
            }
            CalcNode::Clamp(min, value, max) => {
                min.has_percentage() || value.has_percentage() || max.has_percentage()
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[allow(unused)]
pub struct Directional<T> {
    pub top: T,
    pub right: T,
    pub bottom: T,
    pub left: T,
}

impl<T> Directional<T> {
    pub fn set_all(value: T) -> Self
    where
        T: Clone,
    {
        Self {
            top: value.clone(),
            right: value.clone(),
            bottom: value.clone(),
            left: value,
        }
    }
}

impl Default for Directional<Length> {
    fn default() -> Self {
        Self {
            top: Length::Px(0.0),
            right: Length::Px(0.0),
            bottom: Length::Px(0.0),
            left: Length::Px(0.0),
        }
    }
}

impl<T> Default for Directional<Option<T>> {
    fn default() -> Self {
        Self {
            top: None,
            right: None,
            bottom: None,
            left: None,
        }
    }
}

impl<T> Directional<Option<T>>
where
    T: Clone,
{
    pub fn merge(&mut self, other: &Self) {
        if let Some(value) = &other.top {
            self.top = Some(value.clone());
        }
        if let Some(value) = &other.right {
            self.right = Some(value.clone());
        }
        if let Some(value) = &other.bottom {
            self.bottom = Some(value.clone());
        }
        if let Some(value) = &other.left {
            self.left = Some(value.clone());
        }
    }
}

impl Directional<Option<Length>> {
    pub fn resolved(&self) -> Directional<Length> {
        Directional::<Length> {
            top: self.top.clone().unwrap_or(Length::Px(0.0)),
            right: self.right.clone().unwrap_or(Length::Px(0.0)),
            bottom: self.bottom.clone().unwrap_or(Length::Px(0.0)),
            left: self.left.clone().unwrap_or(Length::Px(0.0)),
        }
    }
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Radius {
    pub x: Length,
    pub y: Length,
}

#[derive(Clone, Default, Debug)]
pub struct BorderRadius {
    pub top_left: Option<Radius>,
    pub top_right: Option<Radius>,
    pub bottom_right: Option<Radius>,
    pub bottom_left: Option<Radius>,
}

impl BorderRadius {
    pub fn merge(&mut self, other: &Self) {
        if let Some(v) = &other.top_left {
            self.top_left = Some(v.clone());
        }
        if let Some(v) = &other.top_right {
            self.top_right = Some(v.clone());
        }
        if let Some(v) = &other.bottom_right {
            self.bottom_right = Some(v.clone());
        }
        if let Some(v) = &other.bottom_left {
            self.bottom_left = Some(v.clone());
        }
    }

    pub fn is_empty(&self) -> bool {
        self.top_left.is_none()
            && self.top_right.is_none()
            && self.bottom_right.is_none()
            && self.bottom_left.is_none()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BorderStyle {
    None,
    Hidden,
    Solid,
    Dotted,
    Dashed,
    Double,
    Groove,
    Ridge,
    Inset,
    Outset,
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum Display {
    // Block,
    // Inline,
    // InlineBlock,
    #[default]
    Flex,
    // Grid,
    None,
}

/// Every box is a containing block for absolutely positioned descendants, so `static` and
/// `relative` behave the same.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum Position {
    #[default]
    Relative,
    Absolute,
}

#[derive(Clone, Copy, Default, Debug)]
pub enum FlexDirection {
    #[default]
    Row,
    RowReverse,
    Column,
    ColumnReverse,
}

#[derive(Clone, Copy, Default, Debug)]
pub enum FlexWrap {
    #[default]
    NoWrap,
    Wrap,
    WrapReverse,
}

#[derive(Clone, Copy, Default, Debug)]
pub enum JustifyContent {
    #[default]
    FlexStart,
    Normal,
    Start,
    End,
    Stretch,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Clone, Copy, Default, Debug)]
pub enum AlignItems {
    #[default]
    Stretch,
    Normal,
    Start,
    End,
    FlexStart,
    FlexEnd,
    Center,
    Baseline,
}

#[derive(Clone, Copy, Default, Debug)]
pub enum AlignContent {
    #[default]
    Stretch,
    Normal,
    Start,
    End,
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Clone, Copy, Default, Debug)]
pub enum AlignSelf {
    #[default]
    Auto,
    Normal,
    Start,
    End,
    FlexStart,
    FlexEnd,
    Center,
    Baseline,
    Stretch,
}

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub enum BoxSizing {
    #[default]
    ContentBox,
    BorderBox,
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum Overflow {
    #[default]
    Visible,
    Hidden,
    Scroll,
    Auto,
}

/// `font-weight`; `Bolder` and `Lighter` become numbers in the computed value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontWeight {
    Number(u16),
    Bolder,
    Lighter,
}

/// `font-style`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
    Oblique,
}

/// `line-height`; lengths and percentages become `Length(Length::Px)` in the computed value.
#[derive(Clone, Debug, PartialEq)]
pub enum LineHeight {
    Normal,
    /// A multiple of the font size, inherited as the number.
    Number(f64),
    Length(Length),
}

/// `text-overflow`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextOverflow {
    #[default]
    Clip,
    Ellipsis,
}

pub use sonate_layout::text::{OverflowWrap, TextAlign, TextTransform, WhiteSpace, WordBreak};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Widget {
    None,
    TextInput,
}

impl Widget {
    pub fn is_text_input(&self) -> bool {
        matches!(self, Widget::TextInput)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZIndex {
    Auto,
    Index(i32),
}

#[derive(Clone, Default, Debug, MergeProperties)]
pub struct Style {
    pub display: Option<Display>,
    pub position: Option<Position>,
    /// `top`, `right`, `bottom` and `left`.
    #[merge_by_method_call]
    pub inset: Directional<Option<Length>>,
    pub widget: Option<Widget>,
    pub color: Option<Rgba>,
    pub background_color: Option<Rgba>,
    #[merge_by_method_call]
    pub border_color: Directional<Option<Rgba>>,
    #[merge_by_method_call]
    pub border_width: Directional<Option<Length>>,
    #[merge_by_method_call]
    pub border_style: Directional<Option<BorderStyle>>,
    #[merge_by_method_call]
    pub border_radius: BorderRadius,
    pub box_sizing: Option<BoxSizing>,
    pub overflow: Option<Overflow>,
    #[merge_by_method_call]
    pub margin: Directional<Option<Length>>,
    #[merge_by_method_call]
    pub padding: Directional<Option<Length>>,
    pub width: Option<Length>,
    pub height: Option<Length>,
    pub min_width: Option<Length>,
    pub min_height: Option<Length>,
    /// `Length::Auto` stands for `none`.
    pub max_width: Option<Length>,
    /// `Length::Auto` stands for `none`.
    pub max_height: Option<Length>,

    /// Custom properties (`--name`), inherited, with references already substituted.
    pub custom_properties: Option<Arc<std::collections::BTreeMap<String, String>>>,

    // Text / font properties
    /// A family list as written, e.g. `"Fira Sans", sans-serif`.
    pub font_family: Option<String>,
    pub font_size: Option<Length>,
    pub font_weight: Option<FontWeight>,
    pub font_style: Option<FontStyle>,
    pub line_height: Option<LineHeight>,
    pub text_align: Option<TextAlign>,
    pub white_space: Option<WhiteSpace>,
    pub text_transform: Option<TextTransform>,
    pub overflow_wrap: Option<OverflowWrap>,
    pub word_break: Option<WordBreak>,
    /// `normal` is `Length::Px(0.0)`.
    pub letter_spacing: Option<Length>,
    pub word_spacing: Option<Length>,
    pub text_overflow: Option<TextOverflow>,

    // Flexbox container properties
    pub flex_direction: Option<FlexDirection>,
    pub flex_wrap: Option<FlexWrap>,
    pub justify_content: Option<JustifyContent>,
    pub align_items: Option<AlignItems>,
    pub align_content: Option<AlignContent>,
    pub row_gap: Option<Length>,
    pub column_gap: Option<Length>,

    // Flexbox item properties
    pub flex_grow: Option<f64>,
    pub flex_shrink: Option<f64>,
    pub flex_basis: Option<Length>,
    pub align_self: Option<AlignSelf>,
    pub order: Option<i32>,
    pub z_index: Option<ZIndex>,
}

pub struct StyleSheet {
    pub rules: Vec<Rule>,
    /// `@font-face` rules.
    pub font_faces: Vec<FontFace>,
}

/// A `@font-face` rule.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FontFace {
    pub family: String,
    /// Tried in order.
    pub sources: Vec<FontSource>,
    /// The weight the face provides (400 when not given).
    pub weight: u16,
    pub italic: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FontSource {
    /// A font file; relative paths resolve against the working directory.
    Url(String),
    /// An installed font.
    Local(String),
}

impl std::fmt::Debug for StyleSheet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StyleSheet")
            .field("rules", &self.rules)
            .field("font_faces", &self.font_faces)
            .finish()
    }
}

impl StyleSheet {
    pub fn new() -> Self {
        Self {
            rules: vec![],
            font_faces: vec![],
        }
    }

    pub fn add_rule(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    /// Whether any rule depends on `:hover`, `:active`, `:focus` or `:focus-within`, i.e.
    /// whether styles must be recomputed when those states change.
    pub fn has_state_selectors(&self) -> bool {
        self.rules
            .iter()
            .any(|rule| rule.selectors.depends_on_state())
    }
}

pub struct Rule {
    pub selectors: SelectorList,
    pub declarations: Vec<Declaration>,
}

impl std::fmt::Debug for Rule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Rule")
            .field("selectors", &self.selectors.to_string())
            .field("declarations", &self.declarations)
            .finish()
    }
}

/// `inherit`, `initial` and `unset`, which every property accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssWideKeyword {
    Inherit,
    Initial,
    Unset,
}

/// One declaration of a rule or `style` attribute.
#[derive(Clone, Debug)]
pub struct Declaration {
    /// The parsed value; only this declaration's property is set.
    pub style: Style,
    pub important: bool,
    /// Set instead of `style` for `property: inherit | initial | unset`.
    pub keyword: Option<(String, CssWideKeyword)>,
    /// A custom property, `--name: value`, as token text.
    pub custom: Option<(String, String)>,
    /// A declaration using `var()`, as `(property, text)`; it is parsed after substitution.
    pub unparsed: Option<(String, String)>,
}

impl std::ops::Deref for Declaration {
    type Target = Style;

    fn deref(&self) -> &Style {
        &self.style
    }
}
