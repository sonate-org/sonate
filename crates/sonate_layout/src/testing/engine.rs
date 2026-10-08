//! Runs a snapshot case through this crate's engine.

use super::ahem::measure_text;
use super::harness::CaseOutput;
use super::snapshot::SnapshotCase;
use crate::{
    compute_layout, AlignContent, AlignItems, AvailableSpace, BoxSizing, Dimension, Display,
    FlexDirection, FlexWrap, JustifyContent, LayoutTree, LengthPercentage, NodeId, Overflow,
    Position, Size, Style,
};
use super::calc_parser::parse_calc;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Lays out the case and returns each node's rect relative to its parent, in snapshot order.
/// The engine has no computed values to compare (it gets Chrome's), so `@check`s are skipped.
pub fn layout_case(case: &SnapshotCase) -> Result<CaseOutput, String> {
    let children = case.children();
    let mut tree = LayoutTree::new();
    let mut ids = vec![None; case.nodes.len()];
    push_subtree(case, &children, 0, &mut tree, &mut ids)?;
    let ids: Vec<NodeId> = ids.into_iter().map(Option::unwrap).collect();

    let mut node_index = vec![0; ids.len()];
    for (index, id) in ids.iter().enumerate() {
        node_index[id.index()] = index;
    }
    let mut measure = |id: NodeId, input| {
        let node = &case.nodes[node_index[id.index()]];
        measure_text(node.text.as_deref().unwrap_or(""), node.font_size, &node.text_style, input)
    };
    compute_layout(
        &mut tree,
        ids[0],
        Size::new(AvailableSpace::Definite(10_000.0), AvailableSpace::Definite(10_000.0)),
        &mut measure,
    );

    let rects = ids
        .iter()
        .map(|&id| {
            let layout = tree.layout(id);
            [layout.location.x, layout.location.y, layout.size.width, layout.size.height]
        })
        .collect();
    Ok(CaseOutput {
        rects,
        values: None,
        lines: None,
        problems: Vec::new(),
    })
}

fn push_subtree(
    case: &SnapshotCase,
    children: &[Vec<usize>],
    index: usize,
    tree: &mut LayoutTree,
    ids: &mut [Option<NodeId>],
) -> Result<NodeId, String> {
    let child_ids = children[index]
        .iter()
        .map(|&child| push_subtree(case, children, child, tree, ids))
        .collect::<Result<Vec<_>, _>>()?;
    let node = &case.nodes[index];
    let style = style_from_snapshot(&node.style)
        .map_err(|err| format!("{} (node {index}): {err}", node.label))?;
    let id = tree.push(style, &child_ids, node.text.is_some());
    ids[index] = Some(id);
    Ok(id)
}

