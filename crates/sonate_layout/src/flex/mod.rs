//! CSS Flexible Box Layout Module Level 1, §9 "Flex Layout Algorithm".
//! <https://www.w3.org/TR/css-flexbox-1/#layout-algorithm>
//!
//! [`compute_flexbox`] runs the steps of §9 in order; each step lives in the file named after
//! it. Sizes are border-box sizes unless a name says `inner` (content box) or `outer` (margin
//! box). Positions are computed in flow-relative ("logical") coordinates from main-start and
//! cross-start and converted to physical coordinates at the end, which is how reverse
//! directions and `wrap-reverse` are handled.

mod abspos;
mod alignment;
mod baseline;
mod cross_size;
mod flexible_lengths;
mod intrinsic;
mod items;
mod lines;

use crate::compute::{compute_node, hide_subtree, Context, LayoutInput, LayoutOutput, RunMode};
use crate::geometry::{AvailableSpace, Axis, Edges, MaybeMath, Point, Size};
use crate::resolve::{self, BoxSizes};
use crate::style::{AlignItems, FlexWrap, Style};
use crate::tree::{Layout, LayoutTree, NodeId};
use std::ops::Range;

/// The flex container's resolved style and sizes (§9.2 #2).
pub(crate) struct Container {
    pub style: Style,
    pub main: Axis,
    pub cross: Axis,
    /// `row-reverse` / `column-reverse`: main-start is on the right/bottom.
    pub main_reversed: bool,
    /// `wrap-reverse`: cross-start is on the bottom/right.
    pub cross_reversed: bool,
    pub multi_line: bool,
    pub padding: Edges<f64>,
    pub border: Edges<f64>,
    pub padding_border: Edges<f64>,
    /// Border-box sizes decided by the parent or the container's own style.
    pub known: Size<Option<f64>>,
    /// Content-box sizes decided by the parent or the container's own style.
    pub inner_known: Size<Option<f64>>,
    /// The known content-box sizes that are definite: the basis for the items' percentages.
    pub inner_definite: Size<Option<f64>>,
    pub inner_min: Size<Option<f64>>,
    pub inner_max: Size<Option<f64>>,
    /// Space available to the content box (§9.2 #2).
    pub available: Size<AvailableSpace>,
    pub gap_main: f64,
    pub gap_cross: f64,
}

impl Container {
    fn new(tree: &LayoutTree, id: NodeId, input: &LayoutInput) -> Self {
        let style = tree.style(id).clone();
        let main = if style.flex_direction.is_row() {
            Axis::Horizontal
        } else {
            Axis::Vertical
        };
        let padding = resolve::padding(&style, input.parent_size.width);
        let padding_border = resolve::padding_border(&style, input.parent_size.width);
        let pb = padding_border.sums();
        let sizes = BoxSizes::resolve(&style, input.parent_size, pb).for_input(input.content_sizing);
        let styled = sizes.clamped_size();
        let known = input.known.or(styled);
        let inner = |size: Size<Option<f64>>| {
            size.zip_map(pb, |size, pb| size.map(|size| (size - pb).max(0.0)))
        };
        let inner_known = inner(known);
        // A size is definite if the parent says so or if the container's own style sets it.
        let definite = Size::new(
            known.width.is_some(),
            (input.known.height.is_some() && input.definite.height) || styled.height.is_some(),
        );
        let inner_definite = inner_known.zip_map(definite, |size, definite| size.filter(|_| definite));
        let inner_min = inner(sizes.min);
        let inner_max = inner(sizes.max);

        let available = Size::new(
            available_inner(input.available.width, inner_known.width, inner_min.width, inner_max.width, pb.width),
            available_inner(input.available.height, inner_known.height, inner_min.height, inner_max.height, pb.height),
        );

        // Percentage gaps resolve against the content box, or to zero while it is indefinite.
        let column_gap = style.gap.width.resolve_or_zero(inner_definite.width);
        let row_gap = style.gap.height.resolve_or_zero(inner_definite.height);
        let (gap_main, gap_cross) = match main {
            Axis::Horizontal => (column_gap, row_gap),
            Axis::Vertical => (row_gap, column_gap),
        };

        Self {
            main,
            cross: main.other(),
            main_reversed: style.flex_direction.is_reverse(),
            cross_reversed: style.flex_wrap == FlexWrap::WrapReverse,
            multi_line: style.flex_wrap != FlexWrap::NoWrap,
            padding,
            border: style.border,
            padding_border,
            known,
            inner_known,
            inner_definite,
            inner_min,
            inner_max,
            available,
            gap_main,
            gap_cross,
            style,
        }
    }

