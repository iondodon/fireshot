use eframe::egui;
use image::{Rgba, RgbaImage};

use crate::text::{circlecount_text_scale, draw_text_bitmap, text_bitmap_size};
use crate::shapes::{CircleCountShape, ToolIcon};

pub(crate) const CIRCLECOUNT_PADDING: f32 = 2.0;
const CIRCLECOUNT_THICKNESS_OFFSET: f32 = 15.0;

pub(crate) fn draw_line(
    img: &mut RgbaImage,
    start: egui::Pos2,
    end: egui::Pos2,
    color: egui::Color32,
    size: f32,
) {
    let rgba = color32_to_rgba(color);
    let (w, h) = (img.width() as i32, img.height() as i32);
    let radius = (size.max(1.0) / 2.0).ceil() as i32;
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let steps = dx.abs().max(dy.abs()).max(1.0) as i32;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = (start.x + dx * t).round() as i32;
        let y = (start.y + dy * t).round() as i32;
        for ox in -radius..=radius {
            for oy in -radius..=radius {
                let px = x + ox;
                let py = y + oy;
                if px >= 0 && py >= 0 && px < w && py < h {
                    img.put_pixel(px as u32, py as u32, rgba);
                }
            }
        }
    }
}

pub(crate) fn draw_arrow_head(
    painter: &egui::Painter,
    start: egui::Pos2,
    end: egui::Pos2,
    size: f32,
    color: egui::Color32,
) {
    let (_base, left, right) = arrow_head_points(start, end, size);
    let tip = end;
    painter.add(egui::Shape::convex_polygon(
        vec![tip, left, right],
        color,
        egui::Stroke::new(0.0, color),
    ));
}

pub(crate) fn draw_arrow_head_image(
    img: &mut RgbaImage,
    start: egui::Pos2,
    end: egui::Pos2,
    color: egui::Color32,
    size: f32,
) {
    let (_base, left, right) = arrow_head_points(start, end, size);
    let tip = end;
    fill_triangle(img, tip, left, right, color32_to_rgba(color));
}

pub(crate) fn arrow_head_points(
    start: egui::Pos2,
    end: egui::Pos2,
    size: f32,
) -> (egui::Pos2, egui::Pos2, egui::Pos2) {
    let dir = end - start;
    let len = dir.length().max(1.0);
    let dir = dir / len;
    let perp = egui::vec2(-dir.y, dir.x);
    let head_len = (size * 4.0).max(10.0).min(len * 0.8);
    let head_w = (size * 3.0).max(6.0).min(len * 0.6);
    let base = end - dir * head_len;
    let left = base + perp * head_w * 0.5;
    let right = base - perp * head_w * 0.5;
    (base, left, right)
}

pub(crate) fn circlecount_bubble_size(size: f32) -> f32 {
    size + CIRCLECOUNT_THICKNESS_OFFSET
}

pub(crate) fn circlecount_contrast_colors(color: egui::Color32) -> (egui::Color32, egui::Color32) {
    if color_is_dark(color) {
        (egui::Color32::WHITE, egui::Color32::BLACK)
    } else {
        (egui::Color32::BLACK, egui::Color32::WHITE)
    }
}

pub(crate) fn with_alpha(color: egui::Color32, alpha: u8) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

fn color_is_dark(color: egui::Color32) -> bool {
    let r = color.r() as f32;
    let g = color.g() as f32;
    let b = color.b() as f32;
    (0.2126 * r + 0.7152 * g + 0.0722 * b) < 128.0
}

pub(crate) fn draw_ellipse(
    img: &mut RgbaImage,
    start: egui::Pos2,
    end: egui::Pos2,
    color: egui::Color32,
    size: f32,
) {
    let rect = crate::geometry::normalize_rect(egui::Rect::from_two_pos(start, end));
    let points = ellipse_points(rect, 80);
    for win in points.windows(2) {
        draw_line(img, win[0], win[1], color, size);
    }
}

pub(crate) fn ellipse_points(rect: egui::Rect, steps: usize) -> Vec<egui::Pos2> {
    let cx = (rect.min.x + rect.max.x) * 0.5;
    let cy = (rect.min.y + rect.max.y) * 0.5;
    let rx = (rect.max.x - rect.min.x).abs() * 0.5;
    let ry = (rect.max.y - rect.min.y).abs() * 0.5;
    let mut points = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let t = (i as f32 / steps as f32) * std::f32::consts::TAU;
        points.push(egui::pos2(cx + rx * t.cos(), cy + ry * t.sin()));
    }
    points
}

