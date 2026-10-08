//! Lays out the document with the `sonate_layout` engine.
//!
//! Each pass mirrors the document (with styles computed by the cascade) into a
//! `sonate_layout::LayoutTree`, computes
//! the layout with text measured by the context's `TextMeasurer`, and writes absolute
//! border-box bounds back to the nodes, shifted by the scroll offsets of scroll containers.

use super::{LayoutContext, Node, Rect, ScrollState};
use crate::style::{
    AlignContent, AlignItems, AlignSelf, BorderStyle, BoxSizing, Display, FlexDirection, FlexWrap,
    JustifyContent, Length, Overflow, Position, Style,
};
use crate::text::{FontSpec, TextMeasurer};
use crate::Id;
use sonate_layout as engine;
use sonate_layout::{AvailableSpace, MeasureInput, MeasureOutput, NodeId};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

/// What a measured leaf contains.
enum Content {
    None,
    /// Text that wraps at whitespace.
    Text(String, FontSpec),
    /// The single line of a text input, whitespace kept.
    TextInput(String, FontSpec),
}

/// A node of the layout tree, indexed by its `NodeId`.
struct Entry {
    node: Rc<RefCell<Node>>,
    content: Content,
}

pub(super) fn layout_document(ctx: &LayoutContext) {
    let root = ctx.document.root_node();

    let mut tree = engine::LayoutTree::new();
    let mut entries = Vec::new();
    let root_id = build(&root, &mut tree, &mut entries);

    let measurer = ctx.text_measurer.clone();
    let viewport = engine::Size::new(
        AvailableSpace::Definite(ctx.viewport_size.width),
        AvailableSpace::Definite(ctx.viewport_size.height),
    );
    engine::compute_layout(&mut tree, root_id, viewport, &mut |id, input| {
        measure(&*measurer, &entries[id.index()].content, input)
    });

    write_back(&tree, &entries, root_id, (0.0, 0.0), &ctx.scroll_state);
}

/// Adds `node` and its subtree to the layout tree (children first) and returns its id.
fn build(
    node: &Rc<RefCell<Node>>,
    tree: &mut engine::LayoutTree,
    entries: &mut Vec<Entry>,
) -> NodeId {
    let children = node.borrow().children.clone();
    let child_ids: Vec<NodeId> = children
        .iter()
        .map(|child| build(child, tree, entries))
        .collect();

    let borrowed = node.borrow();
    let style = borrowed.layout.style.as_ref();
    let font = FontSpec::from_style(style);
    let content = if style.widget.is_some_and(|widget| widget.is_text_input()) {
        Content::TextInput(borrowed.text.clone().unwrap_or_default(), font.clone())
    } else {
        match &borrowed.text {
            Some(text) if child_ids.is_empty() => Content::Text(text.clone(), font.clone()),
            _ => Content::None,
        }
    };
    let measured = !matches!(content, Content::None);
    let children = if measured { &[][..] } else { &child_ids[..] };

    let mut layout_style = convert_style(style, &font);
    // Whitespace-only text doesn't generate a flex item (CSS Flexbox §4).
    if matches!(&content, Content::Text(text, _) if text.trim().is_empty()) {
        layout_style.display = engine::Display::None;
    }
    let id = tree.push(layout_style, children, measured);
    debug_assert_eq!(id.index(), entries.len());
    entries.push(Entry {
        node: node.clone(),
        content,
    });
    id
}

/// Measures text the way Chrome sizes the anonymous block around it: an unknown width is the
/// fit-content width, and the text never wraps narrower than its longest word.
fn measure(measurer: &dyn TextMeasurer, content: &Content, input: MeasureInput) -> MeasureOutput {
    match content {
        Content::None => MeasureOutput::default(),
        Content::TextInput(text, font) => {
            let size = measurer.measure_unwrapped(text, font);
            MeasureOutput {
                size: engine::Size::new(
                    input.known.width.unwrap_or(size.width),
                    input.known.height.unwrap_or(size.height),
                ),
                first_baseline: Some(measurer.ascent(font)),
            }
        }
        Content::Text(text, font) => {
            let max_content = measurer.measure_wrapped(text, font, f64::INFINITY);
            let min_content = measurer
                .min_content_width(text, font)
                .min(max_content.width);
            let width = input.known.width.unwrap_or(match input.available.width {
                AvailableSpace::Definite(space) => space.min(max_content.width).max(min_content),
                AvailableSpace::MinContent => min_content,
                AvailableSpace::MaxContent => max_content.width,
            });
            let wrap_width = width.max(min_content).min(max_content.width);
            let height = input
                .known
                .height
                .unwrap_or_else(|| measurer.measure_wrapped(text, font, wrap_width).height);
            MeasureOutput {
                size: engine::Size::new(width, height),
                first_baseline: (!text.trim().is_empty()).then(|| measurer.ascent(font)),
            }
        }
    }
}

