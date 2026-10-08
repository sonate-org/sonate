//! §9.2 #4 main size of the flex container and §9.3 #5 collecting items into flex lines.

use super::{intrinsic, Container, FlexItem, FlexLine};
use crate::compute::Context;
use crate::geometry::{AvailableSpace, Axis, MaybeMath};

/// §9.2 #4: the container's inner main size. When it isn't definite it comes from the items:
/// in the inline axis (rows) as fit-content between the min- and max-content sizes (§9.9.1),
/// in the block axis (columns) as the height of the content.
pub(crate) fn determine_container_main_size(
    ctx: &mut Context,
    container: &Container,
    items: &mut [FlexItem],
) -> f64 {
    let main = container.main;
    if let Some(inner) = container.inner_known.get(main) {
        return inner;
    }

    let content = match (main, container.available.get(main)) {
        (Axis::Horizontal, AvailableSpace::MinContent) => {
            intrinsic::min_content_main_size(ctx, container, items)
        }
        (Axis::Horizontal, AvailableSpace::MaxContent) => {
            intrinsic::max_content_main_size(ctx, container, items)
        }
        (Axis::Horizontal, AvailableSpace::Definite(space)) => {
            let max_content = intrinsic::max_content_main_size(ctx, container, items);
            if max_content <= space {
                max_content
            } else {
                space.max(intrinsic::min_content_main_size(ctx, container, items))
            }
        }
        (Axis::Vertical, _) => {
            // Lines only break at a max-height; an auto height grows with the content.
            let limit = container.inner_max.get(main).unwrap_or(f64::INFINITY);
            collect_lines(container, items, limit)
                .iter()
                .map(|line| outer_hypothetical_main_size(container, &items[line.items.clone()]))
                .fold(0.0, f64::max)
        }
    };
    content.maybe_clamp(container.inner_min.get(main), container.inner_max.get(main))
}

/// Outer hypothetical main sizes plus gaps.
pub(crate) fn outer_hypothetical_main_size(container: &Container, items: &[FlexItem]) -> f64 {
    let sizes: f64 = items
        .iter()
        .map(|item| item.hypothetical_main + item.margin_sum(container.main))
        .sum();
    sizes + container.gap_main * items.len().saturating_sub(1) as f64
}

/// §9.3 #5: a single-line container puts all items on one line; a multi-line container starts
/// a new line whenever the next item's outer hypothetical main size doesn't fit.
pub(crate) fn collect_lines(container: &Container, items: &[FlexItem], inner_main: f64) -> Vec<FlexLine> {
    let new_line = |start: usize, end: usize| FlexLine {
        items: start..end,
        cross_size: 0.0,
        offset_cross: 0.0,
        max_baseline: 0.0,
    };
    if items.is_empty() {
        return Vec::new();
    }
    if !container.multi_line {
        return vec![new_line(0, items.len())];
    }

    let mut lines = Vec::new();
    let mut start = 0;
    let mut used = 0.0;
    for (index, item) in items.iter().enumerate() {
        let outer = item.hypothetical_main + item.margin_sum(container.main);
        if index > start && used + container.gap_main + outer > inner_main + 1e-9 {
            lines.push(new_line(start, index));
            start = index;
            used = outer;
        } else if index == start {
            used = outer;
        } else {
            used += container.gap_main + outer;
        }
    }
    lines.push(new_line(start, items.len()));
    lines
}
