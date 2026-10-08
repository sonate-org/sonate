//! §9.4 Cross size determination.

use super::{Container, FlexItem, FlexLine};
use crate::compute::{compute_node, Context, LayoutInput, RunMode};
use crate::geometry::{AvailableSpace, MaybeMath};
use crate::style::AlignContent;

/// §9.4 #7: lay each item out at its used main size to find its cross size. This is where
/// nested content (wrapping text, nested containers) affects the line's cross size.
pub(crate) fn determine_hypothetical_cross_sizes(
    ctx: &mut Context,
    container: &Container,
    items: &mut [FlexItem],
    inner_main: f64,
) {
    let cross = container.cross;
    let parent_size = container.size(
        container.main_is_definite().then_some(inner_main),
        container.inner_definite.get(cross),
    );
    for item in items {
        let size = match item.sizes.size.get(cross) {
            Some(size) => size,
            None => {
                let output = compute_node(
                    ctx,
                    item.id,
                    LayoutInput {
                        known: container.size(Some(item.target_main), None),
                        definite: container.item_definite(item, false),
                        parent_size,
                        available: container.size(
                            AvailableSpace::Definite(item.target_main),
                            container.available.get(cross).shrink_by(item.margin_sum(cross)),
                        ),
                        mode: RunMode::ComputeSize,
                        content_sizing: false,
                    },
                );
                output.size.get(cross)
            }
        };
        item.hypothetical_cross = item
            .sizes
            .clamp(cross, size)
            .max(item.padding_border.sum(cross));
    }
}

/// §9.4 #8: the cross size of each line. A single-line container with a definite cross size
/// uses that size; otherwise a line is as large as its largest item, where baseline-aligned
/// items count their extent above and below the shared baseline.
pub(crate) fn determine_line_cross_sizes(container: &Container, items: &[FlexItem], lines: &mut [FlexLine]) {
    let cross = container.cross;
    for line in lines.iter_mut() {
        let mut largest: f64 = 0.0;
        let mut above: f64 = 0.0;
        let mut below: f64 = 0.0;
        for item in &items[line.items.clone()] {
            let outer = item.hypothetical_cross + item.margin_sum(cross);
            if item.aligns_baseline(container) {
                // Baselines are measured from the physical top (the cross axis is vertical).
                let baseline = item.margin_or_zero().top + item.baseline.unwrap_or(item.hypothetical_cross);
                above = above.max(baseline);
                below = below.max(outer - baseline);
            } else {
                largest = largest.max(outer);
            }
        }
        line.max_baseline = above;
        line.cross_size = largest.max(above + below);
    }

    if !container.multi_line {
        if let Some(line) = lines.first_mut() {
            line.cross_size = match container.inner_known.get(cross) {
                Some(inner) => inner,
                None => line
                    .cross_size
                    .maybe_clamp(container.inner_min.get(cross), container.inner_max.get(cross)),
            };
        }
    }
}

/// §9.4 #15: the container's inner cross size, if it isn't definite, is the sum of the lines.
/// It is needed before aligning (`align-content: stretch` fills it).
pub(crate) fn determine_container_cross_size(container: &Container, lines: &[FlexLine]) -> f64 {
    let cross = container.cross;
    container.inner_known.get(cross).unwrap_or_else(|| {
        let sum: f64 = lines.iter().map(|line| line.cross_size).sum();
        let gaps = container.gap_cross * lines.len().saturating_sub(1) as f64;
        (sum + gaps).maybe_clamp(container.inner_min.get(cross), container.inner_max.get(cross))
    })
}

/// §9.4 #9: `align-content: normal | stretch` grows the lines of a multi-line container
/// equally to fill its cross size.
pub(crate) fn stretch_lines(container: &Container, lines: &mut [FlexLine], inner_cross: f64) {
    if !container.multi_line
        || lines.is_empty()
        || !matches!(container.style.align_content, AlignContent::Normal | AlignContent::Stretch)
    {
        return;
    }
    let used: f64 = lines.iter().map(|line| line.cross_size).sum::<f64>()
        + container.gap_cross * (lines.len() - 1) as f64;
    if used < inner_cross {
        let extra = (inner_cross - used) / lines.len() as f64;
        for line in lines {
            line.cross_size += extra;
        }
    }
}

/// §9.4 #11: stretched items fill their line (clamped by their min/max cross size); all
/// others keep their hypothetical cross size.
pub(crate) fn determine_used_cross_sizes(container: &Container, items: &mut [FlexItem], lines: &[FlexLine]) {
    let cross = container.cross;
    for line in lines {
        for item in &mut items[line.items.clone()] {
            item.target_cross = if item.stretches(container) {
                item.sizes
                    .clamp(cross, line.cross_size - item.margin_sum(cross))
                    .max(item.padding_border.sum(cross))
            } else {
                item.hypothetical_cross
            };
        }
    }
}
