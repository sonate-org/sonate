use crate::{
    layout::{Rect as LayoutRect, RenderOp, RenderSnapshot},
    scrollbar::{collect_scrollbar_thumbs, ScrollbarAxis, THUMB_CORNER_RADIUS},
    style::{BorderStyle, BoxShadow, Length, Overflow, Rgba, TextOverflow},
    text::{text_style, FontSpec, SkiaTextMeasurer, TextMeasurer},
};
use skia_safe::rrect::Corner;
use skia_safe::{
    BlurStyle, Canvas, ClipOp, Color, Color4f, MaskFilter, Paint, Path, PathEffect, RRect, Rect,
};
use sonate_layout::text as text_layout;

pub struct Painter<'a> {
    canvas: &'a Canvas,
}

fn debug_bounds_enabled() -> bool {
    std::env::var("SONATE_DEBUG_BOUNDS")
        .ok()
        .is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
}

impl<'a> Painter<'a> {
    pub fn new(canvas: &'a Canvas) -> Self {
        Self { canvas }
    }

    pub fn paint(&mut self, snapshot: &RenderSnapshot) {
        self.canvas.clear(Color::WHITE);
        self.paint_ops(&snapshot.render_list);
        self.paint_scrollbars(snapshot);
        if debug_bounds_enabled() {
            self.paint_debug_bounds(&snapshot.root, 0);
        }
    }

    fn paint_debug_bounds(&mut self, node: &crate::layout::RenderNode, depth: usize) {
        let mut paint = Paint::new(Color4f::new(1.0, 0.0, 0.0, 0.95), None);
        paint.set_style(skia_safe::paint::Style::Stroke);
        paint.set_stroke_width((1.0 + depth as f32 * 0.1).min(2.5));
        paint.set_anti_alias(true);

        let rect = Rect::new(
            node.bounds.x as f32,
            node.bounds.y as f32,
            (node.bounds.x + node.bounds.width) as f32,
            (node.bounds.y + node.bounds.height) as f32,
        );
        self.canvas.draw_rect(rect, &paint);

        for child in &node.children {
            self.paint_debug_bounds(child, depth + 1);
        }
    }

    fn paint_scrollbars(&mut self, snapshot: &RenderSnapshot) {
        let thumbs = collect_scrollbar_thumbs(&snapshot.root);
        if thumbs.is_empty() {
            return;
        }

        let mut paint = Paint::new(Color4f::new(0.20, 0.20, 0.20, 0.55), None);
        paint.set_anti_alias(true);

        for thumb in thumbs {
            let has_clip = thumb.visible_clip.is_some();
            if let Some(clip) = thumb.visible_clip {
                self.canvas.save();
                self.canvas
                    .clip_rect(Self::to_skia_rect(clip), ClipOp::Intersect, true);
            }

            let rect = Rect::new(
                thumb.rect.x as f32,
                thumb.rect.y as f32,
                (thumb.rect.x + thumb.rect.width) as f32,
                (thumb.rect.y + thumb.rect.height) as f32,
            );
            let (rx, ry) = match thumb.axis {
                ScrollbarAxis::Vertical => (THUMB_CORNER_RADIUS, THUMB_CORNER_RADIUS),
                ScrollbarAxis::Horizontal => (THUMB_CORNER_RADIUS, THUMB_CORNER_RADIUS),
            };
            let rrect = RRect::new_rect_xy(rect, rx, ry);
            self.canvas.draw_rrect(rrect, &paint);

            if has_clip {
                self.canvas.restore();
            }
        }
    }

    fn to_skia_rect(rect: LayoutRect) -> Rect {
        Rect::new(
            rect.x as f32,
            rect.y as f32,
            (rect.x + rect.width) as f32,
            (rect.y + rect.height) as f32,
        )
    }

