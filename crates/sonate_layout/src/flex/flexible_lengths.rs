//! §9.7 Resolving flexible lengths.
//! <https://www.w3.org/TR/css-flexbox-1/#resolve-flexible-lengths>

use super::{Container, FlexItem};
use crate::geometry::MaybeMath;

/// Distributes the line's free space by the grow or shrink factors, freezing items that hit
/// their min/max sizes and distributing again until every item is frozen.
pub(crate) fn resolve_flexible_lengths(container: &Container, items: &mut [FlexItem], inner_main: f64) {
    let main = container.main;
    let gaps = container.gap_main * items.len().saturating_sub(1) as f64;
    let outer_hypothetical: f64 = items
        .iter()
        .map(|item| item.hypothetical_main + item.margin_sum(main))
        .sum();

    // 1. Use the grow factors if the items fit, the shrink factors otherwise.
    let growing = outer_hypothetical + gaps < inner_main;

    // 2. Size inflexible items.
    for item in items.iter_mut() {
        let factor = if growing { item.grow } else { item.shrink };
        item.target_main = item.hypothetical_main;
        item.frozen = factor == 0.0
            || (growing && item.flex_base_size > item.hypothetical_main)
            || (!growing && item.flex_base_size < item.hypothetical_main);
    }

    // 3. Initial free space.
    let free_space = |items: &[FlexItem]| {
        let used: f64 = items
            .iter()
            .map(|item| {
                let size = if item.frozen {
                    item.target_main
                } else {
                    item.flex_base_size
                };
                size + item.margin_sum(main)
            })
            .sum();
        inner_main - used - gaps
    };
    let initial_free_space = free_space(items);

    // 4. Loop.
    loop {
        // a. Stop when every item is frozen.
        if items.iter().all(|item| item.frozen) {
            break;
        }

        // b. Remaining free space, limited when the flex factors sum to less than 1.
        let mut remaining = free_space(items);
        let factor_sum: f64 = items
            .iter()
            .filter(|item| !item.frozen)
            .map(|item| if growing { item.grow } else { item.shrink })
            .sum();
        if factor_sum < 1.0 {
            let limited = initial_free_space * factor_sum;
            if limited.abs() < remaining.abs() {
                remaining = limited;
            }
        }

        // c. Distribute the free space in proportion to the flex factors.
        if growing {
            for item in items.iter_mut().filter(|item| !item.frozen) {
                item.target_main = item.flex_base_size + remaining * item.grow / factor_sum;
            }
        } else {
            let inner_base = |item: &FlexItem| item.flex_base_size - item.padding_border.sum(main);
            let scaled_sum: f64 = items
                .iter()
                .filter(|item| !item.frozen)
                .map(|item| item.shrink * inner_base(item))
                .sum();
            for item in items.iter_mut().filter(|item| !item.frozen) {
                item.target_main = if scaled_sum > 0.0 {
                    item.flex_base_size - remaining.abs() * item.shrink * inner_base(item) / scaled_sum
                } else {
                    item.flex_base_size
                };
            }
        }

        // d. Fix min/max violations.
        let mut total_violation = 0.0;
        for item in items.iter_mut().filter(|item| !item.frozen) {
            let clamped = item
                .target_main
                .maybe_clamp(Some(item.min_main), item.max_main)
                .max(item.padding_border.sum(main));
            item.violation = clamped - item.target_main;
            item.target_main = clamped;
            total_violation += item.violation;
        }

        // e. Freeze over-flexed items.
        for item in items.iter_mut().filter(|item| !item.frozen) {
            item.frozen = if total_violation.abs() < 1e-9 {
                true
            } else if total_violation > 0.0 {
                item.violation > 0.0
            } else {
                item.violation < 0.0
            };
        }
    }
}
