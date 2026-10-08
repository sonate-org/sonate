//! A CSS flexbox layout engine that follows
//! [CSS Flexible Box Layout Module Level 1](https://www.w3.org/TR/css-flexbox-1/).
//!
//! Build a [`LayoutTree`] (children before parents), then call [`compute_layout`] with a
//! measure function for leaves such as text. Results are read with [`LayoutTree::layout`].

mod cache;
mod calc;
mod compute;
mod flex;
mod geometry;
mod leaf;
mod resolve;
mod style;
mod tree;

#[cfg(feature = "testing")]
pub mod testing;

pub use calc::Calc;
pub use compute::{compute_layout, MeasureFn, MeasureInput, MeasureOutput};
pub use geometry::{AvailableSpace, Axis, Edges, Point, Size};
pub use style::{
    AlignContent, AlignItems, BoxSizing, Dimension, Display, FlexDirection, FlexWrap,
    JustifyContent, LengthPercentage, Overflow, Position, Style,
};
pub use tree::{Layout, LayoutTree, NodeId};
