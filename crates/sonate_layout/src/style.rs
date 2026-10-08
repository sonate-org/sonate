//! The subset of CSS that affects layout, as computed values.
//!
//! `Style::default()` matches the CSS initial values, except for two Sonate-specific choices:
//! every box is a flex container (`display: flex`) and every box is a containing block for
//! absolutely positioned children (`position: relative`).

use crate::calc::Calc;
use crate::geometry::{Edges, Point, Size};
use std::sync::Arc;

/// A length that may be `auto` or a percentage.
///
/// Used for `width`/`height`, `min-*` (`Auto` = `auto`), `max-*` (`Auto` = `none`),
/// `flex-basis`, margins and insets.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Dimension {
    #[default]
    Auto,
    Px(f64),
    /// A percentage, where `Percent(50.0)` is `50%`.
    Percent(f64),
    /// `calc()`, `min()`, `max()` or `clamp()` with percentages.
    Calc(Arc<Calc>),
}

impl Dimension {
    /// Resolves against a percentage basis. Returns `None` for `auto` and when a percentage
    /// meets an indefinite basis (the value then behaves as `auto`).
    pub fn resolve(&self, basis: Option<f64>) -> Option<f64> {
        match self {
            Dimension::Auto => None,
            Dimension::Px(px) => Some(*px),
            Dimension::Percent(percent) => basis.map(|basis| basis * percent / 100.0),
            Dimension::Calc(calc) => calc.resolve(basis),
        }
    }

    /// Resolves against a percentage basis, where percentages of an indefinite basis count as
    /// zero (margins, insets). Returns `None` for `auto`.
    pub fn resolve_with_zero_basis(&self, basis: Option<f64>) -> Option<f64> {
        self.resolve(Some(basis.unwrap_or(0.0)))
    }

    pub fn is_auto(&self) -> bool {
        matches!(self, Dimension::Auto)
    }

    pub fn is_percentage(&self) -> bool {
        match self {
            Dimension::Percent(_) => true,
            Dimension::Calc(calc) => calc.has_percentage(),
            _ => false,
        }
    }
}

/// A length that may be a percentage but not `auto` (padding, gaps).
#[derive(Clone, Debug, PartialEq)]
pub enum LengthPercentage {
    Px(f64),
    /// A percentage, where `Percent(50.0)` is `50%`.
    Percent(f64),
    /// `calc()`, `min()`, `max()` or `clamp()` with percentages.
    Calc(Arc<Calc>),
}

impl Default for LengthPercentage {
    fn default() -> Self {
        LengthPercentage::Px(0.0)
    }
}

impl LengthPercentage {
    /// Resolves against a percentage basis; percentages of an indefinite basis are zero.
    pub fn resolve_or_zero(&self, basis: Option<f64>) -> f64 {
        let basis = basis.unwrap_or(0.0);
        match self {
            LengthPercentage::Px(px) => *px,
            LengthPercentage::Percent(percent) => basis * percent / 100.0,
            LengthPercentage::Calc(calc) => calc.resolve(Some(basis)).unwrap_or(0.0),
        }
    }