pub(crate) fn draw_circle_count_preview<F: Fn(egui::Pos2) -> egui::Pos2>(
    painter: &egui::Painter,
    to_screen: &F,
    counter: &CircleCountShape,
    scale: f32,
) {
    let bubble_size = circlecount_bubble_size(counter.size);
    let (contrast, anti) = circlecount_contrast_colors(counter.color);
    let center = counter.center;
    let pointer = counter.pointer;
    let dir = pointer - center;
    let len = dir.length();
    if len > bubble_size {
        let dir = dir / len;
        let perp = egui::vec2(-dir.y, dir.x);
        let p1 = center + perp * bubble_size;
        let p2 = center - perp * bubble_size;
        painter.add(egui::Shape::convex_polygon(
            vec![to_screen(center), to_screen(p1), to_screen(pointer), to_screen(p2)],
            counter.color,
            egui::Stroke::new(0.0, counter.color),
        ));
    }

    let center_screen = to_screen(center);
    let outer_radius = (bubble_size + CIRCLECOUNT_PADDING) / scale;
    let inner_radius = bubble_size / scale;
    painter.circle_filled(center_screen, outer_radius, anti);
    painter.circle_stroke(
        center_screen,
        outer_radius,
        egui::Stroke::new(1.0, contrast),
    );
    painter.circle_filled(center_screen, inner_radius, counter.color);

    let text = counter.count.to_string();
    let max_width = inner_radius * 2.0;
    let mut font_size = (inner_radius * 1.1).max(8.0);
    loop {
        let est_width = font_size * 0.6 * text.len().max(1) as f32;
        if est_width <= max_width || font_size <= 6.0 {
            break;
        }
        font_size -= 1.0;
    }
    painter.text(
        center_screen,
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(font_size),
        contrast,
    );
}

pub(crate) fn draw_circle_count_image(img: &mut RgbaImage, counter: &CircleCountShape) {
    let bubble_size = circlecount_bubble_size(counter.size);
    let (contrast, anti) = circlecount_contrast_colors(counter.color);
    let center = counter.center;
    let pointer = counter.pointer;
    let dir = pointer - center;
    let len = dir.length();
    if len > bubble_size {
        let dir = dir / len;
        let perp = egui::vec2(-dir.y, dir.x);
        let p1 = center + perp * bubble_size;
        let p2 = center - perp * bubble_size;
        fill_quad(img, center, p1, pointer, p2, color32_to_rgba(counter.color));
    }

    let outer_radius = bubble_size + CIRCLECOUNT_PADDING;
    draw_filled_circle(img, center, outer_radius, anti);
    let outline_start = egui::pos2(center.x - outer_radius, center.y - outer_radius);
    let outline_end = egui::pos2(center.x + outer_radius, center.y + outer_radius);
    draw_ellipse(img, outline_start, outline_end, contrast, 1.0);
    draw_filled_circle(img, center, bubble_size, counter.color);

    let text = counter.count.to_string();
    let scale = circlecount_text_scale(bubble_size, &text);
    let (text_w, text_h) = text_bitmap_size(&text, scale);
    let pos = egui::pos2(
        center.x - text_w as f32 / 2.0,
        center.y - text_h as f32 / 2.0,
    );
    draw_text_bitmap(img, pos, &text, contrast, scale);
}

fn draw_filled_circle(
    img: &mut RgbaImage,
    center: egui::Pos2,
    radius: f32,
    color: egui::Color32,
) {
    let rgba = color32_to_rgba(color);
    let min_x = (center.x - radius).floor().max(0.0) as i32;
    let max_x = (center.x + radius).ceil().min(img.width() as f32) as i32;
    let min_y = (center.y - radius).floor().max(0.0) as i32;
    let max_y = (center.y + radius).ceil().min(img.height() as f32) as i32;
    let r2 = radius * radius;
    for y in min_y..max_y {
        for x in min_x..max_x {
            let dx = x as f32 + 0.5 - center.x;
            let dy = y as f32 + 0.5 - center.y;
            if dx * dx + dy * dy <= r2 {
                img.put_pixel(x as u32, y as u32, rgba);
            }
        }
    }
}

fn fill_triangle(img: &mut RgbaImage, a: egui::Pos2, b: egui::Pos2, c: egui::Pos2, color: Rgba<u8>) {
    let min_x = a.x.min(b.x).min(c.x).floor().max(0.0) as i32;
    let max_x = a.x.max(b.x).max(c.x).ceil().min(img.width() as f32) as i32;
    let min_y = a.y.min(b.y).min(c.y).floor().max(0.0) as i32;
    let max_y = a.y.max(b.y).max(c.y).ceil().min(img.height() as f32) as i32;

    let area = edge_function(a, b, c).abs();
    if area == 0.0 {
        return;
    }

    for y in min_y..max_y {
        for x in min_x..max_x {
            let p = egui::pos2(x as f32 + 0.5, y as f32 + 0.5);
            let w0 = edge_function(b, c, p);
            let w1 = edge_function(c, a, p);
            let w2 = edge_function(a, b, p);
            let has_pos = w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0;
            let has_neg = w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0;
            if has_pos || has_neg {
                img.put_pixel(x as u32, y as u32, color);
            }
        }
    }
}