    fn paint_ops(&mut self, ops: &[RenderOp]) {
        for op in ops {
            match op {
                RenderOp::DrawBackgroundBorder(node) => self.paint_background_and_border(node),
                RenderOp::DrawText(node) => self.paint_text(node),
                RenderOp::PushClip(rect) => {
                    self.canvas.save();
                    self.canvas
                        .clip_rect(pixel_snapped(*rect), ClipOp::Intersect, true);
                }
                RenderOp::PushRoundedClip { bounds, style } => {
                    self.canvas.save();
                    let padding_box = inset_rrect(
                        &rounded_rect(*bounds, &style.border_radius),
                        border_widths(style),
                    );
                    self.canvas.clip_rrect(padding_box, ClipOp::Intersect, true);
                }
                RenderOp::PopClip | RenderOp::PopTransform | RenderOp::PopLayer => {
                    self.canvas.restore();
                }
                RenderOp::PushTransform([a, b, c, d, e, f]) => {
                    self.canvas.save();
                    let matrix = skia_safe::Matrix::new_all(
                        *a as f32, *c as f32, *e as f32, *b as f32, *d as f32, *f as f32, 0.0, 0.0,
                        1.0,
                    );
                    self.canvas.concat(&matrix);
                }
                RenderOp::PushLayer(opacity) => {
                    let alpha = (opacity.clamp(0.0, 1.0) * 255.0).round() as u32;
                    self.canvas.save_layer_alpha(None, alpha);
                }
            }
        }
    }

    fn paint_background_and_border(&mut self, node: &crate::layout::RenderNode) {
        let style = &node.style;
        let outer = rounded_rect(node.bounds, &style.border_radius);
        let widths = border_widths(style);
        let inner = inset_rrect(&outer, widths);
        let current_color = style.color.unwrap_or(BLACK);

        let shadows = style
            .box_shadow
            .as_deref()
            .map(Vec::as_slice)
            .unwrap_or_default();
        // Like browsers, an opaque background lets the shadow reach a pixel under the box, so
        // no gap shows along its anti-aliased edge.
        let opaque_background = style.background_color.is_some_and(|color| color.a == 255);
        let shadow_clip = if opaque_background {
            inset_rrect(&outer, [1.0; 4])
        } else {
            outer
        };
        // The first shadow is on top, so paint from the last.
        for shadow in shadows.iter().rev().filter(|shadow| !shadow.inset) {
            self.paint_outer_shadow(&outer, &shadow_clip, shadow, current_color);
        }

        if let Some(background_color) = &style.background_color {
            let mut paint = Paint::new(background_color.to_color4f(), None);
            paint.set_anti_alias(true);
            // Under an opaque border the background stops halfway into it, so it doesn't bleed
            // through the border's anti-aliased outer edge.
            let area = if border_hides_edge(style, widths, current_color) {
                inset_rrect(&outer, widths.map(|width| width / 2.0))
            } else {
                outer
            };
            self.canvas.draw_rrect(area, &paint);
        }

        for shadow in shadows.iter().rev().filter(|shadow| shadow.inset) {
            self.paint_inset_shadow(&inner, shadow, current_color);
        }

        self.paint_borders(style, &outer, &inner, widths, current_color);
    }

