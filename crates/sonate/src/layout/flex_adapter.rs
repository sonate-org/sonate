//! Lays out the document with the `sonate_layout` engine.
//!
//! Each pass mirrors the document (with styles computed by the cascade) into a
//! `sonate_layout::LayoutTree`, computes
//! the layout with text measured by the context's `TextMeasurer`, and writes absolute
//! border-box bounds back to the nodes, shifted by the scroll offsets of scroll containers.

use super::{LayoutContext, Node, Rect, ScrollState};
use crate::style::{
    AlignContent, AlignItems, AlignSelf, BorderStyle, BoxSizing, CalcNode, Display, FlexDirection,
    FlexWrap, JustifyContent, Length, Overflow, Position, Style,
};
use crate::text::{FontSpec, TextMeasurer};
use crate::Id;
use sonate_layout as engine;
use sonate_layout::text::{TextStyle, WhiteSpace};
use sonate_layout::{AvailableSpace, MeasureInput, MeasureOutput, NodeId};
use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

/// What a measured leaf contains.
enum Content {
    None,
    /// Text that wraps at whitespace.
    Text(String, FontSpec, TextStyle),
    /// The single line of a text input, whitespace kept.
    TextInput(String, FontSpec, TextStyle),
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
    let text_style = crate::text::text_style(style);
    let content = if style.widget.is_some_and(|widget| widget.is_text_input()) {
        Content::TextInput(borrowed.text.clone().unwrap_or_default(), font.clone(), text_style)
    } else {
        match &borrowed.text {
            Some(text) if child_ids.is_empty() => Content::Text(text.clone(), font.clone(), text_style),
            _ => Content::None,
        }
    };
    let measured = !matches!(content, Content::None);
    let children = if measured { &[][..] } else { &child_ids[..] };

