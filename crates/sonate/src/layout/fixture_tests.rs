//! Runs the `sonate_layout` fixtures through Sonate's own HTML loader, CSS parser, cascade and
//! layout, and compares the boxes and `@check`ed computed values with Chrome's snapshots.
//!
//! Fixtures with `@screenshot` are also painted and compared with Chrome's screenshots; failing
//! cases leave the painted image and a diff in `target/paint-fixtures`.
//!
//! `FIXTURE_FILTER=nested/ cargo test -p sonate layout_fixtures` runs a subset.

use super::test_html::load_layout_fixture;
use super::{build_render_snapshot, LayoutContext, Node};
use crate::painter::Painter;
use crate::style::{
    Display, FontStyle, FontWeight, Length, OverflowWrap, Rgba, Style, WhiteSpace, WordBreak,
};
use crate::Id;
use skia_safe::image::CachingHint;
use skia_safe::{AlphaType, ColorType, Data, Image, ImageInfo};
use sonate_layout::testing::harness::{self, CaseOutput, Suite};
use sonate_layout::testing::snapshot::SnapshotCase;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// How far a channel may lie outside the range of Chrome's pixels around it.
const CHANNEL_TOLERANCE: u8 = 24;
/// The share of pixels that may differ, in percent, unless the fixture sets `@pixels`.
const DEFAULT_PIXEL_TOLERANCE: f64 = 0.2;

#[test]
fn layout_fixtures() {
    let known_failures =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/layout/fixture_known_failures.txt");
    let suite = Suite::with_layout_fixtures("sonate", known_failures);
    let filter = std::env::var("FIXTURE_FILTER").ok();

    let engine = |fixture: &harness::Fixture, case: &SnapshotCase| layout_case(fixture, case);
    match harness::run_all(&suite, &engine, filter.as_deref()) {
        Ok(summary) => println!("{summary}"),
        Err(failures) => panic!("{failures}"),
    }
}

fn layout_case(fixture: &harness::Fixture, case: &SnapshotCase) -> Result<CaseOutput, String> {
    let (mut ctx, ids) = load_layout_fixture(&fixture.html, &case.overrides);
    if !case.states.is_empty() {
        for (selector, state) in &case.states {
            apply_state(&mut ctx, &ids, selector, state)?;
        }
        ctx.layout();
    }

    let mut rects = Vec::new();
    let mut values = Vec::new();
    let mut lines = Vec::new();
    for (&id, snapshot_node) in ids.iter().zip(&case.nodes) {
        let node = ctx.document.get_node(id).ok_or("missing fixture node")?;
        let node = node.borrow();
        let parent = node
            .parent
            .and_then(|parent| ctx.document.get_node(parent))
            .map(|parent| parent.borrow().layout.bounds)
            .unwrap_or_default();
        let bounds = node.layout.bounds;
        // Line boxes relative to the border box, computed like the painter places the text.
        lines.push(match (&node.text, node.layout.text_box) {
            (Some(text), Some(text_box)) if node.children.is_empty() => crate::text::line_rects(
                &*ctx.text_measurer,
                text,
                &node.layout.style,
                text_box.width,
            )
            .into_iter()
            .map(|[x, y, width, height]| {
                [
                    text_box.x - bounds.x + x,
                    text_box.y - bounds.y + y,
                    width,
                    height,
                ]
            })
            .collect(),
            _ => Vec::new(),
        });
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
                    computed_value(&node.layout.style, bounds, property)
                        .map(|value| (property.clone(), value))
                })
                .collect::<BTreeMap<_, _>>(),
        );
    }
    let mut problems = Vec::new();
    if let Some(screenshot) = &case.screenshot {
        let pixels = fixture
            .snapshot
            .as_ref()
            .ok()
            .and_then(|snapshot| snapshot.pixels);
        problems.extend(compare_screenshot(&ctx, fixture, screenshot, pixels)?);
    }
    Ok(CaseOutput {
        rects,
        values: Some(values),
        lines: Some(lines),
        problems,
    })
}