    /// The edge at main-start.
    pub fn main_start<T: Copy>(&self, edges: Edges<T>) -> T {
        if self.main_reversed {
            edges.end(self.main)
        } else {
            edges.start(self.main)
        }
    }

    pub fn main_end<T: Copy>(&self, edges: Edges<T>) -> T {
        if self.main_reversed {
            edges.start(self.main)
        } else {
            edges.end(self.main)
        }
    }

    /// The edge at cross-start.
    pub fn cross_start<T: Copy>(&self, edges: Edges<T>) -> T {
        if self.cross_reversed {
            edges.end(self.cross)
        } else {
            edges.start(self.cross)
        }
    }

    pub fn cross_end<T: Copy>(&self, edges: Edges<T>) -> T {
        if self.cross_reversed {
            edges.start(self.cross)
        } else {
            edges.end(self.cross)
        }
    }

    /// Whether the items' main sizes count as definite once flexed (§9.8 #2). Widths always do.
    pub fn main_is_definite(&self) -> bool {
        self.main == Axis::Horizontal || self.inner_definite.get(self.main).is_some()
    }

    /// The containing-block size for the items' percentages once the container's size is known.
    pub fn percentage_basis(&self, inner_main: f64, inner_cross: f64) -> Size<Option<f64>> {
        let cross_definite = self.cross == Axis::Horizontal || self.inner_definite.get(self.cross).is_some();
        self.size(
            self.main_is_definite().then_some(inner_main),
            cross_definite.then_some(inner_cross),
        )
    }

    /// Definiteness of sizes handed to an item: its main size per [`Self::main_is_definite`], its
    /// cross size when it is stretched (§9.8 #1 and #3) or fixed by its own style. Widths are
    /// always definite.
    pub fn item_definite(&self, item: &FlexItem, stretched: bool) -> Size<bool> {
        let cross_definite = stretched || item.sizes.size.get(self.cross).is_some();
        let definite = self.size(self.main_is_definite(), cross_definite);
        Size::new(true, definite.height)
    }

    /// A size with `main` and `cross` placed on the right physical axes.
    pub fn size<T>(&self, main: T, cross: T) -> Size<T> {
        match self.main {
            Axis::Horizontal => Size::new(main, cross),
            Axis::Vertical => Size::new(cross, main),
        }
    }
}

/// §9.2 #2: the content box's definite size, or the space around it minus padding and border,
/// limited by the min/max sizes.
fn available_inner(
    available: AvailableSpace,
    inner_known: Option<f64>,
    inner_min: Option<f64>,
    inner_max: Option<f64>,
    padding_border: f64,
) -> AvailableSpace {
    match (inner_known, available) {
        (Some(size), _) => AvailableSpace::Definite(size),
        (None, AvailableSpace::Definite(space)) => {
            AvailableSpace::Definite((space - padding_border).max(0.0).maybe_clamp(inner_min, inner_max))
        }
        (None, constraint) => constraint,
    }
}