    pub fn is_percentage(&self) -> bool {
        match self {
            LengthPercentage::Percent(_) => true,
            LengthPercentage::Calc(calc) => calc.has_percentage(),
            LengthPercentage::Px(_) => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Display {
    #[default]
    Flex,
    None,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Position {
    #[default]
    Relative,
    Absolute,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BoxSizing {
    #[default]
    ContentBox,
    BorderBox,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Overflow {
    #[default]
    Visible,
    Clip,
    Hidden,
    Scroll,
    Auto,
}

impl Overflow {
    /// `hidden`, `scroll` and `auto` make the box a scroll container.
    pub fn is_scroll_container(self) -> bool {
        matches!(self, Overflow::Hidden | Overflow::Scroll | Overflow::Auto)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FlexDirection {
    #[default]
    Row,
    RowReverse,
    Column,
    ColumnReverse,
}

impl FlexDirection {
    pub fn is_row(self) -> bool {
        matches!(self, FlexDirection::Row | FlexDirection::RowReverse)
    }

    pub fn is_reverse(self) -> bool {
        matches!(self, FlexDirection::RowReverse | FlexDirection::ColumnReverse)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FlexWrap {
    #[default]
    NoWrap,
    Wrap,
    WrapReverse,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum JustifyContent {
    #[default]
    Normal,
    FlexStart,
    FlexEnd,
    Start,
    End,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
    Stretch,
}

/// Values of `align-items`, and of `align-self` other than `auto`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlignItems {
    #[default]
    Normal,
    Stretch,
    FlexStart,
    FlexEnd,
    Start,
    End,
    Center,
    Baseline,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlignContent {
    #[default]
    Normal,
    Stretch,
    FlexStart,
    FlexEnd,
    Start,
    End,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Style {
    pub display: Display,
    pub position: Position,
    /// `top`/`right`/`bottom`/`left`, used for absolutely positioned boxes.
    pub inset: Edges<Dimension>,
    pub box_sizing: BoxSizing,
    pub overflow: Point<Overflow>,

    pub size: Size<Dimension>,
    /// `Auto` means `auto` (the automatic minimum size for flex items).
    pub min_size: Size<Dimension>,
    /// `Auto` means `none`.
    pub max_size: Size<Dimension>,
    pub margin: Edges<Dimension>,
    pub padding: Edges<LengthPercentage>,
    /// Border widths in px.
    pub border: Edges<f64>,

    // Flex container properties.
    pub flex_direction: FlexDirection,
    pub flex_wrap: FlexWrap,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub align_content: AlignContent,
    /// `width` is `column-gap`, `height` is `row-gap`.
    pub gap: Size<LengthPercentage>,

    // Flex item properties.
    /// `None` means `auto`.
    pub align_self: Option<AlignItems>,
    pub flex_grow: f64,
    pub flex_shrink: f64,
    pub flex_basis: Dimension,
    pub order: i32,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            display: Display::Flex,
            position: Position::Relative,
            inset: Edges::all(Dimension::Auto),
            box_sizing: BoxSizing::ContentBox,
            overflow: Point::new(Overflow::Visible, Overflow::Visible),
            size: Size::new(Dimension::Auto, Dimension::Auto),
            min_size: Size::new(Dimension::Auto, Dimension::Auto),
            max_size: Size::new(Dimension::Auto, Dimension::Auto),
            margin: Edges::all(Dimension::Px(0.0)),
            padding: Edges::all(LengthPercentage::Px(0.0)),
            border: Edges::all(0.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::NoWrap,
            justify_content: JustifyContent::Normal,
            align_items: AlignItems::Normal,
            align_content: AlignContent::Normal,
            gap: Size::new(LengthPercentage::Px(0.0), LengthPercentage::Px(0.0)),
            align_self: None,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: Dimension::Auto,
            order: 0,
        }
    }
}

impl Style {
    /// Whether any property that resolves against the containing block is a percentage.
    pub(crate) fn uses_percentages(&self) -> bool {
        let dimensions = [
            &self.size.width,
            &self.size.height,
            &self.min_size.width,
            &self.min_size.height,
            &self.max_size.width,
            &self.max_size.height,
            &self.margin.top,
            &self.margin.right,
            &self.margin.bottom,
            &self.margin.left,
            &self.inset.top,
            &self.inset.right,
            &self.inset.bottom,
            &self.inset.left,
        ];
        let padding = [&self.padding.top, &self.padding.right, &self.padding.bottom, &self.padding.left];
        dimensions.iter().any(|dimension| dimension.is_percentage())
            || padding.iter().any(|padding| padding.is_percentage())
    }

    /// Whether `width`, `height` or any `min-*`/`max-*` is set.
    pub(crate) fn has_sizing_properties(&self) -> bool {
        [
            &self.size.width,
            &self.size.height,
            &self.min_size.width,
            &self.min_size.height,
            &self.max_size.width,
            &self.max_size.height,
        ]
        .iter()
        .any(|dimension| !dimension.is_auto())
    }
}