/// Paints the fixture and compares it with Chrome's screenshot of the root's border box.
fn compare_screenshot(
    ctx: &LayoutContext,
    fixture: &harness::Fixture,
    screenshot: &str,
    tolerance: Option<f64>,
) -> Result<Option<String>, String> {
    let path = fixture.path.with_file_name(screenshot);
    let bytes =
        std::fs::read(&path).map_err(|err| format!("can't read {}: {err}", path.display()))?;
    let expected = Image::from_encoded(Data::new_copy(&bytes))
        .ok_or_else(|| format!("can't decode {}", path.display()))?;
    let (width, height) = (expected.width(), expected.height());

    let mut surface = skia_safe::surfaces::raster_n32_premul((width, height))
        .ok_or("can't create a raster surface")?;
    Painter::new(surface.canvas()).paint(&build_render_snapshot(ctx.document.root_node()));
    let actual = surface.image_snapshot();

    let (expected_pixels, actual_pixels) = (rgba(&expected)?, rgba(&actual)?);
    let (columns, rows) = (width as usize, height as usize);
    fn pixel(pixels: &[u8], columns: usize, x: usize, y: usize) -> &[u8] {
        let start = (y * columns + x) * 4;
        &pixels[start..start + 4]
    }
    // A pixel only differs when one of its channels lies outside the range of Chrome's pixels
    // around it, so edges that are anti-aliased a little differently don't count.
    let mut wrong = vec![false; columns * rows];
    for y in 0..rows {
        for x in 0..columns {
            let (mut low, mut high) = ([u8::MAX; 4], [u8::MIN; 4]);
            for ny in y.saturating_sub(1)..(y + 2).min(rows) {
                for nx in x.saturating_sub(1)..(x + 2).min(columns) {
                    for (channel, &value) in
                        pixel(&expected_pixels, columns, nx, ny).iter().enumerate()
                    {
                        low[channel] = low[channel].min(value);
                        high[channel] = high[channel].max(value);
                    }
                }
            }
            wrong[y * columns + x] =
                pixel(&actual_pixels, columns, x, y)
                    .iter()
                    .enumerate()
                    .any(|(channel, &value)| {
                        value < low[channel].saturating_sub(CHANNEL_TOLERANCE)
                            || value > high[channel].saturating_add(CHANNEL_TOLERANCE)
                    });
        }
    }
    let wrong_count = wrong.iter().filter(|&&wrong| wrong).count();
    let share = 100.0 * wrong_count as f64 / (columns * rows).max(1) as f64;
    if share <= tolerance.unwrap_or(DEFAULT_PIXEL_TOLERANCE) {
        return Ok(None);
    }

    // Keep the painted image and a diff (wrong pixels red over a faded screenshot) for inspection.
    let out_dir = paint_output_dir();
    let stem = screenshot.trim_end_matches(".png");
    let mut diff = Vec::with_capacity(expected_pixels.len());
    for (index, expected) in expected_pixels.chunks(4).enumerate() {
        if wrong[index] {
            diff.extend_from_slice(&[255, 0, 0, 255]);
        } else {
            diff.extend(
                expected[..3]
                    .iter()
                    .map(|&channel| 255 - (255 - channel) / 4),
            );
            diff.push(255);
        }
    }
    let info = ImageInfo::new(
        (width, height),
        ColorType::RGBA8888,
        AlphaType::Unpremul,
        None,
    );
    let diff = skia_safe::images::raster_from_data(&info, Data::new_copy(&diff), columns * 4)
        .ok_or("can't create the diff image")?;
    let written = std::fs::create_dir_all(&out_dir).is_ok()
        && [(&actual, "actual"), (&diff, "diff")]
            .iter()
            .all(|(image, kind)| {
                skia_safe::png_encoder::encode_image(None, image, &Default::default()).is_some_and(
                    |png| {
                        std::fs::write(out_dir.join(format!("{stem}.{kind}.png")), png.as_bytes())
                            .is_ok()
                    },
                )
            });
    let location = if written {
        format!(
            "; see {}",
            out_dir.join(format!("{stem}.diff.png")).display()
        )
    } else {
        String::new()
    };
    Ok(Some(format!(
        "{share:.2}% of the pixels differ from {screenshot}{location}"
    )))
}

/// The image's pixels as unpremultiplied RGBA.
fn rgba(image: &Image) -> Result<Vec<u8>, String> {
    let info = ImageInfo::new(
        image.dimensions(),
        ColorType::RGBA8888,
        AlphaType::Unpremul,
        None,
    );
    let mut pixels = vec![0u8; image.width() as usize * image.height() as usize * 4];
    if image.read_pixels(
        &info,
        &mut pixels,
        image.width() as usize * 4,
        (0, 0),
        CachingHint::Allow,
    ) {
        Ok(pixels)
    } else {
        Err("can't read the pixels of an image".to_owned())
    }
}

fn paint_output_dir() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            manifest
                .ancestors()
                .nth(2)
                .unwrap_or(manifest)
                .join("target")
        })
        .join("paint-fixtures")
}