    /// An outer shadow: the border box shape, offset, grown by the spread and blurred, but only
    /// outside `clip`.
    fn paint_outer_shadow(
        &mut self,
        border_box: &RRect,
        clip: &RRect,
        shadow: &BoxShadow,
        current_color: Rgba,
    ) {
        let spread = shadow.spread.to_px() as f32;
        let mut shape = *border_box;
        shape.offset((
            shadow.offset_x.to_px() as f32,
            shadow.offset_y.to_px() as f32,
        ));
        let shape = outset_rrect(&shape, spread);

        self.canvas.save();
        self.canvas.clip_rrect(*clip, ClipOp::Difference, true);
        let mut paint = Paint::new(shadow.color.unwrap_or(current_color).to_color4f(), None);
        paint.set_anti_alias(true);
        let blur = shadow.blur.to_px() as f32;
        if blur > 0.0 {
            paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, blur / 2.0, false));
        }
        self.canvas.draw_rrect(shape, &paint);
        self.canvas.restore();
    }

    /// An inset shadow: inside the padding box, everything outside the padding box shape
    /// offset and shrunk by the spread, blurred.
    fn paint_inset_shadow(&mut self, padding_box: &RRect, shadow: &BoxShadow, current_color: Rgba) {
        let spread = shadow.spread.to_px() as f32;
        let mut hole = *padding_box;
        hole.offset((
            shadow.offset_x.to_px() as f32,
            shadow.offset_y.to_px() as f32,
        ));
        let hole = outset_rrect(&hole, -spread);
        let blur = shadow.blur.to_px() as f32;
        let margin = blur * 2.0
            + spread.abs()
            + shadow.offset_x.to_px().abs() as f32
            + shadow.offset_y.to_px().abs() as f32
            + 1.0;
        let surround = RRect::new_rect(padding_box.rect().with_outset((margin, margin)));

        self.canvas.save();
        self.canvas
            .clip_rrect(*padding_box, ClipOp::Intersect, true);
        let mut paint = Paint::new(shadow.color.unwrap_or(current_color).to_color4f(), None);
        paint.set_anti_alias(true);
        if blur > 0.0 {
            paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, blur / 2.0, false));
        }
        self.canvas.draw_drrect(surround, hole, &paint);
        self.canvas.restore();
    }

    /// Borders with their own width, style and color per side. Sides meet along the line
    /// from the outer to the inner corner.
    fn paint_borders(
        &mut self,
        style: &crate::style::Style,
        outer: &RRect,
        inner: &RRect,
        widths: [f32; 4],
        current_color: Rgba,
    ) {
        let colors = [
            style.border_color.top,
            style.border_color.right,
            style.border_color.bottom,
            style.border_color.left,
        ]
        .map(|color| color.unwrap_or(current_color));
        let styles = [
            style.border_style.top,
            style.border_style.right,
            style.border_style.bottom,
            style.border_style.left,
        ]
        .map(|style| style.unwrap_or(BorderStyle::None));
        if widths.iter().all(|&width| width <= 0.0) {
            return;
        }

        // The sides meet along the lines from the outer corners to the inner corners. A rounded
        // inner corner moves the meeting point inwards to the chord of its curve, so a side
        // next to one without width covers the whole corner.
        let o = *outer.rect();
        let i = *inner.rect();
        let corner =
            |outer_point: (f32, f32), inner_point: (f32, f32), radius: Corner, sign: (f32, f32)| {
                let skia_safe::Vector { x: rx, y: ry } = inner.radii(radius);
                if rx <= 0.0 || ry <= 0.0 {
                    return inner_point;
                }
                let chord_start = (inner_point.0 + sign.0 * rx, inner_point.1);
                let chord_end = (inner_point.0, inner_point.1 + sign.1 * ry);
                intersection(outer_point, inner_point, chord_start, chord_end)
                    .unwrap_or(inner_point)
            };
        let top_left = corner(
            (o.left, o.top),
            (i.left, i.top),
            Corner::UpperLeft,
            (1.0, 1.0),
        );
        let top_right = corner(
            (o.right, o.top),
            (i.right, i.top),
            Corner::UpperRight,
            (-1.0, 1.0),
        );
        let bottom_right = corner(
            (o.right, o.bottom),
            (i.right, i.bottom),
            Corner::LowerRight,
            (-1.0, -1.0),
        );
        let bottom_left = corner(
            (o.left, o.bottom),
            (i.left, i.bottom),
            Corner::LowerLeft,
            (1.0, -1.0),
        );
        let regions = [
            [(o.left, o.top), (o.right, o.top), top_right, top_left],
            [
                (o.right, o.top),
                (o.right, o.bottom),
                bottom_right,
                top_right,
            ],
            [
                (o.right, o.bottom),
                (o.left, o.bottom),
                bottom_left,
                bottom_right,
            ],
            [(o.left, o.bottom), (o.left, o.top), top_left, bottom_left],
        ];

        // Each side's bands with their colors.
        let bands: [Vec<(Band, Rgba)>; 4] = std::array::from_fn(|side| {
            let top_or_left = side == 0 || side == 3;
            let color = colors[side];
            match styles[side] {
                _ if widths[side] <= 0.0 => Vec::new(),
                BorderStyle::None
                | BorderStyle::Hidden
                | BorderStyle::Dashed
                | BorderStyle::Dotted => Vec::new(),
                BorderStyle::Solid => vec![(Band::Whole, color)],
                BorderStyle::Double => vec![(Band::DoubleOuter, color), (Band::DoubleInner, color)],
                BorderStyle::Inset => vec![(Band::Whole, shade_3d(color, top_or_left))],
                BorderStyle::Outset => vec![(Band::Whole, shade_3d(color, !top_or_left))],
                // An inset outer half and an outset inner half, or the other way round.
                BorderStyle::Groove => vec![
                    (Band::OuterHalf, shade_3d(color, top_or_left)),
                    (Band::InnerHalf, shade_3d(color, !top_or_left)),
                ],
                BorderStyle::Ridge => vec![
                    (Band::OuterHalf, shade_3d(color, !top_or_left)),
                    (Band::InnerHalf, shade_3d(color, top_or_left)),
                ],
            }
        });

        let mut painted = [false; 4];
        for side in 0..4 {
            if painted[side] || widths[side] <= 0.0 {
                continue;
            }
            let dashed = matches!(styles[side], BorderStyle::Dashed | BorderStyle::Dotted);
            // Sides that look the same are painted together, so they join without seams.
            let group: Vec<usize> = if dashed {
                vec![side]
            } else {
                (side..4)
                    .filter(|&other| {
                        !painted[other] && widths[other] > 0.0 && bands[other] == bands[side]
                    })
                    .collect()
            };
            for &member in &group {
                painted[member] = true;
            }
            if !dashed && bands[side].is_empty() {
                continue;
            }
            self.canvas.save();
            if group.len() < 4 {
                self.canvas.clip_rrect(*outer, ClipOp::Intersect, true);
                self.canvas.clip_rrect(*inner, ClipOp::Difference, true);
                let mut region = Path::new();
                for &member in &group {
                    // Reaching a pixel into a neighbor painted later that hides what's below
                    // makes its anti-aliased join blend with this side instead of what's
                    // behind the border.
                    let overlaps = |neighbor: usize| {
                        !dashed
                            && !painted[neighbor]
                            && widths[neighbor] > 0.0
                            && hides_below(&bands[neighbor])
                    };
                    let points = side_region(
                        regions[member],
                        member,
                        overlaps((member + 3) % 4),
                        overlaps((member + 1) % 4),
                    );
                    region.add_poly(&points, true);
                }
                self.canvas.clip_path(&region, ClipOp::Intersect, true);
            }
            if dashed {
                self.paint_dashes(side, styles[side], colors[side], widths[side], o);
            }
            for &(band, color) in &bands[side] {
                let mut paint = Paint::new(color.to_color4f(), None);
                paint.set_anti_alias(true);
                let outer_edge = inset_rrect(outer, widths.map(|width| band.edges(width).0));
                let inner_edge = inset_rrect(outer, widths.map(|width| band.edges(width).1));
                self.canvas.draw_drrect(outer_edge, inner_edge, &paint);
            }
            self.canvas.restore();
        }
    }

    /// A dashed or dotted side, already clipped to its region. Like Blink, the pattern is
    /// stretched so the side starts and ends with a dash or dot; thin dots are squares.
    fn paint_dashes(
        &mut self,
        side: usize,
        style: BorderStyle,
        color: Rgba,
        width: f32,
        outer: Rect,
    ) {
        let mut paint = Paint::new(color.to_color4f(), None);
        paint.set_anti_alias(true);
        // The side's center line along its whole outer length.
        let half = width / 2.0;
        let (start, end) = match side {
            0 => (
                (outer.left, outer.top + half),
                (outer.right, outer.top + half),
            ),
            1 => (
                (outer.right - half, outer.top),
                (outer.right - half, outer.bottom),
            ),
            2 => (
                (outer.left, outer.bottom - half),
                (outer.right, outer.bottom - half),
            ),
            _ => (
                (outer.left + half, outer.top),
                (outer.left + half, outer.bottom),
            ),
        };
        let length = (end.0 - start.0).abs() + (end.1 - start.1).abs();
        let horizontal = matches!(side, 0 | 2);
        // Moves a point along the line.
        let along = |(x, y): (f32, f32), distance: f32| {
            if horizontal {
                (x + distance, y)
            } else {
                (x, y + distance)
            }
        };
        paint.set_style(skia_safe::paint::Style::Stroke);
        paint.set_stroke_width(width);

        let (mut from, mut to) = (start, end);
        if style == BorderStyle::Dashed || width <= 3.0 {
            let (dash, gap) = match style {
                BorderStyle::Dashed if width >= 3.0 => (width * 2.0, width),
                BorderStyle::Dashed => (width * 3.0, width * 2.0),
                _ => (width, width),
            };
            if length > dash * 2.0 {
                let gap = best_dash_gap(length, dash, gap);
                paint.set_path_effect(PathEffect::dash(&[dash, gap], 0.0));
            }
        } else {
            // Round dots: zero-length dashes with round caps, which reach half a width beyond
            // their centers.
            paint.set_stroke_cap(skia_safe::paint::Cap::Round);
            from = along(start, half);
            to = along(end, -half);
            let intervals = if length < width * 2.0 {
                [0.0, width * 2.0]
            } else {
                [0.0, best_dash_gap(length, width, width) + width - 0.01]
            };
            paint.set_path_effect(PathEffect::dash(&intervals, 0.0));
        }
        let mut line = Path::new();
        line.move_to(from);
        line.line_to(to);
        self.canvas.draw_path(&line, &paint);
    }

    /// Draws wrapped text line by line, the way the layout measured it.
    #[allow(clippy::too_many_arguments)]
    fn paint_lines(
        &mut self,
        text: &str,
        font: &skia_safe::Font,
        font_spec: &FontSpec,
        measurer: &SkiaTextMeasurer,
        style: &crate::style::Style,
        text_box: LayoutRect,
        content_box: LayoutRect,
        paint: &Paint,
    ) {
        let text_style = text_style(style);
        let metrics = measurer.metrics(font_spec);
        let line_height = metrics.line_height(&text_style);
        let baseline = metrics.baseline(&text_style);
        let mut advance = |run: &str| measurer.advance(run, font_spec);
        let min_content = text_layout::min_content_width(text, &text_style, &mut advance);
        let lines = text_layout::break_lines(
            text,
            &text_style,
            text_box.width.max(min_content),
            &mut advance,
        );

        // `text-overflow: ellipsis` shortens lines that overflow a clipping element. (Unlike in
        // Chrome, it applies to the element's own text, since that is always in a flex item.)
        let ellipsis = style.text_overflow == Some(TextOverflow::Ellipsis)
            && matches!(
                style.overflow,
                Some(Overflow::Hidden | Overflow::Scroll | Overflow::Auto)
            );

        for (index, line) in lines.iter().enumerate() {
            let (offset, width) =
                text_layout::align_line(line, text_style.text_align, text_box.width);
            let y = (text_box.y + index as f64 * line_height + baseline) as f32;
            let mut x = text_box.x + offset;

            let mut content = line.text.clone();
            if ellipsis && line.width > content_box.x + content_box.width - x + 0.01 {
                let available = content_box.x + content_box.width - x - advance("…");
                while !content.is_empty()
                    && measure_spaced(&content, &text_style, &mut advance) > available
                {
                    content.pop();
                }
                content.push('…');
            }

            // Justified lines spread their free space over the spaces.
            let spaces = content.chars().filter(|&c| c == ' ').count();
            let justify_extra = if width > line.width && spaces > 0 {
                (width - line.width) / spaces as f64
            } else {
                0.0
            };
            if text_style.letter_spacing == 0.0
                && text_style.word_spacing == 0.0
                && justify_extra == 0.0
            {
                self.canvas.draw_str(&content, (x as f32, y), font, paint);
                continue;
            }
            for c in content.chars() {
                let c = c.to_string();
                self.canvas.draw_str(&c, (x as f32, y), font, paint);
                x += advance(&c) + text_style.letter_spacing;
                if c == " " {
                    x += text_style.word_spacing + justify_extra;
                }
            }
        }
    }

    fn paint_text(&mut self, node: &crate::layout::RenderNode) {
        let style = &node.style;

        let font_spec = FontSpec::from_style(style);
        let font = SkiaTextMeasurer::make_font(&font_spec);
        let (_scale, metrics) = font.metrics();
        let measurer = SkiaTextMeasurer::new();
        let text_style = text_style(style);
        let font_metrics = measurer.metrics(&font_spec);

        // The text goes into the box the layout placed it in, else the content box.
        let padding = style.padding.resolved();
        let border = style.used_border_width();
        let content_box = LayoutRect {
            x: node.bounds.x + padding.left.to_px() + border.left.to_px(),
            y: node.bounds.y + padding.top.to_px() + border.top.to_px(),
            width: node.bounds.width
                - padding.left.to_px()
                - padding.right.to_px()
                - border.left.to_px()
                - border.right.to_px(),
            height: 0.0,
        };
        let text_box = node.text_box.unwrap_or(content_box);
        let x = text_box.x as f32;
        let baseline_y = (text_box.y + font_metrics.baseline(&text_style)) as f32;
        let is_text_input = style.widget.is_some_and(|widget| widget.is_text_input());

        if let Some(text) = &node.text {
            let text_color = style.color.unwrap_or(Rgba {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            });

            let mut paint = Paint::new(text_color.to_color4f(), None);
            paint.set_anti_alias(true);

            if is_text_input {
                self.canvas.draw_str(text, (x, baseline_y), &font, &paint);
            } else {
                self.paint_lines(
                    text,
                    &font,
                    &font_spec,
                    &measurer,
                    style,
                    text_box,
                    content_box,
                    &paint,
                );
            }
        }

        if is_text_input {
            if let Some(state) = &node.text_input_state {
                if state.focused {
                    let text = node.text.as_deref().unwrap_or("");
                    let caret_prefix: String = text.chars().take(state.caret).collect();
                    let (caret_advance, _) = font.measure_str(caret_prefix.as_str(), None);

                    let caret_x = x + caret_advance;
                    let caret_top = (baseline_y as f64 + metrics.ascent as f64) as f32;
                    let caret_bottom = (baseline_y as f64 + metrics.descent as f64) as f32;

                    let mut caret_path = Path::new();
                    caret_path.move_to((caret_x, caret_top));
                    caret_path.line_to((caret_x, caret_bottom));

                    let mut caret_paint = Paint::new(
                        style
                            .color
                            .unwrap_or(Rgba {
                                r: 0,
                                g: 0,
                                b: 0,
                                a: 255,
                            })
                            .to_color4f(),
                        None,
                    );
                    caret_paint.set_style(skia_safe::paint::Style::Stroke);
                    caret_paint.set_stroke_width(1.0);
                    caret_paint.set_anti_alias(true);
                    self.canvas.draw_path(&caret_path, &caret_paint);
                }
            }
        }
    }
}

