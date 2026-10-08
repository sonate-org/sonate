//! Resolving computed styles into px values: percentages, `box-sizing` and min/max clamping.

use crate::geometry::{Axis, Edges, MaybeMath, Size};
use crate::style::{BoxSizing, Dimension, Style};

/// Padding in px. Percentages resolve against the containing block's width on both axes.
pub(crate) fn padding(style: &Style, containing_width: Option<f64>) -> Edges<f64> {
    style
        .padding
        .map(|value| value.resolve_or_zero(containing_width).max(0.0))
}

/// Padding plus border in px.
pub(crate) fn padding_border(style: &Style, containing_width: Option<f64>) -> Edges<f64> {
    padding(style, containing_width).plus(style.border.map(|width| width.max(0.0)))
}

/// Margins in px, with `None` for `auto`. Percentages resolve against the containing block's
/// width on both axes.
pub(crate) fn margin(style: &Style, containing_width: Option<f64>) -> Edges<Option<f64>> {
    style.margin.map(|value| {
        if value.is_auto() {
            None
        } else {
            Some(value.resolve(containing_width).unwrap_or(0.0))
        }
    })
}

/// Margins in px with `auto` treated as zero.
pub(crate) fn margin_or_zero(style: &Style, containing_width: Option<f64>) -> Edges<f64> {
    margin(style, containing_width).map(|value| value.unwrap_or(0.0))
}

/// The `width`/`height`/`min-*`/`max-*` properties resolved to border-box sizes.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BoxSizes {
    /// Preferred size (not clamped), `None` when `auto` or indefinite.
    pub size: Size<Option<f64>>,
    /// Minimum size, `None` when `auto`.
    pub min: Size<Option<f64>>,
    /// Maximum size, `None` when `none`.
    pub max: Size<Option<f64>>,
}

impl BoxSizes {
    /// Resolves the sizing properties. `parent_size` is the containing block's content size
    /// used for percentages and `padding_border` the box's padding plus border sums.
    pub fn resolve(style: &Style, parent_size: Size<Option<f64>>, padding_border: Size<f64>) -> Self {
        let resolve = |dimensions: Size<Dimension>| {
            Size::new(
                to_border_box(
                    style,
                    dimensions.width.resolve(parent_size.width),
                    padding_border.width,
                ),
                to_border_box(
                    style,
                    dimensions.height.resolve(parent_size.height),
                    padding_border.height,
                ),
            )
        };

        Self {
            size: resolve(style.size),
            min: resolve(style.min_size),
            max: resolve(style.max_size),
        }
    }

    /// Sizes for a box sized by its content only (see `LayoutInput::content_sizing`).
    pub fn for_input(self, content_sizing: bool) -> Self {
        if content_sizing {
            Self {
                size: Size::NONE,
                min: Size::NONE,
                max: Size::NONE,
            }
        } else {
            self
        }
    }

    /// The preferred size clamped between min and max.
    pub fn clamped_size(&self) -> Size<Option<f64>> {
        Size::new(
            self.size.width.maybe_clamp(self.min.width, self.max.width),
            self.size.height.maybe_clamp(self.min.height, self.max.height),
        )
    }

    /// Clamps a border-box length along `axis` between the min and max sizes.
    pub fn clamp(&self, axis: Axis, value: f64) -> f64 {
        value.maybe_clamp(self.min.get(axis), self.max.get(axis))
    }
}

/// Converts a `width`-like value to a border-box size according to `box-sizing`. The content box
/// never becomes negative.
pub(crate) fn to_border_box(style: &Style, value: Option<f64>, padding_border: f64) -> Option<f64> {
    value.map(|value| match style.box_sizing {
        BoxSizing::ContentBox => value.max(0.0) + padding_border,
        BoxSizing::BorderBox => value.max(padding_border),
    })
}
