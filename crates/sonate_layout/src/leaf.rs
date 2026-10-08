//! Sizing of measured leaves (text, text inputs): padding, border, `box-sizing` and min/max
//! around the content size reported by the measure function.

use crate::compute::{Context, LayoutInput, LayoutOutput, MeasureInput, RunMode};
use crate::geometry::{AvailableSpace, Axis, MaybeMath, Size};
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
    LayoutOutput {
        size,
        content_size: size,
        first_baseline: measured.first_baseline.map(|baseline| baseline + content_top),
    }
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
