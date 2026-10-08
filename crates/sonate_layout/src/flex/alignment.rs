//! §9.5 main-axis alignment (§8.1 auto margins, §8.2 `justify-content`) and §9.6 cross-axis
//! alignment (§8.3 `align-self`, §8.4 `align-content`).
//!
//! Offsets are flow-relative: from main-start and cross-start. `start`/`end` values follow the
//! writing mode, so in reversed directions they map to the opposite flex-relative edge.

use super::{Container, FlexItem, FlexLine};
use crate::geometry::Edges;
use crate::style::{AlignContent, AlignItems, JustifyContent};

/// How free space is distributed along an axis.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Distribution {
    Start,
    End,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

impl Distribution {
    pub fn from_justify(justify: JustifyContent, reversed: bool) -> Self {
        match justify {
            JustifyContent::Normal | JustifyContent::Stretch | JustifyContent::FlexStart => Self::Start,
            JustifyContent::FlexEnd => Self::End,
            JustifyContent::Start if reversed => Self::End,
            JustifyContent::Start => Self::Start,
            JustifyContent::End if reversed => Self::Start,
            JustifyContent::End => Self::End,
            JustifyContent::Center => Self::Center,
            JustifyContent::SpaceBetween => Self::SpaceBetween,
            JustifyContent::SpaceAround => Self::SpaceAround,
            JustifyContent::SpaceEvenly => Self::SpaceEvenly,
        }
    }

    fn from_align_content(align: AlignContent, reversed: bool) -> Self {
        match align {
            // Lines were already stretched.
            AlignContent::Normal | AlignContent::Stretch | AlignContent::FlexStart => Self::Start,
            AlignContent::FlexEnd => Self::End,
            AlignContent::Start if reversed => Self::End,
            AlignContent::Start => Self::Start,
            AlignContent::End if reversed => Self::Start,
            AlignContent::End => Self::End,
            AlignContent::Center => Self::Center,
            AlignContent::SpaceBetween => Self::SpaceBetween,
            AlignContent::SpaceAround => Self::SpaceAround,
            AlignContent::SpaceEvenly => Self::SpaceEvenly,
        }
    }

    /// Returns the offset before the first subject and the extra space between subjects.
    /// `reversed` says whether the flex-relative start is the writing mode's end.
    ///
    /// Without free space the `space-*` values use their fallback alignment from CSS Box
    /// Alignment, as Chrome does: `space-between` falls back to `flex-start`, `space-around`
    /// and `space-evenly` to `safe center`, which aligns overflowing content to the writing
    /// mode's start. (Flexbox Level 1 says plain `center`.)
    pub fn offsets(self, free_space: f64, count: usize, reversed: bool) -> (f64, f64) {
        match self {
            Self::Start => (0.0, 0.0),
            Self::End => (free_space, 0.0),
            Self::Center => (free_space / 2.0, 0.0),
            Self::SpaceBetween if free_space > 0.0 && count > 1 => {
                (0.0, free_space / (count - 1) as f64)
            }
            Self::SpaceBetween => (0.0, 0.0),
            Self::SpaceAround if free_space > 0.0 => {
                let between = free_space / count as f64;
                (between / 2.0, between)
            }
            Self::SpaceEvenly if free_space > 0.0 => {
                let between = free_space / (count + 1) as f64;
                (between, between)
            }
            Self::SpaceAround | Self::SpaceEvenly if reversed => (free_space, 0.0),
            Self::SpaceAround | Self::SpaceEvenly => (0.0, 0.0),
        }
    }
}