fn fill_quad(
    img: &mut RgbaImage,
    a: egui::Pos2,
    b: egui::Pos2,
    c: egui::Pos2,
    d: egui::Pos2,
    color: Rgba<u8>,
) {
    fill_triangle(img, a, b, c, color);
    fill_triangle(img, a, c, d, color);
}

fn edge_function(a: egui::Pos2, b: egui::Pos2, c: egui::Pos2) -> f32 {
    (c.x - a.x) * (b.y - a.y) - (c.y - a.y) * (b.x - a.x)
}

pub(crate) fn draw_handles(painter: &egui::Painter, rect: egui::Rect, radius: f32, color: egui::Color32) {
    let corners = [
        rect.min,
        egui::pos2(rect.max.x, rect.min.y),
        egui::pos2(rect.min.x, rect.max.y),
        rect.max,
    ];
    for corner in corners {
        painter.circle_filled(corner, radius, color);
    }
}

pub(crate) fn draw_selection_hud(
    painter: &egui::Painter,
    sel_rect_screen: egui::Rect,
    sel_rect_image: egui::Rect,
    image_rect: egui::Rect,
) {
    let width = sel_rect_image.width().round().max(0.0) as i32;
    let height = sel_rect_image.height().round().max(0.0) as i32;
    let x = sel_rect_image.min.x.round() as i32;
    let y = sel_rect_image.min.y.round() as i32;
    let label = format!("{}x{}  {},{}", width, height, x, y);

    let font_id = egui::FontId::proportional(12.0);
    let text_color = egui::Color32::WHITE;
    let padding = egui::vec2(6.0, 3.0);
    let text_size = painter
        .layout_no_wrap(label.clone(), font_id.clone(), text_color)
        .size();
    let mut hud_rect = egui::Rect::from_min_size(
        sel_rect_screen.min + egui::vec2(6.0, 6.0),
        text_size + padding * 2.0,
    );

    if hud_rect.max.x > image_rect.max.x {
        hud_rect = hud_rect.translate(egui::vec2(image_rect.max.x - hud_rect.max.x, 0.0));
    }
    if hud_rect.max.y > image_rect.max.y {
        hud_rect = hud_rect.translate(egui::vec2(0.0, image_rect.max.y - hud_rect.max.y));
    }
    if hud_rect.min.x < image_rect.min.x {
        hud_rect = hud_rect.translate(egui::vec2(image_rect.min.x - hud_rect.min.x, 0.0));
    }
    if hud_rect.min.y < image_rect.min.y {
        hud_rect = hud_rect.translate(egui::vec2(0.0, image_rect.min.y - hud_rect.min.y));
    }

    painter.rect_filled(
        hud_rect,
        3.0,
        egui::Color32::from_rgba_premultiplied(0, 0, 0, 190),
    );
    painter.text(
        hud_rect.min + padding,
        egui::Align2::LEFT_TOP,
        label,
        font_id,
        text_color,
    );
}