// Helper method to convert Length to pixels
#[allow(unused)]
trait ToPx {
    fn to_px(&self) -> f64;
}

impl ToPx for Length {
    fn to_px(&self) -> f64 {
        match self {
            Length::Px(value) => *value,
            _ => 0.0, // Handle other cases as needed
        }
    }
}

pub(crate) trait ToColor4f {
    fn to_color4f(&self) -> Color4f;
}

impl ToColor4f for Rgba {
    fn to_color4f(&self) -> Color4f {
        let color = Color::from_argb(self.a, self.r, self.g, self.b);
        Color4f::new(
            color.r() as f32 / 255.0,
            color.g() as f32 / 255.0,
            color.b() as f32 / 255.0,
            color.a() as f32 / 255.0,
        )
    }
}

/// The width of a run including letter and word spacing.
fn measure_spaced(
    text: &str,
    style: &text_layout::TextStyle,
    advance: &mut dyn FnMut(&str) -> f64,
) -> f64 {
    let characters = text.chars().count() as f64;
    let spaces = text.chars().filter(|&c| c == ' ').count() as f64;
    advance(text) + style.letter_spacing * characters + style.word_spacing * spaces
}

const BLACK: Rgba = Rgba {
    r: 0,
    g: 0,
    b: 0,
    a: 255,
};