/// Puts the element into `state` like bless does in Chrome: hover and active come from a
/// pointer at the element's center, focus from focusing the element.
fn apply_state(
    ctx: &mut LayoutContext,
    ids: &[Id],
    selector: &str,
    state: &str,
) -> Result<(), String> {
    let id = ids
        .iter()
        .copied()
        .find(|&id| {
            ctx.document
                .get_node(id)
                .is_some_and(|node| matches_selector(&node.borrow(), selector))
        })
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

/// The computed value of `property` as `getComputedStyle` would serialize it, for an element
/// with the border box `bounds`.
fn computed_value(style: &Style, bounds: super::Rect, property: &str) -> Option<String> {
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
        "font-family" => style.font_family.clone().unwrap_or_default(),
        "font-weight" => match style.font_weight {
            Some(FontWeight::Number(weight)) => weight.to_string(),
            _ => "400".to_owned(),
        },
        "font-style" => match style.font_style.unwrap_or_default() {
            FontStyle::Normal => "normal",
            FontStyle::Italic => "italic",
            FontStyle::Oblique => "oblique",
        }
        .to_owned(),
        "line-height" => match crate::text::text_style(style).line_height {
            Some(px) => format!("{}px", css_number(px)),
            None => "normal".to_owned(),
        },
        "letter-spacing" => match &style.letter_spacing {
            None | Some(Length::Auto) => "normal".to_owned(),
            Some(length) => format!("{}px", css_number(length.to_px())),
        },
        "word-spacing" => format!(
            "{}px",
            css_number(style.word_spacing.as_ref().map_or(0.0, Length::to_px))
        ),
        "text-align" => format!("{:?}", style.text_align.unwrap_or_default()).to_lowercase(),
        "white-space" => match style.white_space.unwrap_or_default() {
            WhiteSpace::Normal => "normal",
            WhiteSpace::NoWrap => "nowrap",
            WhiteSpace::Pre => "pre",
            WhiteSpace::PreWrap => "pre-wrap",
            WhiteSpace::PreLine => "pre-line",
            WhiteSpace::BreakSpaces => "break-spaces",
        }
        .to_owned(),
        "text-transform" => {
            format!("{:?}", style.text_transform.unwrap_or_default()).to_lowercase()
        }
        "overflow-wrap" => match style.overflow_wrap.unwrap_or_default() {
            OverflowWrap::Normal => "normal",
            OverflowWrap::BreakWord => "break-word",
            OverflowWrap::Anywhere => "anywhere",
        }
        .to_owned(),
        "word-break" => match style.word_break.unwrap_or_default() {
            WordBreak::Normal => "normal",
            WordBreak::BreakAll => "break-all",
            WordBreak::KeepAll => "keep-all",
        }
        .to_owned(),
        "text-overflow" => format!("{:?}", style.text_overflow.unwrap_or_default()).to_lowercase(),
        "opacity" => css_number(style.opacity.unwrap_or(1.0)),
        "transform" => match crate::transform::transform_matrix(style, bounds.width, bounds.height)
        {
            Some(matrix) => format!("matrix({})", matrix.map(chrome_number).join(", ")),
            None => "none".to_owned(),
        },
        "transform-origin" => {
            let (x, y) = crate::transform::transform_origin(style, bounds.width, bounds.height);
            format!("{}px {}px", chrome_number(x), chrome_number(y))
        }
        "box-shadow" => match style.box_shadow.as_deref().map(Vec::as_slice) {
            None | Some([]) => "none".to_owned(),
            Some(shadows) => shadows
                .iter()
                .map(|shadow| {
                    let lengths = [
                        &shadow.offset_x,
                        &shadow.offset_y,
                        &shadow.blur,
                        &shadow.spread,
                    ]
                    .map(|length| format!("{}px", chrome_number(length.to_px())));
                    let inset = if shadow.inset { " inset" } else { "" };
                    format!(
                        "{} {}{inset}",
                        css_color(shadow.color.unwrap_or(color)),
                        lengths.join(" ")
                    )
                })
                .collect::<Vec<_>>()
                .join(", "),
        },
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
        format!(
            "rgba({}, {}, {}, {})",
            color.r,
            color.g,
            color.b,
            css_alpha(color.a)
        )
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

/// A number with six significant digits, like Chrome serializes them.
fn chrome_number(value: f64) -> String {
    if value == 0.0 {
        return "0".to_owned();
    }
    let decimals = (5 - value.abs().log10().floor() as i32).max(0) as usize;
    let text = format!("{value:.decimals$}");
    let text = if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.')
    } else {
        &text
    };
    if text == "-0" {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

fn css_number(value: f64) -> String {
    let text = format!("{value:.4}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}
