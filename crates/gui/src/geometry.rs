use eframe::egui;

use crate::shapes::SelectionCorner;

pub(crate) fn normalize_rect(rect: egui::Rect) -> egui::Rect {
    let min = egui::pos2(rect.min.x.min(rect.max.x), rect.min.y.min(rect.max.y));
    let max = egui::pos2(rect.min.x.max(rect.max.x), rect.min.y.max(rect.max.y));
    egui::Rect::from_min_max(min, max)
}

pub(crate) fn hit_corner(rect: egui::Rect, pos: egui::Pos2, radius: f32) -> Option<SelectionCorner> {
    let radius_sq = radius * radius;
    let corners = [
        (rect.min, SelectionCorner::TopLeft),
        (egui::pos2(rect.max.x, rect.min.y), SelectionCorner::TopRight),
        (egui::pos2(rect.min.x, rect.max.y), SelectionCorner::BottomLeft),
        (rect.max, SelectionCorner::BottomRight),
    ];
    for (corner_pos, corner) in corners {
        let dx = pos.x - corner_pos.x;
        let dy = pos.y - corner_pos.y;
        if dx * dx + dy * dy <= radius_sq {
            return Some(corner);
        }
    }
    None
}

pub(crate) fn selection_screen_rect(
    sel_rect_image: egui::Rect,
    image_rect: egui::Rect,
    scale: f32,
) -> egui::Rect {
    let min = image_rect.min + egui::vec2(sel_rect_image.min.x / scale, sel_rect_image.min.y / scale);
    let max = image_rect.min + egui::vec2(sel_rect_image.max.x / scale, sel_rect_image.max.y / scale);
    egui::Rect::from_min_max(min, max)
}

pub(crate) fn layout_tool_buttons(
    selection: egui::Rect,
    bounds: egui::Rect,
    button_size: egui::Vec2,
    spacing: f32,
    count: usize,
) -> Vec<egui::Pos2> {
    let mut positions = Vec::new();
    let mut remaining = count;
    let step_x = button_size.x + spacing;
    let step_y = button_size.y + spacing;

    let max_fit_row = ((bounds.width() + spacing) / step_x).floor().max(0.0) as usize;
    let max_fit_col = ((bounds.height() + spacing) / step_y).floor().max(0.0) as usize;
    if max_fit_row == 0 && max_fit_col == 0 {
        return positions;
    }

    let row_y = selection.max.y + spacing;
    if row_y >= bounds.min.y && row_y + button_size.y <= bounds.max.y {
        let max_by_sel = ((selection.width().max(button_size.x) + spacing) / step_x)
            .floor()
            .max(1.0) as usize;
        let count_here = remaining.min(max_by_sel).min(max_fit_row);
        if count_here > 0 {
            let row = row_positions(selection.center().x, row_y, count_here, button_size, spacing, bounds);
            remaining -= row.len();
            positions.extend(row);
        }
    }
    if remaining > 0 {
        let col_x = selection.max.x + spacing;
        if col_x >= bounds.min.x && col_x + button_size.x <= bounds.max.x {
            let max_by_sel = ((selection.height().max(button_size.y) + spacing) / step_y)
                .floor()
                .max(1.0) as usize;
            let count_here = remaining.min(max_by_sel).min(max_fit_col);
            if count_here > 0 {
                let col = col_positions(
                    selection.center().y,
                    col_x,
                    count_here,
                    button_size,
                    spacing,
                    bounds,
                );
                remaining -= col.len();
                positions.extend(col);
            }
        }
    }
    if remaining > 0 {
        let row_y = selection.min.y - spacing - button_size.y;
        if row_y >= bounds.min.y && row_y + button_size.y <= bounds.max.y {
            let max_by_sel = ((selection.width().max(button_size.x) + spacing) / step_x)
                .floor()
                .max(1.0) as usize;
            let count_here = remaining.min(max_by_sel).min(max_fit_row);
            if count_here > 0 {
                let row = row_positions(
                    selection.center().x,
                    row_y,
                    count_here,
                    button_size,
                    spacing,
                    bounds,
                );
                remaining -= row.len();
                positions.extend(row);
            }
        }
    }
    if remaining > 0 {
        let col_x = selection.min.x - spacing - button_size.x;
        if col_x >= bounds.min.x && col_x + button_size.x <= bounds.max.x {
            let max_by_sel = ((selection.height().max(button_size.y) + spacing) / step_y)
                .floor()
                .max(1.0) as usize;
            let count_here = remaining.min(max_by_sel).min(max_fit_col);
            if count_here > 0 {
                let col = col_positions(
                    selection.center().y,
                    col_x,
                    count_here,
                    button_size,
                    spacing,
                    bounds,
                );
                remaining -= col.len();
                positions.extend(col);
            }
        }
    }

    if remaining > 0 && positions.is_empty() {
        let y = (selection.max.y - button_size.y).clamp(bounds.min.y, bounds.max.y - button_size.y);
        let row = row_positions(selection.center().x, y, remaining.min(max_fit_row.max(1)), button_size, spacing, bounds);
        positions.extend(row);
    }

    positions
}

pub(crate) fn row_positions(
    center_x: f32,
    y: f32,
    count: usize,
    button_size: egui::Vec2,
    spacing: f32,
    bounds: egui::Rect,
) -> Vec<egui::Pos2> {
    let total_width = count as f32 * button_size.x + (count.saturating_sub(1) as f32) * spacing;
    let mut start_x = center_x - total_width / 2.0;
    let max_start = bounds.max.x - total_width;
    if max_start.is_finite() {
        start_x = start_x.clamp(bounds.min.x, max_start);
    }
    (0..count)
        .map(|i| egui::pos2(start_x + i as f32 * (button_size.x + spacing), y))
        .collect()
}

pub(crate) fn col_positions(
    center_y: f32,
    x: f32,
    count: usize,
    button_size: egui::Vec2,
    spacing: f32,
    bounds: egui::Rect,
) -> Vec<egui::Pos2> {
    let total_height = count as f32 * button_size.y + (count.saturating_sub(1) as f32) * spacing;
    let mut start_y = center_y - total_height / 2.0;
    let max_start = bounds.max.y - total_height;
    if max_start.is_finite() {
        start_y = start_y.clamp(bounds.min.y, max_start);
    }
    (0..count)
        .map(|i| egui::pos2(x, start_y + i as f32 * (button_size.y + spacing)))
        .collect()
}