    let mut layout_style = convert_style(style, &font);
    // Whitespace-only text doesn't generate a flex item (CSS Flexbox §4), unless spaces are
    // preserved.
    if matches!(&content, Content::Text(text, _, style)
        if text.trim().is_empty() && matches!(style.white_space, WhiteSpace::Normal | WhiteSpace::NoWrap | WhiteSpace::PreLine))
    {
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

/// Measures text with the shared line breaking of `sonate_layout::text`.
fn measure(measurer: &dyn TextMeasurer, content: &Content, input: MeasureInput) -> MeasureOutput {
    let (text, font, style) = match content {
        Content::None => return MeasureOutput::default(),
        Content::Text(text, font, style) => (text, font, *style),
        // A text input is one line that keeps its spaces.
        Content::TextInput(text, font, style) => (
            text,
            font,
            TextStyle {
                white_space: WhiteSpace::Pre,
                ..*style
            },
        ),
    };
    let mut output = sonate_layout::text::measure_text(text, &style, measurer.metrics(font), input, &mut |run| {
        measurer.advance(run, font)
    });
    if let Content::TextInput(..) = content {
        // An empty input is still one line high.
        let line_height = measurer.metrics(font).line_height(&style);
        output.size.height = input.known.height.unwrap_or(output.size.height.max(line_height));
        output.first_baseline = Some(measurer.metrics(font).baseline(&style));
    }
    output
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
        node.layout.text_box = layout.text_box.map(|text_box| {
            Rect::new(
                x + text_box.location.x,
                y + text_box.location.y,
                text_box.size.width,
                text_box.size.height,
            )
        });

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
    let em = font.size_px();
    // After the cascade, lengths are px, percentages or math with both (other units are
    // handled for styles set without the cascade).
    let to_px = |length: &Length| match length {
        Length::Px(px) => Some(*px),
        Length::Em(value) => Some(value * em),
        _ => None,
    };
    let to_calc = |node: &CalcNode| Arc::new(convert_calc(node, &to_px));
    let dimension = |length: &Option<Length>| match length {
        None | Some(Length::Auto) => engine::Dimension::Auto,
        Some(Length::Percent(percent)) => engine::Dimension::Percent(*percent),
        Some(Length::Calc(node)) => engine::Dimension::Calc(to_calc(node)),
        Some(other) => to_px(other).map_or(engine::Dimension::Auto, engine::Dimension::Px),
    };
    // Margins default to 0 instead of `auto`.
    let margin = |length: &Option<Length>| match length {
        None => engine::Dimension::Px(0.0),
        other => dimension(other),
    };
    let length_percentage = |length: &Option<Length>| match length {
        Some(Length::Percent(percent)) => engine::LengthPercentage::Percent(*percent),
        Some(Length::Calc(node)) => engine::LengthPercentage::Calc(to_calc(node)),
        Some(other) => engine::LengthPercentage::Px(to_px(other).unwrap_or(0.0)),
        None => engine::LengthPercentage::Px(0.0),
    };
    // Like the painter, a border is drawn unless its style is explicitly `none` or `hidden`.
    let border = |width: &Option<Length>, style: Option<BorderStyle>| match style {
        Some(BorderStyle::None | BorderStyle::Hidden) => 0.0,
        _ => width.as_ref().map_or(0.0, |width| width.to_px().max(0.0)),
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
            top: dimension(&style.inset.top),
            right: dimension(&style.inset.right),
            bottom: dimension(&style.inset.bottom),
            left: dimension(&style.inset.left),
        },
        box_sizing: match style.box_sizing.unwrap_or_default() {
            BoxSizing::ContentBox => engine::BoxSizing::ContentBox,
            BoxSizing::BorderBox => engine::BoxSizing::BorderBox,
        },
        overflow: engine::Point::new(overflow, overflow),
        size: engine::Size::new(dimension(&style.width), dimension(&style.height)),
        min_size: engine::Size::new(dimension(&style.min_width), dimension(&style.min_height)),
        max_size: engine::Size::new(dimension(&style.max_width), dimension(&style.max_height)),
        margin: engine::Edges {
            top: margin(&style.margin.top),
            right: margin(&style.margin.right),
            bottom: margin(&style.margin.bottom),
            left: margin(&style.margin.left),
        },
        padding: engine::Edges {
            top: length_percentage(&style.padding.top),
            right: length_percentage(&style.padding.right),
            bottom: length_percentage(&style.padding.bottom),
            left: length_percentage(&style.padding.left),
        },
        border: engine::Edges {
            top: border(&style.border_width.top, style.border_style.top),
            right: border(&style.border_width.right, style.border_style.right),
            bottom: border(&style.border_width.bottom, style.border_style.bottom),
            left: border(&style.border_width.left, style.border_style.left),
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
            length_percentage(&style.column_gap),
            length_percentage(&style.row_gap),
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
        flex_basis: dimension(&style.flex_basis),
        order: style.order.unwrap_or(0),
    }
}

/// Converts a computed math expression (px and % leaves) for the engine.
fn convert_calc(node: &CalcNode, to_px: &dyn Fn(&Length) -> Option<f64>) -> engine::Calc {
    let all = |values: &[CalcNode]| values.iter().map(|value| convert_calc(value, to_px)).collect();
    match node {
        CalcNode::Value(Length::Percent(percent)) => engine::Calc::Percent(*percent),
        CalcNode::Value(length) => engine::Calc::Px(to_px(length).unwrap_or(0.0)),
        CalcNode::Sum(terms) => engine::Calc::Sum(all(terms)),
        CalcNode::Scale(value, factor) => engine::Calc::Scale(Box::new(convert_calc(value, to_px)), *factor),
        CalcNode::Min(values) => engine::Calc::Min(all(values)),
        CalcNode::Max(values) => engine::Calc::Max(all(values)),
        CalcNode::Clamp(min, value, max) => engine::Calc::Clamp(
            Box::new(convert_calc(min, to_px)),
            Box::new(convert_calc(value, to_px)),
            Box::new(convert_calc(max, to_px)),
        ),
    }
}
