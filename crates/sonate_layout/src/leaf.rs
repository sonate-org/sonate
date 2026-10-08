//! Sizing of measured leaves (text, text inputs): padding, border, `box-sizing` and min/max
//! around the content size reported by the measure function, and the placement of that
//! content inside the leaf.

use crate::compute::{Context, LayoutInput, LayoutOutput, MeasureInput, RunMode};
use crate::flex::alignment::Distribution;
use crate::flex::physical_offset;
use crate::geometry::{AvailableSpace, Axis, Edges, MaybeMath, Point, Size};
use crate::style::AlignItems;
use crate::tree::TextBox;
use crate::resolve::{self, BoxSizes};
use crate::tree::NodeId;

pub(crate) fn compute_leaf(ctx: &mut Context, id: NodeId, input: LayoutInput) -> LayoutOutput {
    let style = ctx.tree.style(id);
    let padding = resolve::padding(style, input.parent_size.width);
    let padding_border = resolve::padding_border(style, input.parent_size.width);
    let pb = padding_border.sums();
    let sizes = BoxSizes::resolve(style, input.parent_size, pb).for_input(input.content_sizing);
    let content_top = padding.top + style.border.top;

    let known = input.known.or(sizes.clamped_size());
    if input.mode == RunMode::ComputeSize {
        if let (Some(width), Some(height)) = (known.width, known.height) {
            let size = Size::new(width, height);
            return LayoutOutput {
                size,
                content_size: size,
                first_baseline: None,
                text_box: None,
            };
        }
    }

    let measure = |ctx: &mut Context, known: Size<Option<f64>>| {
        let content_known = known.zip_map(pb, |size, pb| size.map(|size| (size - pb).max(0.0)));
        let content_available = Size::new(
            content_available(input.available.width, &sizes, Axis::Horizontal, pb.width),
            content_available(input.available.height, &sizes, Axis::Vertical, pb.height),
        );
        (ctx.measure)(
            id,
            MeasureInput {
                known: content_known,
                available: content_available,
            },
        )
    };

    let mut measured = measure(ctx, known);
    let width = known.width.unwrap_or_else(|| {
        (measured.size.width + pb.width)
            .maybe_clamp(sizes.min.width, sizes.max.width)
            .max(pb.width)
    });
    // A clamped width changes where the content wraps, so measure again at that width.
    if known.width.is_none()
        && known.height.is_none()
        && (width - pb.width - measured.size.width).abs() > 1e-9
    {
        measured = measure(ctx, Size::new(Some(width), None));
    }
    let height = known.height.unwrap_or_else(|| {
        (measured.size.height + pb.height)
            .maybe_clamp(sizes.min.height, sizes.max.height)
            .max(pb.height)
    });

    let size = Size::new(width, height);
    if input.mode == RunMode::ComputeSize {
        return LayoutOutput {
            size,
            content_size: size,
            first_baseline: measured.first_baseline.map(|baseline| baseline + content_top),
            text_box: None,
        };
    }

    // Place the content like Chrome places the anonymous block around an element's text: as
    // the element's only flex item.
    let padding_border = resolve::padding_border(ctx.tree.style(id), input.parent_size.width);
    let content = Size::new(
        (size.width - pb.width).max(0.0),
        (size.height - pb.height).max(0.0),
    );
    let (text_box, baseline) = place_content(ctx, id, content, padding_border);
    let content_size = Size::new(
        size.width.max(text_box.location.x + text_box.size.width + padding_border.right),
        size.height.max(text_box.location.y + text_box.size.height + padding_border.bottom),
    );
    LayoutOutput {
        size,
        content_size,
        first_baseline: baseline.map(|baseline| baseline + text_box.location.y),
        text_box: Some(text_box),
    }
}

/// Sizes and aligns the leaf's content within its content box: fit-content in the main axis,
/// stretched or measured in the cross axis, positioned by `justify-content` and `align-items`.
/// Returns the box (relative to the border box) and the content's first baseline.
fn place_content(
    ctx: &mut Context,
    id: NodeId,
    content: Size<f64>,
    padding_border: Edges<f64>,
) -> (TextBox, Option<f64>) {
    let style = ctx.tree.style(id);
    let main = if style.flex_direction.is_row() {
        Axis::Horizontal
    } else {
        Axis::Vertical
    };
    let cross = main.other();
    let main_reversed = style.flex_direction.is_reverse();
    let justify = style.justify_content;
    let align = style.align_items;
    let stretch = matches!(align, AlignItems::Normal | AlignItems::Stretch | AlignItems::Baseline);

    let measure = |ctx: &mut Context, known_width: Option<f64>| {
        (ctx.measure)(
            id,
            MeasureInput {
                known: Size::new(known_width, None),
                available: Size::new(AvailableSpace::Definite(content.width), AvailableSpace::MaxContent),
            },
        )
    };
    let (size, measured) = if main == Axis::Horizontal {
        let measured = measure(ctx, None);
        let height = if stretch { content.height } else { measured.size.height };
        (Size::new(measured.size.width, height), measured)
    } else {
        let width = if stretch { Some(content.width) } else { None };
        let measured = measure(ctx, width);
        (Size::new(width.unwrap_or(measured.size.width), measured.size.height), measured)
    };

    let free_main = content.get(main) - size.get(main);
    let (offset, _) = Distribution::from_justify(justify, main_reversed).offsets(free_main, 1, main_reversed);
    let main_offset = physical_offset(offset, size.get(main), content.get(main), main_reversed);
    let free_cross = content.get(cross) - size.get(cross);
    let cross_offset = match align {
        AlignItems::Center => free_cross / 2.0,
        AlignItems::FlexEnd | AlignItems::End => free_cross,
        _ => 0.0,
    };

    let mut location = Point::new(0.0, 0.0);
    location.set(main, main_offset + padding_border.start(main));
    location.set(cross, cross_offset + padding_border.start(cross));
    (TextBox { location, size }, measured.first_baseline)
}

/// The space for the content box: the border-box space minus padding and border, limited by
/// the min/max sizes so that content wraps where the box will end up.
fn content_available(
    available: AvailableSpace,
    sizes: &BoxSizes,
    axis: Axis,
    padding_border: f64,
) -> AvailableSpace {
    match available {
        AvailableSpace::Definite(space) => AvailableSpace::Definite(
            (space.maybe_clamp(sizes.min.get(axis), sizes.max.get(axis)) - padding_border).max(0.0),
        ),
        other => other,
    }
}