/// Writes absolute bounds; children of scroll containers move by the clamped scroll offset.
fn write_back(
    tree: &engine::LayoutTree,
    entries: &[Entry],
    id: NodeId,
    origin: (f64, f64),
    scroll_state: &HashMap<Id, ScrollState>,
) {
    let layout = tree.layout(id);
    let x = origin.0 + layout.location.x;
    let y = origin.1 + layout.location.y;

    let mut scroll = (0.0, 0.0);
    {
        let mut node = entries[id.index()].node.borrow_mut();
        node.layout.bounds = Rect::new(x, y, layout.size.width, layout.size.height);

        let style = node.layout.style.clone();
        if matches!(
            style.overflow,
            Some(Overflow::Scroll) | Some(Overflow::Auto)
        ) {
            if let Some(state) = scroll_state.get(&node.id) {
                let border = style.border_width.resolved();
                let max_scroll = |content: f64, size: f64, border_end: f64| {
                    if content > size {
                        (content - (size - border_end)).max(0.0)
                    } else {
                        0.0
                    }
                };
                let max_x = max_scroll(
                    layout.content_size.width,
                    layout.size.width,
                    border.right.to_px(),
                );
                let max_y = max_scroll(
                    layout.content_size.height,
                    layout.size.height,
                    border.bottom.to_px(),
                );
                state.scroll_x.set(state.scroll_x.get().clamp(0.0, max_x));
                state.scroll_y.set(state.scroll_y.get().clamp(0.0, max_y));
                scroll = (state.scroll_x.get(), state.scroll_y.get());
            }
        }
    }

    for &child in tree.children(id) {
        write_back(
            tree,
            entries,
            child,
            (x - scroll.0, y - scroll.1),
            scroll_state,
        );
    }
}