/// Whether the border covers the edge of the border box everywhere with opaque color.
fn border_hides_edge(style: &crate::style::Style, widths: [f32; 4], current_color: Rgba) -> bool {
    let sides = [
        (style.border_style.top, style.border_color.top),
        (style.border_style.right, style.border_color.right),
        (style.border_style.bottom, style.border_color.bottom),
        (style.border_style.left, style.border_color.left),
    ];
    sides
        .iter()
        .zip(widths)
        .all(|(&(border_style, color), width)| {
            width >= 1.0
                && color.unwrap_or(current_color).a == 255
                && matches!(
                    border_style,
                    Some(
                        BorderStyle::Solid
                            | BorderStyle::Groove
                            | BorderStyle::Ridge
                            | BorderStyle::Inset
                            | BorderStyle::Outset
                    )
                )
        })
}

/// The region of a side, from its corners `[outer start, outer end, inner end, inner start]`
/// (clockwise). The joins with the previous and next side can move a pixel into those sides.
fn side_region(
    [outer_start, outer_end, inner_end, inner_start]: [(f32, f32); 4],
    side: usize,
    into_previous: bool,
    into_next: bool,
) -> Vec<skia_safe::Point> {
    // Along the side's inward normal, which points into both neighbors near the corners.
    let (nx, ny) = [(0.0, 1.0), (-1.0, 0.0), (0.0, -1.0), (1.0, 0.0)][side];
    let shift = |(x, y): (f32, f32), shifted: bool| if shifted { (x + nx, y + ny) } else { (x, y) };
    let mut points = vec![outer_start, outer_end];
    if into_next {
        points.push(shift(outer_end, true));
    }
    points.push(shift(inner_end, into_next));
    points.push(shift(inner_start, into_previous));
    if into_previous {
        points.push(shift(outer_start, true));
    }
    points.into_iter().map(skia_safe::Point::from).collect()
}