/// A flex item and the sizes the algorithm computes for it.
pub(crate) struct FlexItem {
    pub id: NodeId,
    /// Border-box sizes, with percentages resolved against the container's content box.
    pub sizes: BoxSizes,
    /// `None` for `auto`.
    pub margin: Edges<Option<f64>>,
    pub padding_border: Edges<f64>,
    /// `align-self` with `auto` and `normal` resolved.
    pub align: AlignItems,
    pub grow: f64,
    pub shrink: f64,
    pub is_scroll_container: bool,
    /// The flex basis as a border-box size when it is definite (§9.2 #3 A).
    pub definite_flex_basis: Option<f64>,

    pub flex_base_size: f64,
    /// Minimum main size including the automatic minimum (§4.5).
    pub min_main: f64,
    /// The automatic minimum size is computed lazily, only for lines that shrink.
    pub auto_min_pending: bool,
    pub max_main: Option<f64>,
    pub hypothetical_main: f64,
    pub target_main: f64,
    pub frozen: bool,
    pub violation: f64,
    pub hypothetical_cross: f64,
    pub target_cross: f64,
    /// First baseline from the border-box top, for baseline alignment.
    pub baseline: Option<f64>,
    /// Margins with `auto` resolved (§9.5 #12, §9.6 #14).
    pub resolved_margin: Edges<f64>,
    /// Border-box offsets from the line's main-start and cross-start.
    pub offset_main: f64,
    pub offset_cross: f64,
}

impl FlexItem {
    pub fn margin_or_zero(&self) -> Edges<f64> {
        self.margin.map(|margin| margin.unwrap_or(0.0))
    }

    /// Sum of the margins along `axis`, with `auto` as zero.
    pub fn margin_sum(&self, axis: Axis) -> f64 {
        self.margin_or_zero().sum(axis)
    }

    /// Whether the item stretches in the cross axis (§9.4 #11).
    pub fn stretches(&self, container: &Container) -> bool {
        self.align == AlignItems::Stretch
            && self.sizes.size.get(container.cross).is_none()
            && self.margin.start(container.cross).is_some()
            && self.margin.end(container.cross).is_some()
    }

    /// Whether the item takes part in baseline alignment (§8.3).
    pub fn aligns_baseline(&self, container: &Container) -> bool {
        self.align == AlignItems::Baseline
            && container.cross == Axis::Vertical
            && self.margin.start(container.cross).is_some()
            && self.margin.end(container.cross).is_some()
    }
}

pub(crate) struct FlexLine {
    /// Indices into the item list.
    pub items: Range<usize>,
    pub cross_size: f64,
    /// Offset of the line from the content box's cross-start.
    pub offset_cross: f64,
    /// Largest distance from a baseline-aligned item's cross-start margin edge to its baseline.
    pub max_baseline: f64,
}