/// §9.5 #12–13: auto margins take positive free space first, otherwise `justify-content`
/// distributes it.
pub(crate) fn align_main_axis(container: &Container, items: &mut [FlexItem], lines: &[FlexLine], inner_main: f64) {
    let main = container.main;
    let distribution = Distribution::from_justify(container.style.justify_content, container.main_reversed);
    for line in lines {
        let items = &mut items[line.items.clone()];
        let used: f64 = items
            .iter()
            .map(|item| item.target_main + item.margin_sum(main))
            .sum::<f64>()
            + container.gap_main * items.len().saturating_sub(1) as f64;
        let free_space = inner_main - used;
        let auto_margins: usize = items
            .iter()
            .map(|item| {
                usize::from(item.margin.start(main).is_none()) + usize::from(item.margin.end(main).is_none())
            })
            .sum();

        for item in items.iter_mut() {
            item.resolved_margin = item.margin_or_zero();
        }
        let (start, between) = if auto_margins > 0 {
            if free_space > 0.0 {
                let share = free_space / auto_margins as f64;
                for item in items.iter_mut() {
                    set_auto_margins(&mut item.resolved_margin, item.margin, main, share, share);
                }
            }
            (0.0, 0.0)
        } else {
            distribution.offsets(free_space, items.len(), container.main_reversed)
        };

        let mut cursor = start;
        for (index, item) in items.iter_mut().enumerate() {
            if index > 0 {
                cursor += container.gap_main + between;
            }
            cursor += container.main_start(item.resolved_margin);
            item.offset_main = cursor;
            cursor += item.target_main + container.main_end(item.resolved_margin);
        }
    }
}

/// Replaces the `auto` margins along `axis` with the given values.
fn set_auto_margins(
    resolved: &mut Edges<f64>,
    specified: Edges<Option<f64>>,
    axis: crate::geometry::Axis,
    start: f64,
    end: f64,
) {
    use crate::geometry::Axis;
    match axis {
        Axis::Horizontal => {
            if specified.left.is_none() {
                resolved.left = start;
            }
            if specified.right.is_none() {
                resolved.right = end;
            }
        }
        Axis::Vertical => {
            if specified.top.is_none() {
                resolved.top = start;
            }
            if specified.bottom.is_none() {
                resolved.bottom = end;
            }
        }
    }
}

/// §9.6 #13–14: positions every item within its line: cross-axis auto margins first, then
/// `align-self`.
pub(crate) fn align_cross_axis(container: &Container, items: &mut [FlexItem], lines: &mut [FlexLine]) {
    let cross = container.cross;
    let to_logical = |physical: f64, size: f64, line_size: f64| {
        if container.cross_reversed {
            line_size - physical - size
        } else {
            physical
        }
    };

    for line in lines.iter() {
        for item in &mut items[line.items.clone()] {
            let start_auto = item.margin.start(cross).is_none();
            let end_auto = item.margin.end(cross).is_none();
            let free_space = line.cross_size - item.target_cross - item.margin_sum(cross);

            if start_auto || end_auto {
                if free_space > 0.0 {
                    let share = free_space / (usize::from(start_auto) + usize::from(end_auto)) as f64;
                    set_auto_margins(&mut item.resolved_margin, item.margin, cross, share, share);
                }
                // Without free space, the physical start margin is zero and the item overflows
                // at the end.
                let physical = item.resolved_margin.start(cross);
                item.offset_cross = to_logical(physical, item.target_cross, line.cross_size);
                continue;
            }

            let margin = item.resolved_margin;
            item.offset_cross = match item.align {
                AlignItems::FlexEnd => line.cross_size - item.target_cross - container.cross_end(margin),
                AlignItems::Center => container.cross_start(margin) + free_space / 2.0,
                AlignItems::Baseline if item.aligns_baseline(container) => {
                    let baseline = item.baseline.unwrap_or(item.target_cross);
                    to_logical(line.max_baseline - baseline, item.target_cross, line.cross_size)
                }
                _ => container.cross_start(margin),
            };
        }
    }
}

/// §9.6 #16: distributes the lines of a multi-line container by `align-content`.
pub(crate) fn align_lines(container: &Container, lines: &mut [FlexLine], inner_cross: f64) {
    if !container.multi_line {
        return;
    }
    let used: f64 = lines.iter().map(|line| line.cross_size).sum::<f64>()
        + container.gap_cross * lines.len().saturating_sub(1) as f64;
    let (start, between) = Distribution::from_align_content(container.style.align_content, container.cross_reversed)
        .offsets(inner_cross - used, lines.len(), container.cross_reversed);

    let mut cursor = start;
    for line in lines {
        line.offset_cross = cursor;
        cursor += line.cross_size + container.gap_cross + between;
    }
}
