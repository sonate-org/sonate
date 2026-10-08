//! Absolutely positioned children (§4.1, CSS 2 §10.3.7/§10.6.4).
//!
//! The containing block is the container's padding box (in Sonate every box is positioned).
//! Insets place the child; without insets in an axis it goes to its static position, which in
//! a flex container is where it would be as the only flex item (§4.1).

use super::alignment::Distribution;
use super::{physical_offset, Container};
use crate::compute::{compute_node, Context, LayoutInput, RunMode};
use crate::geometry::{AvailableSpace, Axis, Edges, Point, Size};
use crate::resolve::{self, BoxSizes};
use crate::style::AlignItems;
use crate::tree::{Layout, NodeId};

pub(crate) fn layout_absolute_child(
    ctx: &mut Context,
    container: &Container,
    child: NodeId,
    border_box: Size<f64>,
    inner_main: f64,
    inner_cross: f64,
) -> Layout {
    let border = container.border;
    let block = Size::new(
        (border_box.width - border.left - border.right).max(0.0),
        (border_box.height - border.top - border.bottom).max(0.0),
    );
    let style = ctx.tree.style(child).clone();
    let margin = resolve::margin(&style, Some(block.width));
    let margin_or_zero = margin.map(|margin| margin.unwrap_or(0.0));
    let padding_border = resolve::padding_border(&style, Some(block.width));
    let sizes = BoxSizes::resolve(&style, block.map(Some), padding_border.sums());
    let inset = Edges {
        top: style.inset.top.resolve(Some(block.height)),
        right: style.inset.right.resolve(Some(block.width)),
        bottom: style.inset.bottom.resolve(Some(block.height)),
        left: style.inset.left.resolve(Some(block.width)),
    };

    // An auto size between two insets fills the space between them.
    let between_insets = |axis: Axis| match (inset.start(axis), inset.end(axis)) {
        (Some(start), Some(end)) => Some(
            sizes
                .clamp(axis, block.get(axis) - start - end - margin_or_zero.sum(axis))
                .max(padding_border.sum(axis)),
        ),
        _ => None,
    };
    let known = sizes.clamped_size().zip_map(
        Size::new(between_insets(Axis::Horizontal), between_insets(Axis::Vertical)),
        Option::or,
    );
    let available = Size::new(Axis::Horizontal, Axis::Vertical).map(|axis| {
        AvailableSpace::Definite(
            (block.get(axis)
                - inset.start(axis).unwrap_or(0.0)
                - inset.end(axis).unwrap_or(0.0)
                - margin_or_zero.sum(axis))
            .max(0.0),
        )
    });

    // Find the shrink-to-fit size first, then lay out at that size so that percentages inside
    // resolve against it. Heights are only definite when they don't depend on the content.
    let definite = Size::new(true, known.height.is_some());
    let size = compute_node(
        ctx,
        child,
        LayoutInput {
            known,
            definite,
            parent_size: block.map(Some),
            available,
            mode: RunMode::ComputeSize,
            content_sizing: false,
        },
    )
    .size;
    let output = compute_node(
        ctx,
        child,
        LayoutInput {
            known: size.map(Some),
            definite,
            parent_size: block.map(Some),
            available: size.map(AvailableSpace::Definite),
            mode: RunMode::PerformLayout,
            content_sizing: false,
        },
    );

    let static_position = static_position(container, &style.align_self, size, margin_or_zero, inner_main, inner_cross);
    let mut location = Point::new(0.0, 0.0);
    for axis in [Axis::Horizontal, Axis::Vertical] {
        let start_margin = margin.start(axis);
        let end_margin = margin.end(axis);
        let offset = match (inset.start(axis), inset.end(axis)) {
            (Some(start), Some(end)) => {
                let free = block.get(axis) - start - end - size.get(axis) - margin_or_zero.sum(axis);
                match (start_margin, end_margin) {
                    (None, None) if free > 0.0 => start + free / 2.0,
                    (None, Some(_)) => start + free,
                    _ => start + margin_or_zero.start(axis),
                }
            }
            (Some(start), None) => start + margin_or_zero.start(axis),
            (None, Some(end)) => block.get(axis) - end - margin_or_zero.end(axis) - size.get(axis),
            (None, None) => static_position.get(axis),
        };
        location.set(axis, offset + border.start(axis));
    }

    let layout = Layout {
        location,
        size,
        content_size: output.content_size,
        first_baseline: output.first_baseline,
    };
    ctx.tree.set_layout(child, layout);
    layout
}

/// The child's border-box position in the padding box if it were the container's only flex
/// item, aligned by `justify-content` and its `align-self` (`stretch` acts as `flex-start`).
fn static_position(
    container: &Container,
    align_self: &Option<AlignItems>,
    size: Size<f64>,
    margin: Edges<f64>,
    inner_main: f64,
    inner_cross: f64,
) -> Point<f64> {
    let (main, cross) = (container.main, container.cross);
    let padding = container.padding;

    let outer_main = size.get(main) + margin.sum(main);
    let (offset, _) = Distribution::from_justify(container.style.justify_content, container.main_reversed)
        .offsets(inner_main - outer_main, 1, container.main_reversed);
    let logical_main = offset + container.main_start(margin);
    let main_position = physical_offset(logical_main, size.get(main), inner_main, container.main_reversed);

    let free_cross = inner_cross - size.get(cross) - margin.sum(cross);
    let logical_cross = match align_self.unwrap_or(container.style.align_items) {
        AlignItems::FlexEnd => free_cross + container.cross_start(margin),
        AlignItems::End if !container.cross_reversed => free_cross + container.cross_start(margin),
        AlignItems::Start if container.cross_reversed => free_cross + container.cross_start(margin),
        AlignItems::Center => free_cross / 2.0 + container.cross_start(margin),
        _ => container.cross_start(margin),
    };
    let cross_position = physical_offset(logical_cross, size.get(cross), inner_cross, container.cross_reversed);

    let mut position = Point::new(0.0, 0.0);
    position.set(main, main_position + padding.start(main));
    position.set(cross, cross_position + padding.start(cross));
    position
}