/// Whether the bands of a side cover it completely with opaque color.
fn hides_below(bands: &[(Band, Rgba)]) -> bool {
    !bands.is_empty()
        && bands.iter().all(|&(band, color)| {
            color.a == 255 && !matches!(band, Band::DoubleOuter | Band::DoubleInner)
        })
}

/// A band of a border side, along its whole length.
#[derive(Clone, Copy, PartialEq)]
enum Band {
    Whole,
    DoubleOuter,
    DoubleInner,
    OuterHalf,
    InnerHalf,
}

impl Band {
    /// Where the band starts and ends, measured from the outer edge of a side of `width`.
    /// Browsers round the stripes of `double`, `groove` and `ridge` borders to whole pixels.
    fn edges(self, width: f32) -> (f32, f32) {
        match self {
            Band::Whole => (0.0, width),
            Band::DoubleOuter => (0.0, ((width + 1.0) / 3.0).floor()),
            Band::DoubleInner => (((width * 2.0 + 1.0) / 3.0).floor(), width),
            Band::OuterHalf => (0.0, (width / 2.0).floor()),
            Band::InnerHalf => ((width / 2.0).floor(), width),
        }
    }
}

/// The rect with its edges rounded to whole pixels, which is how browsers paint boxes laid out
/// at fractional positions.
fn pixel_snapped(rect: LayoutRect) -> Rect {
    let round = |value: f64| (value + 0.5).floor() as f32;
    Rect::new(
        round(rect.x),
        round(rect.y),
        round(rect.x + rect.width),
        round(rect.y + rect.height),
    )
}

