//! Runs the `sonate_layout` fixtures through Sonate's own HTML loader, CSS parser, cascade and
//! layout, and compares the boxes and `@check`ed computed values with Chrome's snapshots.
//!
//! `FIXTURE_FILTER=nested/ cargo test -p sonate layout_fixtures` runs a subset.

use super::test_html::load_layout_fixture;
use super::{build_render_snapshot, LayoutContext, Node};
use crate::style::{Display, Length, Rgba, Style};
use crate::Id;
use sonate_layout::testing::harness::{self, CaseOutput, Suite};
use sonate_layout::testing::snapshot::SnapshotCase;
use std::collections::BTreeMap;
use std::path::Path;

#[test]
fn layout_fixtures() {
    let known_failures =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/layout/fixture_known_failures.txt");
    let suite = Suite::with_layout_fixtures("sonate", known_failures);
    let filter = std::env::var("FIXTURE_FILTER").ok();

    let engine = |fixture: &harness::Fixture, case: &SnapshotCase| layout_case(&fixture.html, case);
    match harness::run_all(&suite, &engine, filter.as_deref()) {
        Ok(summary) => println!("{summary}"),
        Err(failures) => panic!("{failures}"),
    }
}

fn layout_case(html: &str, case: &SnapshotCase) -> Result<CaseOutput, String> {
    let (mut ctx, ids) = load_layout_fixture(html, &case.overrides);
    if !case.states.is_empty() {
        for (selector, state) in &case.states {
            apply_state(&mut ctx, &ids, selector, state)?;
        }
        ctx.layout();
    }

    let mut rects = Vec::new();
    let mut values = Vec::new();
    for (&id, snapshot_node) in ids.iter().zip(&case.nodes) {
        let node = ctx.document.get_node(id).ok_or("missing fixture node")?;
        let node = node.borrow();
        let parent = node
            .parent
            .and_then(|parent| ctx.document.get_node(parent))
            .map(|parent| parent.borrow().layout.bounds)
            .unwrap_or_default();
        let bounds = node.layout.bounds;
        rects.push([
            bounds.x - parent.x,
            bounds.y - parent.y,
            bounds.width,
            bounds.height,
        ]);
        values.push(
            snapshot_node
                .checks
                .keys()
                .filter_map(|property| {
                    computed_value(&node.layout.style, property).map(|value| (property.clone(), value))
                })
                .collect::<BTreeMap<_, _>>(),
        );
    }
    Ok(CaseOutput {
        rects,
        values: Some(values),
    })
}

/// Puts the element into `state` like bless does in Chrome: hover and active come from a
/// pointer at the element's center, focus from focusing the element.
fn apply_state(ctx: &mut LayoutContext, ids: &[Id], selector: &str, state: &str) -> Result<(), String> {
    let id = ids
        .iter()
        .copied()
        .find(|&id| ctx.document.get_node(id).is_some_and(|node| matches_selector(&node.borrow(), selector)))
        .ok_or_else(|| format!("@state selector {selector} matches nothing"))?;

    if state == "focus" {
        ctx.focus_path(&[id]);
        return Ok(());
    }
    let bounds = ctx.document.get_node(id).unwrap().borrow().layout.bounds;
    let snapshot = build_render_snapshot(ctx.document.root_node());
    let path = snapshot.root.find_element_at_position(
        bounds.x + bounds.width / 2.0,
        bounds.y + bounds.height / 2.0,
    );
    ctx.set_hovered(&path);
    if state == "active" {
        // Pressing the mouse also focuses, like in browsers.
        ctx.set_active(&path);
        ctx.focus_path(&path);
    }
    Ok(())
}

fn matches_selector(node: &Node, selector: &str) -> bool {
    match selector.split_at(1) {
        ("#", id) => node.attributes.get("id").is_some_and(|value| value == id),
        (".", class) => node
            .attributes
            .get("class")
            .is_some_and(|classes| classes.split_whitespace().any(|c| c == class)),
        _ => false,
    }
}

/// The computed value of `property` as `getComputedStyle` would serialize it.
fn computed_value(style: &Style, property: &str) -> Option<String> {
    let black = Rgba {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };
    let color = style.color.unwrap_or(black);
    Some(match property {
        "color" => css_color(color),
        "background-color" => css_color(style.background_color.unwrap_or_default()),
        // Unset border colors are `currentcolor`.
        "border-top-color" => css_color(style.border_color.top.unwrap_or(color)),
        "border-right-color" => css_color(style.border_color.right.unwrap_or(color)),
        "border-bottom-color" => css_color(style.border_color.bottom.unwrap_or(color)),
        "border-left-color" => css_color(style.border_color.left.unwrap_or(color)),
        "font-size" => match style.font_size {
            Some(Length::Px(px)) => format!("{}px", css_number(px)),
            _ => format!("{}px", crate::cascade::DEFAULT_FONT_SIZE_PX),
        },
        name if name.starts_with("--") => style
            .custom_properties
            .as_ref()
            .and_then(|custom| custom.get(name).cloned())
            .unwrap_or_default(),
        "display" => match style.display {
            Some(Display::None) => "none".to_owned(),
            _ => "flex".to_owned(),
        },
        _ => return None,
    })
}

fn css_color(color: Rgba) -> String {
    if color.a == 255 {
        format!("rgb({}, {}, {})", color.r, color.g, color.b)
    } else {
        format!("rgba({}, {}, {}, {})", color.r, color.g, color.b, css_alpha(color.a))
    }
}

/// The shortest decimal that rounds back to the same 8-bit alpha, like Chrome serializes it.
fn css_alpha(alpha: u8) -> String {
    for digits in 1..=4 {
        let text = format!("{:.*}", digits, f64::from(alpha) / 255.0);
        if (text.parse::<f64>().unwrap() * 255.0).round() as u8 == alpha {
            return css_number(text.parse().unwrap());
        }
    }
    css_number(f64::from(alpha) / 255.0)
}

fn css_number(value: f64) -> String {
    let text = format!("{value:.4}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}
