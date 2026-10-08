//! §9.1 Initial setup and §9.2 #3 flex base size and hypothetical main size, including the
//! automatic minimum size of flex items (§4.5).

use super::{Container, FlexItem};
use crate::compute::{compute_node, Context, LayoutInput, RunMode};
use crate::geometry::{AvailableSpace, Axis, MaybeMath};
use crate::resolve::{self, BoxSizes};
use crate::style::{AlignItems, Dimension, Display, Position};
use crate::tree::{LayoutTree, NodeId};

pub(crate) struct GeneratedItems {
    /// In-flow items in `order`-modified document order.
    pub items: Vec<FlexItem>,
    /// Absolutely positioned children; they don't take part in flex layout (§4.1).
    pub absolute: Vec<NodeId>,
    /// `display: none` children.
    pub hidden: Vec<NodeId>,
}

/// §9.1: every in-flow child becomes a flex item, sorted by `order` (stable).
pub(crate) fn generate_items(tree: &LayoutTree, id: NodeId, container: &Container) -> GeneratedItems {
    let mut in_flow = Vec::new();
    let mut absolute = Vec::new();
    let mut hidden = Vec::new();
    for &child in tree.children(id) {
        let style = tree.style(child);
        if style.display == Display::None {
            hidden.push(child);
        } else if style.position == Position::Absolute {
            absolute.push(child);
        } else {
            in_flow.push((style.order, child));
        }
    }
    in_flow.sort_by_key(|&(order, _)| order);

    GeneratedItems {
        items: in_flow
            .into_iter()
            .map(|(_, child)| new_item(tree, child, container))
            .collect(),
        absolute,
        hidden,
    }
}

fn new_item(tree: &LayoutTree, id: NodeId, container: &Container) -> FlexItem {
    let style = tree.style(id);
    let containing_width = container.inner_definite.width;
    let padding_border = resolve::padding_border(style, containing_width);
    let sizes = BoxSizes::resolve(style, container.inner_definite, padding_border.sums());

    let mut align = match style.align_self.unwrap_or(container.style.align_items) {
        AlignItems::Normal => AlignItems::Stretch,
        AlignItems::Start if container.cross_reversed => AlignItems::FlexEnd,
        AlignItems::Start => AlignItems::FlexStart,
        AlignItems::End if container.cross_reversed => AlignItems::FlexStart,
        AlignItems::End => AlignItems::FlexEnd,
        other => other,
    };
    // Baselines run horizontally, so baseline alignment needs a vertical cross axis; otherwise
    // it falls back to `self-start` (CSS Box Alignment §9.3).
    if align == AlignItems::Baseline && container.cross == Axis::Horizontal {
        align = AlignItems::FlexStart;
    }

    let main = container.main;
    let definite_flex_basis = match &style.flex_basis {
        Dimension::Auto => sizes.size.get(main),
        basis => resolve::to_border_box(
            style,
            basis.resolve(container.inner_definite.get(main)),
            padding_border.sum(main),
        ),
    };

    FlexItem {
        id,
        sizes,
        margin: resolve::margin(style, containing_width),
        padding_border,
        align,
        grow: style.flex_grow.max(0.0),
        shrink: style.flex_shrink.max(0.0),
        is_scroll_container: style.overflow.x.is_scroll_container()
            || style.overflow.y.is_scroll_container(),
        definite_flex_basis,
        flex_base_size: 0.0,
        min_main: 0.0,
        auto_min_pending: false,
        max_main: None,
        hypothetical_main: 0.0,
        target_main: 0.0,
        frozen: false,
        violation: 0.0,
        hypothetical_cross: 0.0,
        target_cross: 0.0,
        baseline: None,
        resolved_margin: Default::default(),
        offset_main: 0.0,
        offset_cross: 0.0,
    }
}

