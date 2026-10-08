//! Snapshots written by the bless tool (`tools/bless`): the fixture as Chrome parsed it plus the
//! layout Chrome produced, for every case of the fixture's `@matrix`.

use crate::text::{OverflowWrap, TextAlign, TextStyle, TextTransform, WhiteSpace, WordBreak};
use serde_json::Value;
use std::collections::BTreeMap;

/// Bumped whenever the snapshot format or the meaning of its fields changes.
pub const SCHEMA: u64 = 1;

/// A border-box rect `[x, y, width, height]`, relative to the parent's border box.
pub type Rect4 = [f64; 4];

#[derive(Clone, Debug)]
pub struct Snapshot {
    /// Hash of the fixture, `base.css` and the schema; see [`fixture_hash`].
    pub hash: String,
    /// The Chrome version that produced the snapshot (informational).
    pub chrome: String,
    /// Per-fixture comparison tolerance in px (`<meta name="tolerance">`).
    pub tolerance: Option<f64>,
    pub cases: Vec<SnapshotCase>,
}

#[derive(Clone, Debug)]
pub struct SnapshotCase {
    /// The matrix values of this case, e.g. `#root flex-direction=row`. Empty without a matrix.
    pub params: String,
    /// The matrix values as inline-style overrides, applied in order.
    pub overrides: Vec<Override>,
    /// Interaction states from `@state`: `(selector, "hover" | "active" | "focus")`.
    pub states: Vec<(String, String)>,
    /// Elements in pre-order; the first one is the fixture root.
    pub nodes: Vec<SnapshotNode>,
}

#[derive(Clone, Debug)]
pub struct Override {
    /// `#id` or `.class`.
    pub selector: String,
    pub property: String,
    pub value: String,
}

#[derive(Clone, Debug)]
pub struct SnapshotNode {
    pub depth: usize,
    /// `#id`, `.class` or the tag name, for messages.
    pub label: String,
    /// Computed values that differ from the initial values, e.g. `width: "50px"`, `flex-grow: "1"`.
    pub style: BTreeMap<String, String>,
    /// Text content (whitespace collapsed) when the element's only child is text.
    pub text: Option<String>,
    /// Font size in px.
    pub font_size: f64,
    /// Chrome's border-box rect relative to the parent's border box (zero for `display: none`).
    pub rect: Rect4,
    /// Computed values of the fixture's `@check` properties, as `getComputedStyle` reports them.
    pub checks: BTreeMap<String, String>,
    /// For text: the text properties that affect line breaking and alignment.
    pub text_style: TextStyle,
    /// For text: Chrome's line boxes relative to the element's border box.
    pub lines: Option<Vec<Rect4>>,
}

impl SnapshotCase {
    /// The child indices of every node.
    pub fn children(&self) -> Vec<Vec<usize>> {
        let mut children = vec![Vec::new(); self.nodes.len()];
        let mut stack: Vec<usize> = Vec::new();
        for (index, node) in self.nodes.iter().enumerate() {
            stack.truncate(node.depth);
            if let Some(&parent) = stack.last() {
                children[parent].push(index);
            }
            stack.push(index);
        }
        children
    }
}

/// FNV-1a (64 bit) over the schema, `base.css` and the fixture, with line endings normalized.
/// `tools/bless/bless.ts` computes the same value.
pub fn fixture_hash(html: &str, base_css: &str) -> String {
    let input = format!(
        "schema={SCHEMA}\n{}\n\u{0}\n{}",
        base_css.replace("\r\n", "\n"),
        html.replace("\r\n", "\n")
    );
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in input.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

pub fn parse(json: &str) -> Result<Snapshot, String> {
    let root: Value = serde_json::from_str(json).map_err(|err| format!("invalid JSON: {err}"))?;
    let schema = root["schema"].as_u64().ok_or("missing schema")?;
    if schema != SCHEMA {
        return Err(format!("snapshot schema {schema}, expected {SCHEMA}"));
    }

    let cases = root["cases"]
        .as_array()
        .ok_or("missing cases")?
        .iter()
        .map(parse_case)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Snapshot {
        hash: string(&root["hash"])?,
        chrome: string(&root["chrome"]).unwrap_or_default(),
        tolerance: root["tolerance"].as_f64(),
        cases,
    })
}