pub(crate) fn compute_flexbox(ctx: &mut Context, id: NodeId, input: LayoutInput) -> LayoutOutput {
    let container = Container::new(ctx.tree, id, &input);
    if input.mode == RunMode::ComputeSize {
        if let (Some(width), Some(height)) = (container.known.width, container.known.height) {
            let size = Size::new(width, height);
            return LayoutOutput {
                size,
                content_size: size,
                first_baseline: None,
            };
        }
    }

    // §9.1 Initial setup.
    let generated = items::generate_items(ctx.tree, id, &container);
    let mut items = generated.items;

    // §9.2 Line length determination.
    items::determine_flex_base_sizes(ctx, &container, &mut items);
    let inner_main = lines::determine_container_main_size(ctx, &container, &mut items);

    // §9.3 Main size determination.
    let mut lines = lines::collect_lines(&container, &items, inner_main);
    for line in &lines {
        let line_items = &mut items[line.items.clone()];
        if lines::outer_hypothetical_main_size(&container, line_items) > inner_main {
            items::resolve_automatic_minimums(ctx, &container, line_items);
        }
        flexible_lengths::resolve_flexible_lengths(&container, line_items, inner_main);
    }

    // §9.4 Cross size determination.
    cross_size::determine_hypothetical_cross_sizes(ctx, &container, &mut items, inner_main);
    baseline::determine_item_baselines(ctx, &container, &mut items, inner_main);
    cross_size::determine_line_cross_sizes(&container, &items, &mut lines);
    let inner_cross = cross_size::determine_container_cross_size(&container, &lines);
    cross_size::stretch_lines(&container, &mut lines, inner_cross);
    cross_size::determine_used_cross_sizes(&container, &mut items, &lines);

    // §9.5 Main-axis alignment and §9.6 cross-axis alignment.
    alignment::align_main_axis(&container, &mut items, &lines, inner_main);
    alignment::align_cross_axis(&container, &mut items, &mut lines);
    alignment::align_lines(&container, &mut lines, inner_cross);

    let pb = container.padding_border.sums();
    let size = container.known.zip_map(
        container.size(inner_main, inner_cross).zip_map(pb, |inner, pb| inner + pb),
        |known, computed| known.unwrap_or(computed),
    );
    if input.mode == RunMode::ComputeSize {
        return LayoutOutput {
            size,
            content_size: size,
            first_baseline: None,
        };
    }

    // Position the items and lay out their contents with the final sizes.
    let parent_size = container.percentage_basis(inner_main, inner_cross);
    let mut locations = vec![Point::new(0.0, 0.0); items.len()];
    let mut overflow = Size::ZERO;
    for line in &lines {
        for index in line.items.clone() {
            let item = &mut items[index];
            let item_size = container.size(item.target_main, item.target_cross);
            let output = compute_node(
                ctx,
                item.id,
                LayoutInput {
                    known: item_size.map(Some),
                    definite: container.item_definite(item, item.stretches(&container)),
                    parent_size,
                    available: item_size.map(AvailableSpace::Definite),
                    mode: RunMode::PerformLayout,
                    content_sizing: false,
                },
            );
            if item.baseline.is_none() {
                item.baseline = output.first_baseline;
            }

            let main = physical_offset(item.offset_main, item.target_main, inner_main, container.main_reversed);
            let cross = physical_offset(
                line.offset_cross + item.offset_cross,
                item.target_cross,
                inner_cross,
                container.cross_reversed,
            );
            let mut location = Point::new(0.0, 0.0);
            location.set(container.main, main + container.padding_border.start(container.main));
            location.set(container.cross, cross + container.padding_border.start(container.cross));
            locations[index] = location;

            let layout = Layout {
                location,
                size: output.size,
                content_size: output.content_size,
                first_baseline: output.first_baseline,
            };
            overflow = extend_overflow(overflow, &layout, item.resolved_margin);
            ctx.tree.set_layout(item.id, layout);
        }
    }

    for &child in &generated.hidden {
        hide_subtree(ctx.tree, child);
    }
    for &child in &generated.absolute {
        let layout = abspos::layout_absolute_child(ctx, &container, child, size, inner_main, inner_cross);
        overflow = extend_overflow(overflow, &layout, Edges::ZERO);
    }

    // Scrollable overflow: the children's margin boxes plus the container's end padding.
    let content_size = Size::new(
        size.width.max(overflow.width + container.padding.right),
        size.height.max(overflow.height + container.padding.bottom),
    );

    LayoutOutput {
        size,
        content_size,
        first_baseline: baseline::container_baseline(&container, &items, &lines, &locations, inner_cross),
    }
}

/// Extends the scrollable overflow area by a child's margin box and its own overflow.
fn extend_overflow(overflow: Size<f64>, child: &Layout, margin: Edges<f64>) -> Size<f64> {
    let extent = child.content_size.zip_map(child.size, f64::max);
    Size::new(
        overflow.width.max(child.location.x + extent.width + margin.right),
        overflow.height.max(child.location.y + extent.height + margin.bottom),
    )
}

/// Converts a flow-relative offset into a physical one along the same axis.
pub(crate) fn physical_offset(logical: f64, size: f64, container_size: f64, reversed: bool) -> f64 {
    if reversed {
        container_size - logical - size
    } else {
        logical
    }
}
