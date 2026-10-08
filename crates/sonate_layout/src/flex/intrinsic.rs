//! §9.9 intrinsic sizes of flex containers, as Chrome computes them.
//!
//! The max-content main size is the sum of the items' max-content contributions; the
//! min-content main size is their sum for a single-line container and the largest one for a
//! multi-line container. Like Chrome, the contributions use the items' preferred sizes and
//! min/max sizes, not their flex basis. (The cross-axis intrinsic sizes fall out of the regular
//! algorithm because the constraint is passed on to the items.)

use super::{items, Container, FlexItem};
use crate::compute::Context;
use crate::geometry::{AvailableSpace, MaybeMath};

pub(crate) fn max_content_main_size(ctx: &mut Context, container: &Container, items: &[FlexItem]) -> f64 {
    let sum: f64 = items
        .iter()
        .map(|item| contribution(ctx, container, item, AvailableSpace::MaxContent))
        .sum();
    sum + container.gap_main * items.len().saturating_sub(1) as f64
}

pub(crate) fn min_content_main_size(ctx: &mut Context, container: &Container, items: &[FlexItem]) -> f64 {
    let contributions = items
        .iter()
        .map(|item| contribution(ctx, container, item, AvailableSpace::MinContent));
    if container.multi_line {
        contributions.fold(0.0, f64::max)
    } else {
        contributions.sum::<f64>() + container.gap_main * items.len().saturating_sub(1) as f64
    }
}

/// The item's outer main size under `constraint`.
fn contribution(ctx: &mut Context, container: &Container, item: &FlexItem, constraint: AvailableSpace) -> f64 {
    let main = container.main;
    let size = match item.sizes.size.get(main) {
        Some(size) => size,
        None => items::measure_main(ctx, container, item, constraint),
    };
    size.maybe_clamp(item.sizes.min.get(main), item.sizes.max.get(main))
        .max(item.padding_border.sum(main))
        + item.margin_sum(main)
}
