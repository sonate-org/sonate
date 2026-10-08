use crate::{
    layout::{Rect as LayoutRect, RenderOp, RenderSnapshot},
    scrollbar::{collect_scrollbar_thumbs, ScrollbarAxis, THUMB_CORNER_RADIUS},
    style::{BorderStyle, Length, Overflow, Rgba, TextOverflow},
    text::{text_style, FontSpec, SkiaTextMeasurer, TextMeasurer},
};
use sonate_layout::text as text_layout;
use skia_safe::{Canvas, ClipOp, Color, Color4f, Paint, Path, RRect, Rect};

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
                    let clip_rect = Rect::new(
                        rect.x as f32,
                        rect.y as f32,
                        (rect.x + rect.width) as f32,
                        (rect.y + rect.height) as f32,
                    );
                    self.canvas.clip_rect(clip_rect, ClipOp::Intersect, true);
                }
                RenderOp::PopClip => {
                    self.canvas.restore();
                }
            }
        }
    }

    fn paint_background_and_border(&mut self, node: &crate::layout::RenderNode) {
        let style = &node.style;

        let client_rect = Rect::new(
            node.bounds.x as f32,
            node.bounds.y as f32,
            (node.bounds.x + node.bounds.width) as f32,
            (node.bounds.y + node.bounds.height) as f32,
        );

        let client_rrect = if style.border_radius.is_empty() {
            RRect::new_rect_xy(client_rect, 0.0, 0.0)
        } else {
            let tl = style
                .border_radius
                .top_left
                .as_ref()
                .map(|r| (r.x.to_px() as f32, r.y.to_px() as f32));
            let tr = style
                .border_radius
                .top_right
                .as_ref()
                .map(|r| (r.x.to_px() as f32, r.y.to_px() as f32));
            let br = style
                .border_radius
                .bottom_right
                .as_ref()
                .map(|r| (r.x.to_px() as f32, r.y.to_px() as f32));
            let bl = style
                .border_radius
                .bottom_left
                .as_ref()
                .map(|r| (r.x.to_px() as f32, r.y.to_px() as f32));

            RRect::new_rect_radii(
                client_rect,
                &[
                    skia_safe::Vector::new(
                        tl.map(|v| v.0).unwrap_or(0.0),
                        tl.map(|v| v.1).unwrap_or(0.0),
                    ),
                    skia_safe::Vector::new(
                        tr.map(|v| v.0).unwrap_or(0.0),
                        tr.map(|v| v.1).unwrap_or(0.0),
                    ),
                    skia_safe::Vector::new(
                        br.map(|v| v.0).unwrap_or(0.0),
                        br.map(|v| v.1).unwrap_or(0.0),
                    ),
                    skia_safe::Vector::new(
                        bl.map(|v| v.0).unwrap_or(0.0),
                        bl.map(|v| v.1).unwrap_or(0.0),
                    ),
                ],
            )
        };

        if let Some(background_color) = &style.background_color {
            let paint = Paint::new(background_color.to_color4f(), None);

            self.canvas.draw_rrect(client_rrect, &paint);
        }

        let border_is_hidden = matches!(
            style.border_style.top,
            Some(BorderStyle::None) | Some(BorderStyle::Hidden)
        );

        if !border_is_hidden {
            let border_width = style.border_width.resolved();
            let stroke_width_px = border_width
                .top
                .to_px()
                .max(border_width.right.to_px())
                .max(border_width.bottom.to_px())
                .max(border_width.left.to_px());

            if stroke_width_px > 0.0 {
                let color = style.border_color.top.unwrap_or(Rgba {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 255,
                });

                let mut paint = Paint::new(color.to_color4f(), None);
                paint.set_style(skia_safe::paint::Style::Stroke);
                paint.set_stroke_width(stroke_width_px as f32);
                paint.set_anti_alias(true);
                self.canvas.draw_rrect(client_rrect, &paint);
            }
        }
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
        let lines = text_layout::break_lines(text, &text_style, text_box.width.max(min_content), &mut advance);

        // `text-overflow: ellipsis` shortens lines that overflow a clipping element. (Unlike in
        // Chrome, it applies to the element's own text, since that is always in a flex item.)
        let ellipsis = style.text_overflow == Some(TextOverflow::Ellipsis)
            && matches!(
                style.overflow,
                Some(Overflow::Hidden | Overflow::Scroll | Overflow::Auto)
            );

        for (index, line) in lines.iter().enumerate() {
            let (offset, width) = text_layout::align_line(line, text_style.text_align, text_box.width);
            let y = (text_box.y + index as f64 * line_height + baseline) as f32;
            let mut x = text_box.x + offset;

            let mut content = line.text.clone();
            if ellipsis && line.width > content_box.x + content_box.width - x + 0.01 {
                let available = content_box.x + content_box.width - x - advance("…");
                while !content.is_empty() && measure_spaced(&content, &text_style, &mut advance) > available {
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
            if text_style.letter_spacing == 0.0 && text_style.word_spacing == 0.0 && justify_extra == 0.0 {
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
        let border = style.border_width.resolved();
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
                self.paint_lines(text, &font, &font_spec, &measurer, style, text_box, content_box, &paint);
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
fn measure_spaced(text: &str, style: &text_layout::TextStyle, advance: &mut dyn FnMut(&str) -> f64) -> f64 {
    let characters = text.chars().count() as f64;
    let spaces = text.chars().filter(|&c| c == ' ').count() as f64;
    advance(text) + style.letter_spacing * characters + style.word_spacing * spaces
}