/// The color of a side of an `inset`, `outset`, `groove` or `ridge` border, like Blink's
/// `CalculateBorderStyleColor`: the shadowed sides are darker and the lit sides lighter. A color
/// close to black is lightened for both, and one close to white stays the same on the lit sides.
fn shade_3d(color: Rgba, shadowed: bool) -> Rgba {
    let distance = |gray: i32| {
        [color.r, color.g, color.b]
            .iter()
            .map(|&channel| (i32::from(channel) - gray).pow(2))
            .sum::<i32>()
    };
    if distance(0) <= 3 * 0x20 * 0x20 {
        let shadow = lighten(color);
        return if shadowed { shadow } else { lighten(shadow) };
    }
    if shadowed {
        darken(color)
    } else if distance(255) > 3 * (255 - 0xeb) * (255 - 0xeb) {
        lighten(color)
    } else {
        color
    }
}

/// Blink's `Color::Dark()`: scales the color so its largest channel drops by a third.
fn darken(color: Rgba) -> Rgba {
    scale_color(color, |max| {
        if max == 0.0 {
            0.0
        } else {
            ((max - 0.33) / max).max(0.0)
        }
    })
}

/// Blink's `Color::Light()`: scales the color so its largest channel grows by a third.
fn lighten(color: Rgba) -> Rgba {
    if color.r == 0 && color.g == 0 && color.b == 0 {
        return Rgba {
            r: 0x54,
            g: 0x54,
            b: 0x54,
            a: color.a,
        };
    }
    scale_color(color, |max| (max + 0.33).min(1.0) / max)
}

fn scale_color(color: Rgba, multiplier: impl Fn(f32) -> f32) -> Rgba {
    // Blink scales by the largest value just below 256.
    const SCALE: f32 = 255.999_98;
    let [r, g, b] = [color.r, color.g, color.b].map(|channel| f32::from(channel) / 255.0);
    let multiplier = multiplier(r.max(g).max(b));
    let channel = |value: f32| (multiplier * value * SCALE) as u8;
    Rgba {
        r: channel(r),
        g: channel(g),
        b: channel(b),
        a: color.a,
    }
}