/// Converts Sonate's computed style to the engine's style. Unset properties get their CSS
/// initial values; `em` lengths resolve against the node's font size.
fn convert_style(style: &Style, font: &FontSpec) -> engine::Style {
    let em = font.size_px as f64;
    let dimension = |length: Option<Length>| match length {
        None | Some(Length::Auto) => engine::Dimension::Auto,
        Some(Length::Px(px)) => engine::Dimension::Px(px),
        Some(Length::Em(value)) => engine::Dimension::Px(value * em),
        Some(Length::Percent(percent)) => engine::Dimension::Percent(percent),
    };
    // Margins default to 0 instead of `auto`.
    let margin = |length: Option<Length>| match length {
        None => engine::Dimension::Px(0.0),
        other => dimension(other),
    };
    let length_percentage = |length: Option<Length>| match length {
        Some(Length::Px(px)) => engine::LengthPercentage::Px(px),
        Some(Length::Em(value)) => engine::LengthPercentage::Px(value * em),
        Some(Length::Percent(percent)) => engine::LengthPercentage::Percent(percent),
        None | Some(Length::Auto) => engine::LengthPercentage::Px(0.0),
    };
    // Like the painter, a border is drawn unless its style is explicitly `none` or `hidden`.
    let border = |width: Option<Length>, style: Option<BorderStyle>| match style {
        Some(BorderStyle::None | BorderStyle::Hidden) => 0.0,
        _ => match width {
            Some(Length::Px(px)) => px.max(0.0),
            Some(Length::Em(value)) => (value * em).max(0.0),
            _ => 0.0,
        },
    };
    let overflow = match style.overflow.unwrap_or_default() {
        Overflow::Visible => engine::Overflow::Visible,
        Overflow::Hidden => engine::Overflow::Hidden,
        Overflow::Scroll => engine::Overflow::Scroll,
        Overflow::Auto => engine::Overflow::Auto,
    };
    let align = |align: AlignItems| match align {
        AlignItems::Normal => engine::AlignItems::Normal,
        AlignItems::Start => engine::AlignItems::Start,
        AlignItems::End => engine::AlignItems::End,
        AlignItems::Stretch => engine::AlignItems::Stretch,
        AlignItems::FlexStart => engine::AlignItems::FlexStart,
        AlignItems::FlexEnd => engine::AlignItems::FlexEnd,
        AlignItems::Center => engine::AlignItems::Center,
        AlignItems::Baseline => engine::AlignItems::Baseline,
    };

    engine::Style {
        display: match style.display {
            Some(Display::None) => engine::Display::None,
            _ => engine::Display::Flex,
        },
        position: match style.position {
            Some(Position::Absolute) => engine::Position::Absolute,
            _ => engine::Position::Relative,
        },
        inset: engine::Edges {
            top: dimension(style.inset.top),
            right: dimension(style.inset.right),
            bottom: dimension(style.inset.bottom),
            left: dimension(style.inset.left),
        },
        box_sizing: match style.box_sizing.unwrap_or_default() {
            BoxSizing::ContentBox => engine::BoxSizing::ContentBox,
            BoxSizing::BorderBox => engine::BoxSizing::BorderBox,
        },
        overflow: engine::Point::new(overflow, overflow),
        size: engine::Size::new(dimension(style.width), dimension(style.height)),
        min_size: engine::Size::new(dimension(style.min_width), dimension(style.min_height)),
        max_size: engine::Size::new(dimension(style.max_width), dimension(style.max_height)),
        margin: engine::Edges {
            top: margin(style.margin.top),
            right: margin(style.margin.right),
            bottom: margin(style.margin.bottom),
            left: margin(style.margin.left),
        },
        padding: engine::Edges {
            top: length_percentage(style.padding.top),
            right: length_percentage(style.padding.right),
            bottom: length_percentage(style.padding.bottom),
            left: length_percentage(style.padding.left),
        },
        border: engine::Edges {
            top: border(style.border_width.top, style.border_style.top),
            right: border(style.border_width.right, style.border_style.right),
            bottom: border(style.border_width.bottom, style.border_style.bottom),
            left: border(style.border_width.left, style.border_style.left),
        },
        flex_direction: match style.flex_direction.unwrap_or_default() {
            FlexDirection::Row => engine::FlexDirection::Row,
            FlexDirection::RowReverse => engine::FlexDirection::RowReverse,
            FlexDirection::Column => engine::FlexDirection::Column,
            FlexDirection::ColumnReverse => engine::FlexDirection::ColumnReverse,
        },
        flex_wrap: match style.flex_wrap.unwrap_or_default() {
            FlexWrap::NoWrap => engine::FlexWrap::NoWrap,
            FlexWrap::Wrap => engine::FlexWrap::Wrap,
            FlexWrap::WrapReverse => engine::FlexWrap::WrapReverse,
        },
        justify_content: match style.justify_content {
            None | Some(JustifyContent::Normal) => engine::JustifyContent::Normal,
            Some(JustifyContent::Start) => engine::JustifyContent::Start,
            Some(JustifyContent::End) => engine::JustifyContent::End,
            Some(JustifyContent::Stretch) => engine::JustifyContent::Stretch,
            Some(JustifyContent::FlexStart) => engine::JustifyContent::FlexStart,
            Some(JustifyContent::FlexEnd) => engine::JustifyContent::FlexEnd,
            Some(JustifyContent::Center) => engine::JustifyContent::Center,
            Some(JustifyContent::SpaceBetween) => engine::JustifyContent::SpaceBetween,
            Some(JustifyContent::SpaceAround) => engine::JustifyContent::SpaceAround,
            Some(JustifyContent::SpaceEvenly) => engine::JustifyContent::SpaceEvenly,
        },
        align_items: style.align_items.map_or(engine::AlignItems::Normal, align),
        align_content: match style.align_content {
            None | Some(AlignContent::Normal) => engine::AlignContent::Normal,
            Some(AlignContent::Start) => engine::AlignContent::Start,
            Some(AlignContent::End) => engine::AlignContent::End,
            Some(AlignContent::Stretch) => engine::AlignContent::Stretch,
            Some(AlignContent::FlexStart) => engine::AlignContent::FlexStart,
            Some(AlignContent::FlexEnd) => engine::AlignContent::FlexEnd,
            Some(AlignContent::Center) => engine::AlignContent::Center,
            Some(AlignContent::SpaceBetween) => engine::AlignContent::SpaceBetween,
            Some(AlignContent::SpaceAround) => engine::AlignContent::SpaceAround,
            Some(AlignContent::SpaceEvenly) => engine::AlignContent::SpaceEvenly,
        },
        gap: engine::Size::new(
            length_percentage(style.column_gap),
            length_percentage(style.row_gap),
        ),
        align_self: match style.align_self {
            None | Some(AlignSelf::Auto) => None,
            Some(AlignSelf::Normal) => Some(engine::AlignItems::Normal),
            Some(AlignSelf::Start) => Some(engine::AlignItems::Start),
            Some(AlignSelf::End) => Some(engine::AlignItems::End),
            Some(AlignSelf::Stretch) => Some(engine::AlignItems::Stretch),
            Some(AlignSelf::FlexStart) => Some(engine::AlignItems::FlexStart),
            Some(AlignSelf::FlexEnd) => Some(engine::AlignItems::FlexEnd),
            Some(AlignSelf::Center) => Some(engine::AlignItems::Center),
            Some(AlignSelf::Baseline) => Some(engine::AlignItems::Baseline),
        },
        flex_grow: style.flex_grow.unwrap_or(0.0),
        flex_shrink: style.flex_shrink.unwrap_or(1.0),
        flex_basis: dimension(style.flex_basis),
        order: style.order.unwrap_or(0),
    }
}
