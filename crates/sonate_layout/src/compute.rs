//! The recursive entry point shared by all layout algorithms.
//!
//! Every box is laid out through [`compute_node`], both to measure it (`RunMode::ComputeSize`)
//! and to lay it out for real (`RunMode::PerformLayout`). A parent measures a child under a
//! min-content, max-content or definite constraint and later lays it out with the sizes it
//! decided, which it passes as `known` sizes; this is what makes flexed and stretched sizes
//! definite for the child's own content (CSS Flexbox §9.8).

use crate::geometry::{AvailableSpace, Axis, Point, Size};
use crate::resolve::{self, BoxSizes};
use crate::style::Display;
use crate::tree::{Layout, LayoutTree, NodeId};
use crate::{flex, leaf};

/// What the measure function gets for a measured leaf. All sizes refer to the content box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeasureInput {
    /// Sizes the parent already decided.
    pub known: Size<Option<f64>>,
    /// Space to size the content into when a size is not known.
    pub available: Size<AvailableSpace>,
}

/// The content size of a measured leaf.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MeasureOutput {
    pub size: Size<f64>,
    /// Distance from the content-box top to the first baseline.
    pub first_baseline: Option<f64>,
}

pub type MeasureFn<'a> = dyn FnMut(NodeId, MeasureInput) -> MeasureOutput + 'a;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RunMode {
    /// Only compute the border-box size.
    ComputeSize,
    /// Compute the size and position all descendants.
    PerformLayout,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LayoutInput {
    /// Border-box sizes decided by the parent. They take precedence over the box's own style.
    pub known: Size<Option<f64>>,
    /// Whether each known size is definite, i.e. a basis for the box's percentages (§9.8).
    /// Widths are always definite once known; heights only when the parent says so.
    pub definite: Size<bool>,
    /// The containing block's content size, for resolving percentages.
    pub parent_size: Size<Option<f64>>,
    /// Space available to the border box.
    pub available: Size<AvailableSpace>,
    pub mode: RunMode,
    /// Size the box by its content only, ignoring its own `width`/`height`/`min-*`/`max-*`
    /// (used for the content size suggestion of the automatic minimum size, §4.5).
    pub content_sizing: bool,
}

impl LayoutInput {
    pub fn same_sizing_inputs(&self, other: &Self) -> bool {
        self.known == other.known
            && self.definite == other.definite
            && self.parent_size == other.parent_size
            && self.available == other.available
            && self.content_sizing == other.content_sizing
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct LayoutOutput {
    /// Border-box size.
    pub size: Size<f64>,
    /// Scrollable overflow extent from the border-box origin.
    pub content_size: Size<f64>,
    /// Distance from the border-box top to the first baseline.
    pub first_baseline: Option<f64>,
}

pub(crate) struct Context<'a, 'm> {
    pub tree: &'a mut LayoutTree,
    pub measure: &'a mut MeasureFn<'m>,
}

/// Measures or lays out `id`, using the per-pass cache.
pub(crate) fn compute_node(ctx: &mut Context, id: NodeId, input: LayoutInput) -> LayoutOutput {
    let input = normalize(ctx.tree, id, input);
    if let Some(output) = ctx.tree.cache_mut(id).get(&input) {
        return output;
    }

    let output = if ctx.tree.style(id).display == Display::None {
        if input.mode == RunMode::PerformLayout {
            hide_subtree(ctx.tree, id);
        }
        LayoutOutput::default()
    } else if ctx.tree.is_measured(id) {
        leaf::compute_leaf(ctx, id, input)
    } else {
        flex::compute_flexbox(ctx, id, input)
    };

    ctx.tree.cache_mut(id).store(input, output);
    output
}

/// Drops the parts of the input that cannot affect the result, so that equivalent requests hit
/// the cache. A parent measures each child several times in each of its own layouts; if
/// irrelevant differences reached the cache, the work would grow exponentially with depth.
fn normalize(tree: &LayoutTree, id: NodeId, mut input: LayoutInput) -> LayoutInput {
    for axis in [Axis::Horizontal, Axis::Vertical] {
        match input.known.get(axis) {
            Some(size) => input.available.set(axis, AvailableSpace::Definite(size)),
            None => input.definite.set(axis, false),
        }
    }
    let node = tree.node(id);
    if !node.uses_percentages {
        input.parent_size = Size::NONE;
    }
    if !node.has_sizing_properties {
        input.content_sizing = false;
    }
    input
}

/// Gives a box and its descendants an empty layout (`display: none`).
pub(crate) fn hide_subtree(tree: &mut LayoutTree, id: NodeId) {
    tree.set_layout(id, Layout::default());
    for index in 0..tree.children(id).len() {
        let child = tree.children(id)[index];
        hide_subtree(tree, child);
    }
}

/// Lays out the tree below `root`.
///
/// The root's `width`/`height` resolve against `available`; when they are `auto` the root fills
/// the definite available space, like a viewport.
pub fn compute_layout(
    tree: &mut LayoutTree,
    root: NodeId,
    available: Size<AvailableSpace>,
    measure: &mut MeasureFn,
) {
    for node in &mut tree.nodes {
        node.cache.clear();
    }

    let style = tree.style(root);
    if style.display == Display::None {
        hide_subtree(tree, root);
        return;
    }

    let parent_size = available.map(AvailableSpace::into_option);
    let margin = resolve::margin_or_zero(style, parent_size.width);
    let padding_border = resolve::padding_border(style, parent_size.width).sums();
    let sizes = BoxSizes::resolve(style, parent_size, padding_border);
    let fill = Size::new(
        parent_size
            .width
            .map(|width| sizes.clamp(Axis::Horizontal, width - margin.left - margin.right)),
        parent_size
            .height
            .map(|height| sizes.clamp(Axis::Vertical, height - margin.top - margin.bottom)),
    );
    let known = sizes.clamped_size().or(fill);

    let mut ctx = Context { tree, measure };
    let output = compute_node(
        &mut ctx,
        root,
        LayoutInput {
            known,
            definite: Size::new(true, true),
            parent_size,
            available,
            mode: RunMode::PerformLayout,
            content_sizing: false,
        },
    );
    ctx.tree.set_layout(
        root,
        Layout {
            location: Point::new(margin.left, margin.top),
            size: output.size,
            content_size: output.content_size,
            first_baseline: output.first_baseline,
        },
    );
}