pub(crate) fn paint_tool_icon(painter: &egui::Painter, rect: egui::Rect, icon: ToolIcon, color: egui::Color32) {
    let stroke = egui::Stroke::new(1.5, color);
    let pad = rect.width().min(rect.height()) * 0.28;
    let inner = rect.shrink(pad);
    match icon {
        ToolIcon::Select => {
            painter.rect_stroke(inner, 2.0, stroke);
            let handle = (rect.width().min(rect.height()) * 0.08).max(1.0);
            let corners = [
                inner.min,
                egui::pos2(inner.max.x, inner.min.y),
                egui::pos2(inner.min.x, inner.max.y),
                inner.max,
            ];
            for corner in corners {
                let handle_rect =
                    egui::Rect::from_center_size(corner, egui::vec2(handle, handle));
                painter.rect_filled(handle_rect, 1.0, color);
            }
        }
        ToolIcon::Pencil => {
            let a = egui::pos2(inner.min.x, inner.max.y);
            let b = egui::pos2(inner.max.x, inner.min.y);
            painter.line_segment([a, b], stroke);
            let tip = egui::pos2(b.x - 2.0, b.y + 2.0);
            painter.line_segment([b, tip], stroke);
        }
        ToolIcon::Line => {
            let a = egui::pos2(inner.min.x, inner.max.y);
            let b = egui::pos2(inner.max.x, inner.min.y);
            painter.line_segment([a, b], stroke);
            painter.circle_filled(a, 2.0, color);
            painter.circle_filled(b, 2.0, color);
        }
        ToolIcon::Arrow => {
            let a = egui::pos2(inner.min.x, inner.max.y);
            let b = egui::pos2(inner.max.x, inner.min.y);
            painter.line_segment([a, b], stroke);
            draw_arrow_head(painter, a, b, 2.5, color);
        }
        ToolIcon::Rect => {
            painter.rect_stroke(inner, 2.0, stroke);
        }
        ToolIcon::Circle => {
            painter.circle_stroke(inner.center(), inner.width().min(inner.height()) * 0.5, stroke);
        }
        ToolIcon::Marker => {
            let a = egui::pos2(inner.min.x, inner.max.y - 2.0);
            let b = egui::pos2(inner.max.x, inner.min.y + 2.0);
            painter.line_segment([a, b], egui::Stroke::new(3.5, color));
        }
        ToolIcon::MarkerLine => {
            let a = egui::pos2(inner.min.x, inner.max.y - 2.0);
            let b = egui::pos2(inner.max.x, inner.min.y + 2.0);
            painter.line_segment([a, b], egui::Stroke::new(3.5, color));
            painter.circle_filled(a, 2.0, color);
            painter.circle_filled(b, 2.0, color);
        }
        ToolIcon::CircleCount => {
            painter.circle_stroke(inner.center(), inner.width().min(inner.height()) * 0.42, stroke);
            painter.text(
                inner.center(),
                egui::Align2::CENTER_CENTER,
                "1",
                egui::FontId::proportional(12.0),
                color,
            );
        }
        ToolIcon::Text => {
            painter.text(
                inner.center(),
                egui::Align2::CENTER_CENTER,
                "T",
                egui::FontId::proportional(14.0),
                color,
            );
        }
        ToolIcon::Pixelate => {
            let size = (inner.width().min(inner.height()) * 0.3).max(2.0);
            let step = size + 2.0;
            let mut y = inner.min.y;
            while y + size <= inner.max.y {
                let mut x = inner.min.x;
                while x + size <= inner.max.x {
                    let rect = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(size, size));
                    painter.rect_filled(rect, 1.0, color);
                    x += step;
                }
                y += step;
            }
        }
        ToolIcon::Blur => {
            painter.circle_filled(inner.center(), inner.width().min(inner.height()) * 0.22, color);
            painter.circle_filled(
                inner.center() + egui::vec2(4.0, -3.0),
                inner.width().min(inner.height()) * 0.16,
                color,
            );
        }
        ToolIcon::Undo => {
            let mid = rect.center();
            let left = egui::pos2(inner.min.x, mid.y);
            let right = egui::pos2(inner.max.x, mid.y);
            painter.line_segment([right, left], stroke);
            painter.line_segment([left, egui::pos2(left.x + 4.0, left.y - 4.0)], stroke);
            painter.line_segment([left, egui::pos2(left.x + 4.0, left.y + 4.0)], stroke);
        }
        ToolIcon::Copy => {
            let back = inner.translate(egui::vec2(3.0, -3.0));
            painter.rect_stroke(back, 2.0, stroke);
            painter.rect_stroke(inner, 2.0, stroke);
        }
        ToolIcon::Save => {
            painter.rect_stroke(inner, 2.0, stroke);
            let top = egui::Rect::from_min_max(
                egui::pos2(inner.min.x, inner.min.y),
                egui::pos2(inner.max.x, inner.min.y + inner.height() * 0.35),
            );
            painter.line_segment([top.min, egui::pos2(top.max.x, top.min.y)], stroke);
            let notch = egui::Rect::from_min_max(
                egui::pos2(inner.min.x + inner.width() * 0.15, inner.min.y + inner.height() * 0.45),
                egui::pos2(inner.min.x + inner.width() * 0.45, inner.min.y + inner.height() * 0.8),
            );
            painter.rect_stroke(notch, 1.5, stroke);
        }
        ToolIcon::Clear => {
            painter.line_segment([inner.min, inner.max], stroke);
            painter.line_segment(
                [egui::pos2(inner.min.x, inner.max.y), egui::pos2(inner.max.x, inner.min.y)],
                stroke,
            );
        }
    }
}

fn color32_to_rgba(color: egui::Color32) -> Rgba<u8> {
    Rgba([color.r(), color.g(), color.b(), color.a()])
}