/// Converts the computed values recorded by bless into a [`Style`].
pub fn style_from_snapshot(values: &BTreeMap<String, String>) -> Result<Style, String> {
    let mut style = Style::default();
    for (property, value) in values {
        let value = value.as_str();
        match property.as_str() {
            "display" => style.display = keyword(value, &[("flex", Display::Flex), ("none", Display::None)])?,
            "position" => {
                style.position = keyword(
                    value,
                    &[("relative", Position::Relative), ("absolute", Position::Absolute)],
                )?
            }
            "box-sizing" => {
                style.box_sizing = keyword(
                    value,
                    &[("content-box", BoxSizing::ContentBox), ("border-box", BoxSizing::BorderBox)],
                )?
            }
            "overflow-x" => style.overflow.x = overflow(value)?,
            "overflow-y" => style.overflow.y = overflow(value)?,
            "width" => style.size.width = dimension(value)?,
            "height" => style.size.height = dimension(value)?,
            "min-width" => style.min_size.width = dimension(value)?,
            "min-height" => style.min_size.height = dimension(value)?,
            "max-width" => style.max_size.width = dimension(value)?,
            "max-height" => style.max_size.height = dimension(value)?,
            "margin-top" => style.margin.top = dimension(value)?,
            "margin-right" => style.margin.right = dimension(value)?,
            "margin-bottom" => style.margin.bottom = dimension(value)?,
            "margin-left" => style.margin.left = dimension(value)?,
            "padding-top" => style.padding.top = length_percentage(value)?,
            "padding-right" => style.padding.right = length_percentage(value)?,
            "padding-bottom" => style.padding.bottom = length_percentage(value)?,
            "padding-left" => style.padding.left = length_percentage(value)?,
            "border-top-width" => style.border.top = px(value)?,
            "border-right-width" => style.border.right = px(value)?,
            "border-bottom-width" => style.border.bottom = px(value)?,
            "border-left-width" => style.border.left = px(value)?,
            "top" => style.inset.top = dimension(value)?,
            "right" => style.inset.right = dimension(value)?,
            "bottom" => style.inset.bottom = dimension(value)?,
            "left" => style.inset.left = dimension(value)?,
            "flex-direction" => {
                style.flex_direction = keyword(
                    value,
                    &[
                        ("row", FlexDirection::Row),
                        ("row-reverse", FlexDirection::RowReverse),
                        ("column", FlexDirection::Column),
                        ("column-reverse", FlexDirection::ColumnReverse),
                    ],
                )?
            }
            "flex-wrap" => {
                style.flex_wrap = keyword(
                    value,
                    &[
                        ("nowrap", FlexWrap::NoWrap),
                        ("wrap", FlexWrap::Wrap),
                        ("wrap-reverse", FlexWrap::WrapReverse),
                    ],
                )?
            }
            "justify-content" => {
                style.justify_content = keyword(
                    value,
                    &[
                        ("normal", JustifyContent::Normal),
                        ("flex-start", JustifyContent::FlexStart),
                        ("flex-end", JustifyContent::FlexEnd),
                        ("start", JustifyContent::Start),
                        ("end", JustifyContent::End),
                        ("center", JustifyContent::Center),
                        ("space-between", JustifyContent::SpaceBetween),
                        ("space-around", JustifyContent::SpaceAround),
                        ("space-evenly", JustifyContent::SpaceEvenly),
                        ("stretch", JustifyContent::Stretch),
                    ],
                )?
            }
            "align-items" => style.align_items = align_items(value)?,
            "align-self" => {
                style.align_self = if value == "auto" {
                    None
                } else {
                    Some(align_items(value)?)
                }
            }
            "align-content" => {
                style.align_content = keyword(
                    value,
                    &[
                        ("normal", AlignContent::Normal),
                        ("stretch", AlignContent::Stretch),
                        ("flex-start", AlignContent::FlexStart),
                        ("flex-end", AlignContent::FlexEnd),
                        ("start", AlignContent::Start),
                        ("end", AlignContent::End),
                        ("center", AlignContent::Center),
                        ("space-between", AlignContent::SpaceBetween),
                        ("space-around", AlignContent::SpaceAround),
                        ("space-evenly", AlignContent::SpaceEvenly),
                    ],
                )?
            }
            "flex-grow" => style.flex_grow = number(value)?,
            "flex-shrink" => style.flex_shrink = number(value)?,
            "flex-basis" => style.flex_basis = dimension(value)?,
            "order" => style.order = number(value)? as i32,
            "row-gap" => style.gap.height = length_percentage(value)?,
            "column-gap" => style.gap.width = length_percentage(value)?,
            // Only affects text measurement.
            "font-size" => {}
            other => return Err(format!("unsupported property `{other}`")),
        }
    }
    Ok(style)
}

fn keyword<T: Copy>(value: &str, options: &[(&str, T)]) -> Result<T, String> {
    options
        .iter()
        .find(|(name, _)| *name == value)
        .map(|(_, parsed)| *parsed)
        .ok_or_else(|| format!("unsupported value `{value}`"))
}

fn overflow(value: &str) -> Result<Overflow, String> {
    keyword(
        value,
        &[
            ("visible", Overflow::Visible),
            ("clip", Overflow::Clip),
            ("hidden", Overflow::Hidden),
            ("scroll", Overflow::Scroll),
            ("auto", Overflow::Auto),
        ],
    )
}

fn align_items(value: &str) -> Result<AlignItems, String> {
    keyword(
        value,
        &[
            ("normal", AlignItems::Normal),
            ("stretch", AlignItems::Stretch),
            ("flex-start", AlignItems::FlexStart),
            ("flex-end", AlignItems::FlexEnd),
            ("start", AlignItems::Start),
            ("end", AlignItems::End),
            ("center", AlignItems::Center),
            ("baseline", AlignItems::Baseline),
        ],
    )
}

fn number(value: &str) -> Result<f64, String> {
    value
        .parse()
        .map_err(|_| format!("expected a number, got `{value}`"))
}

fn px(value: &str) -> Result<f64, String> {
    value
        .strip_suffix("px")
        .ok_or_else(|| format!("expected px, got `{value}`"))
        .and_then(number)
}

fn is_math(value: &str) -> bool {
    ["calc(", "min(", "max(", "clamp("]
        .iter()
        .any(|prefix| value.starts_with(prefix))
}

fn dimension(value: &str) -> Result<Dimension, String> {
    match value {
        "auto" | "none" => Ok(Dimension::Auto),
        _ if is_math(value) => Ok(Dimension::Calc(Arc::new(parse_calc(value)?))),
        _ => match value.strip_suffix('%') {
            Some(percent) => number(percent).map(Dimension::Percent),
            None => px(value).map(Dimension::Px),
        },
    }
}

fn length_percentage(value: &str) -> Result<LengthPercentage, String> {
    match value {
        "normal" => Ok(LengthPercentage::Px(0.0)),
        _ if is_math(value) => Ok(LengthPercentage::Calc(Arc::new(parse_calc(value)?))),
        _ => match value.strip_suffix('%') {
            Some(percent) => number(percent).map(LengthPercentage::Percent),
            None => px(value).map(LengthPercentage::Px),
        },
    }
}