/// §9.2 #3: the flex base size and hypothetical main size of every item.
pub(crate) fn determine_flex_base_sizes(ctx: &mut Context, container: &Container, items: &mut [FlexItem]) {
    let main = container.main;
    for item in items {
        item.flex_base_size = match item.definite_flex_basis {
            // A. A definite flex basis is the flex base size.
            Some(basis) => basis,
            // C/E. Otherwise size the item under the container's constraint, or as max-content.
            None => {
                let constraint = match container.available.get(main) {
                    AvailableSpace::MinContent => AvailableSpace::MinContent,
                    _ => AvailableSpace::MaxContent,
                };
                measure_main(ctx, container, item, constraint)
            }
        }
        .max(item.padding_border.sum(main));

        item.max_main = item.sizes.max.get(main);
        let pb = item.padding_border.sum(main);
        item.min_main = match item.sizes.min.get(main) {
            Some(min) => min.max(pb),
            None if item.is_scroll_container => pb,
            // A content-based flex base size is at least the min-content size, so the automatic
            // minimum only matters once the line shrinks; see `resolve_automatic_minimums`.
            None if item.definite_flex_basis.is_none() => {
                item.auto_min_pending = true;
                pb
            }
            None => automatic_minimum_size(ctx, container, item),
        };
        item.hypothetical_main = item
            .flex_base_size
            .maybe_clamp(Some(item.min_main), item.max_main);
        item.target_main = item.hypothetical_main;
    }
}

/// Computes the automatic minimum sizes that were deferred, for items on a shrinking line.
pub(crate) fn resolve_automatic_minimums(ctx: &mut Context, container: &Container, items: &mut [FlexItem]) {
    for item in items.iter_mut().filter(|item| item.auto_min_pending) {
        item.auto_min_pending = false;
        item.min_main = automatic_minimum_size(ctx, container, item);
        item.hypothetical_main = item
            .flex_base_size
            .maybe_clamp(Some(item.min_main), item.max_main);
        item.target_main = item.hypothetical_main;
    }
}

/// The automatic minimum size of §4.5 (`min-width: auto`), which keeps items from shrinking
/// below their content: the min-content size, or the specified size if that is smaller.
fn automatic_minimum_size(ctx: &mut Context, container: &Container, item: &FlexItem) -> f64 {
    let main = container.main;
    let content_size_suggestion =
        measure_content_main(ctx, container, item, AvailableSpace::MinContent, true).maybe_min(item.max_main);
    let min = match item.sizes.size.get(main) {
        Some(specified) => specified.maybe_min(item.max_main).min(content_size_suggestion),
        None => content_size_suggestion,
    };
    min.max(item.padding_border.sum(main))
}

/// Measures the item's border-box main size under `constraint`, with its cross size known
/// when it is definite or stretched by a single-line container with a definite cross size.
pub(crate) fn measure_main(
    ctx: &mut Context,
    container: &Container,
    item: &FlexItem,
    constraint: AvailableSpace,
) -> f64 {
    measure_content_main(ctx, container, item, constraint, false)
}

/// Like [`measure_main`]; with `content_sizing` the item's own sizing properties are ignored.
fn measure_content_main(
    ctx: &mut Context,
    container: &Container,
    item: &FlexItem,
    constraint: AvailableSpace,
    content_sizing: bool,
) -> f64 {
    let cross = container.cross;
    // §9.8 #1: in a single-line container with a definite cross size, stretched items are as
    // large as the line.
    let stretched = !container.multi_line && item.stretches(container);
    let known_cross = match item.sizes.size.get(cross) {
        Some(size) => Some(item.sizes.clamp(cross, size)),
        None if stretched => container.inner_definite.get(cross).map(|inner| {
            item.sizes
                .clamp(cross, inner - item.margin_sum(cross))
                .max(item.padding_border.sum(cross))
        }),
        None => None,
    };
    let available_cross = container.available.get(cross).shrink_by(item.margin_sum(cross));

    let output = compute_node(
        ctx,
        item.id,
        LayoutInput {
            known: container.size(None, known_cross),
            definite: container.item_definite(item, known_cross.is_some()),
            parent_size: container.inner_definite,
            available: container.size(constraint, available_cross),
            mode: RunMode::ComputeSize,
            content_sizing,
        },
    );
    output.size.get(container.main)
}