/// The gap that fits a whole number of dashes into `length`, starting and ending with a dash,
/// closest to `gap` (Blink's `StrokeData::SelectBestDashGap`).
fn best_dash_gap(length: f32, dash: f32, gap: f32) -> f32 {
    let fewer = ((length + gap) / (dash + gap)).floor();
    let more = fewer + 1.0;
    let fewer_gap = (length - fewer * dash) / (fewer - 1.0);
    let more_gap = (length - more * dash) / (more - 1.0);
    if more_gap <= 0.0 || (fewer_gap - gap).abs() < (more_gap - gap).abs() {
        fewer_gap
    } else {
        more_gap
    }
}

/// Where the line through `a1` and `a2` meets the line through `b1` and `b2`.
fn intersection(
    a1: (f32, f32),
    a2: (f32, f32),
    b1: (f32, f32),
    b2: (f32, f32),
) -> Option<(f32, f32)> {
    let (ax, ay) = (a2.0 - a1.0, a2.1 - a1.1);
    let (bx, by) = (b2.0 - b1.0, b2.1 - b1.1);
    let denominator = ax * by - ay * bx;
    if denominator.abs() < 1e-6 {
        return None;
    }
    let t = ((b1.0 - a1.0) * by - (b1.1 - a1.1) * bx) / denominator;
    Some((a1.0 + t * ax, a1.1 + t * ay))
}

/// Border widths (top, right, bottom, left).
fn border_widths(style: &crate::style::Style) -> [f32; 4] {
    let border = style.used_border_width();
    [border.top, border.right, border.bottom, border.left].map(|width| width.to_px() as f32)
}

/// The border box with its `border-radius` (Skia scales overlapping radii down like CSS).
fn rounded_rect(bounds: LayoutRect, radius: &crate::style::BorderRadius) -> RRect {
    let rect = pixel_snapped(bounds);
    let corner = |corner: &Option<crate::style::Radius>| {
        corner
            .as_ref()
            .map_or(skia_safe::Point::new(0.0, 0.0), |radius| {
                let resolve = |length: &Length, basis: f32| match length {
                    Length::Percent(percent) => basis * *percent as f32 / 100.0,
                    other => other.to_px() as f32,
                };
                skia_safe::Point::new(
                    resolve(&radius.x, rect.width()),
                    resolve(&radius.y, rect.height()),
                )
            })
    };
    RRect::new_rect_radii(
        rect,
        &[
            corner(&radius.top_left),
            corner(&radius.top_right),
            corner(&radius.bottom_right),
            corner(&radius.bottom_left),
        ],
    )
}

/// The rounded rect inside borders of the given widths: the corners shrink by the widths.
fn inset_rrect(outer: &RRect, [top, right, bottom, left]: [f32; 4]) -> RRect {
    let o = *outer.rect();
    let rect = Rect::new(
        o.left + left,
        o.top + top,
        (o.right - right).max(o.left + left),
        (o.bottom - bottom).max(o.top + top),
    );
    let shrink = |corner: Corner, dx: f32, dy: f32| {
        let radius = outer.radii(corner);
        skia_safe::Point::new((radius.x - dx).max(0.0), (radius.y - dy).max(0.0))
    };
    RRect::new_rect_radii(
        rect,
        &[
            shrink(Corner::UpperLeft, left, top),
            shrink(Corner::UpperRight, right, top),
            shrink(Corner::LowerRight, right, bottom),
            shrink(Corner::LowerLeft, left, bottom),
        ],
    )
}

/// Grows (or shrinks, for a negative amount) a rounded rect; rounded corners keep their
/// shape, square corners stay square, as for `box-shadow` spread.
fn outset_rrect(rrect: &RRect, amount: f32) -> RRect {
    let rect = rrect.rect().with_outset((amount, amount));
    let rect = Rect::new(
        rect.left,
        rect.top,
        rect.right.max(rect.left),
        rect.bottom.max(rect.top),
    );
    let grow = |corner: skia_safe::rrect::Corner| {
        let radius = rrect.radii(corner);
        let grow = |value: f32| {
            if value > 0.0 {
                (value + amount).max(0.0)
            } else {
                0.0
            }
        };
        skia_safe::Point::new(grow(radius.x), grow(radius.y))
    };
    RRect::new_rect_radii(
        rect,
        &[
            grow(Corner::UpperLeft),
            grow(Corner::UpperRight),
            grow(Corner::LowerRight),
            grow(Corner::LowerLeft),
        ],
    )
}
