//! Baselines: items' first baselines for `align-self: baseline` (§8.3) and the container's own
//! first baseline (§8.5).

use super::{physical_offset, Container, FlexItem, FlexLine};
use crate::compute::{compute_node, Context, LayoutInput, RunMode};
use crate::geometry::{AvailableSpace, Axis, Point};

/// Finds the first baseline of every baseline-aligned item by laying it out at its
/// hypothetical size. Items without a baseline get one synthesized from the bottom of their
/// border box.
pub(crate) fn determine_item_baselines(
    ctx: &mut Context,
    container: &Container,
    items: &mut [FlexItem],
    inner_main: f64,
) {
    let parent_size = container.size(
        container.main_is_definite().then_some(inner_main),
        container.inner_definite.get(container.cross),
    );
    for item in items.iter_mut().filter(|item| item.aligns_baseline(container)) {
        let size = container.size(item.target_main, item.hypothetical_cross);
        let output = compute_node(
            ctx,
            item.id,
            LayoutInput {
                known: size.map(Some),
                definite: container.item_definite(item, false),
                parent_size,
                available: size.map(AvailableSpace::Definite),
                mode: RunMode::PerformLayout,
                content_sizing: false,
            },
        );
        item.baseline = Some(output.first_baseline.unwrap_or(item.hypothetical_cross));
    }
}

/// The container's first baseline, from its border-box top: the shared baseline of the first
/// line's baseline-aligned items, or else the baseline of the line's first item (synthesized
/// from its border box if it has none).
pub(crate) fn container_baseline(
    container: &Container,
    items: &[FlexItem],
    lines: &[FlexLine],
    locations: &[Point<f64>],
    inner_cross: f64,
) -> Option<f64> {
    let line = lines.first()?;
    let first_line = &items[line.items.clone()];

    if container.cross == Axis::Vertical && first_line.iter().any(|item| item.aligns_baseline(container)) {
        let line_top = physical_offset(line.offset_cross, line.cross_size, inner_cross, container.cross_reversed);
        return Some(container.padding_border.top + line_top + line.max_baseline);
    }

    let index = line.items.start;
    let item = &items[index];
    let height = container.size(item.target_main, item.target_cross).height;
    Some(locations[index].y + item.baseline.unwrap_or(height))
}