fn parse_case(case: &Value) -> Result<SnapshotCase, String> {
    let overrides = case["overrides"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(|entry| {
            Ok(Override {
                selector: string(&entry[0])?,
                property: string(&entry[1])?,
                value: string(&entry[2])?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let states = case["states"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(|entry| Ok((string(&entry[0])?, string(&entry[1])?)))
        .collect::<Result<Vec<_>, String>>()?;

    let nodes = case["nodes"]
        .as_array()
        .ok_or("case without nodes")?
        .iter()
        .map(parse_node)
        .collect::<Result<Vec<_>, _>>()?;
    if nodes.first().map(|node| node.depth) != Some(0) {
        return Err("the first node must be the root".to_owned());
    }

    Ok(SnapshotCase {
        params: string(&case["params"]).unwrap_or_default(),
        overrides,
        states,
        nodes,
    })
}

fn parse_node(node: &Value) -> Result<SnapshotNode, String> {
    let style = node["style"]
        .as_object()
        .map(|style| {
            style
                .iter()
                .map(|(key, value)| {
                    let value = match value {
                        Value::String(value) => value.clone(),
                        Value::Number(value) => value.to_string(),
                        other => return Err(format!("unexpected value for {key}: {other}")),
                    };
                    Ok((key.clone(), value))
                })
                .collect::<Result<BTreeMap<_, _>, String>>()
        })
        .transpose()?
        .unwrap_or_default();

    let parsed_rect = rect4(&node["rect"])?;

    let checks = node["checks"]
        .as_object()
        .map(|checks| {
            checks
                .iter()
                .map(|(key, value)| Ok((key.clone(), string(value)?)))
                .collect::<Result<BTreeMap<_, _>, String>>()
        })
        .transpose()?
        .unwrap_or_default();

    let lines = node["lines"]
        .as_array()
        .map(|lines| lines.iter().map(rect4).collect::<Result<Vec<_>, String>>())
        .transpose()?;

    Ok(SnapshotNode {
        checks,
        text_style: text_style(&node["ts"])?,
        lines,
        depth: node["d"].as_u64().ok_or("node without depth")? as usize,
        label: string(&node["label"]).unwrap_or_else(|_| "div".to_owned()),
        style,
        text: node["text"].as_str().map(str::to_owned),
        font_size: node["font"].as_f64().unwrap_or(10.0),
        rect: parsed_rect,
    })
}

fn string(value: &Value) -> Result<String, String> {
    value
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("expected a string, got {value}"))
}

fn rect4(value: &Value) -> Result<Rect4, String> {
    let values = value.as_array().ok_or("expected a rect")?;
    if values.len() != 4 {
        return Err("rect must have 4 values".to_owned());
    }
    let mut rect = [0.0; 4];
    for (target, value) in rect.iter_mut().zip(values) {
        *target = value.as_f64().ok_or("rect values must be numbers")?;
    }
    Ok(rect)
}

/// The text properties recorded as `ts`, with Chrome's computed values.
fn text_style(value: &Value) -> Result<TextStyle, String> {
    let mut style = TextStyle::default();
    let Some(values) = value.as_object() else {
        return Ok(style);
    };
    let px = |value: &str| {
        value
            .strip_suffix("px")
            .and_then(|number| number.parse::<f64>().ok())
            .ok_or_else(|| format!("expected px, got `{value}`"))
    };
    for (property, value) in values {
        let value = value.as_str().ok_or("text style values must be strings")?;
        let unsupported = || format!("unsupported {property}: {value}");
        match property.as_str() {
            "white-space" => {
                style.white_space = match value {
                    "normal" => WhiteSpace::Normal,
                    "nowrap" => WhiteSpace::NoWrap,
                    "pre" => WhiteSpace::Pre,
                    "pre-wrap" => WhiteSpace::PreWrap,
                    "pre-line" => WhiteSpace::PreLine,
                    "break-spaces" => WhiteSpace::BreakSpaces,
                    _ => return Err(unsupported()),
                }
            }
            "text-transform" => {
                style.text_transform = match value {
                    "none" => TextTransform::None,
                    "uppercase" => TextTransform::Uppercase,
                    "lowercase" => TextTransform::Lowercase,
                    "capitalize" => TextTransform::Capitalize,
                    _ => return Err(unsupported()),
                }
            }
            "overflow-wrap" => {
                style.overflow_wrap = match value {
                    "normal" => OverflowWrap::Normal,
                    "break-word" => OverflowWrap::BreakWord,
                    "anywhere" => OverflowWrap::Anywhere,
                    _ => return Err(unsupported()),
                }
            }
            "word-break" => {
                style.word_break = match value {
                    "normal" => WordBreak::Normal,
                    "break-all" => WordBreak::BreakAll,
                    "keep-all" => WordBreak::KeepAll,
                    _ => return Err(unsupported()),
                }
            }
            "text-align" => {
                style.text_align = match value {
                    "start" => TextAlign::Start,
                    "end" => TextAlign::End,
                    "left" => TextAlign::Left,
                    "right" => TextAlign::Right,
                    "center" => TextAlign::Center,
                    "justify" => TextAlign::Justify,
                    _ => return Err(unsupported()),
                }
            }
            "letter-spacing" if value == "normal" => {}
            "letter-spacing" => style.letter_spacing = px(value)?,
            "word-spacing" => style.word_spacing = px(value)?,
            "line-height" if value == "normal" => {}
            "line-height" => style.line_height = Some(px(value)?),
            _ => return Err(format!("unsupported text property {property}")),
        }
    }
    Ok(style)
}