use eframe::egui;
use egui_file_dialog::{DialogState, FileDialog};
use fireshot_core::CaptureError;
use image::{DynamicImage, RgbaImage};

use crate::clipboard::{encode_bmp, encode_png, is_wayland, try_wl_copy_png, try_xclip};
use crate::draw::{
    arrow_head_points, circlecount_bubble_size, circlecount_contrast_colors, draw_arrow_head,
    draw_arrow_head_image, draw_circle_count_image, draw_circle_count_preview, draw_ellipse,
    draw_handles, draw_line, draw_selection_hud, ellipse_points, paint_tool_icon, with_alpha,
    CIRCLECOUNT_PADDING,
};
use crate::effects::{apply_blur, apply_blur_full, apply_pixelate, apply_pixelate_full};
use crate::geometry::{hit_corner, normalize_rect, selection_screen_rect, layout_tool_buttons};
use crate::image_ops::{crop_image, crop_image_exact, rect_to_u32};
use crate::shapes::{
    EffectKind, EffectPreview, EffectShape, SelectionCorner, SelectionDrag, SelectionRect, Shape,
    TextInput, TextShape, Tool, ToolAction, ToolIcon, CircleCountShape, FILE_DIALOG_SIZE,
};
use crate::text::draw_text_bitmap;

pub(crate) struct EditorApp {
    base_image: RgbaImage,
    texture_image: egui::ColorImage,
    texture: Option<egui::TextureHandle>,
    tool: Tool,
    last_draw_tool: Tool,
    color: egui::Color32,
    size: f32,
    shapes: Vec<Shape>,
    active_shape: Option<Shape>,
    redo_stack: Vec<Shape>,
    selection: Option<SelectionRect>,
    selection_drag: Option<SelectionDrag>,
    status: Option<String>,
    last_image_rect: Option<egui::Rect>,
    last_pixels_per_point: f32,
    tool_button_rects: Vec<egui::Rect>,
    tool_controls_rect: Option<egui::Rect>,
    text_input: Option<TextInput>,
    text_editor_rect: Option<egui::Rect>,
    shapes_version: u64,
    effect_previews: Vec<EffectPreview>,
    file_dialog: FileDialog,
    file_dialog_open: bool,
}

impl EditorApp {
    fn new(image: DynamicImage) -> Self {
        let rgba = image.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let pixels = rgba.clone().into_raw();
        let image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
        Self {
            base_image: rgba,
            texture_image: image,
            texture: None,
            tool: Tool::Select,
            last_draw_tool: Tool::Pencil,
            color: egui::Color32::from_rgb(255, 0, 0),
            size: 3.0,
            shapes: Vec::new(),
            active_shape: None,
            redo_stack: Vec::new(),
            selection: None,
            selection_drag: None,
            status: None,
            last_image_rect: None,
            last_pixels_per_point: 1.0,
            tool_button_rects: Vec::new(),
            tool_controls_rect: None,
            text_input: None,
            text_editor_rect: None,
            shapes_version: 0,
            effect_previews: Vec::new(),
            file_dialog: FileDialog::new()
                .default_file_name("screenshot.png")
                .default_size(FILE_DIALOG_SIZE),
            file_dialog_open: false,
        }
    }

    fn image_size(&self) -> egui::Vec2 {
        egui::vec2(self.base_image.width() as f32, self.base_image.height() as f32)
    }

    fn handle_input(&mut self, response: &egui::Response) {
        if self.file_dialog_open {
            return;
        }
        let scale = response.ctx.pixels_per_point();
        let pointer = response.ctx.input(|i| i.pointer.clone());
        let Some(pointer_pos) = pointer.hover_pos() else {
            return;
        };
        if self.is_over_ui(pointer_pos) {
            response.ctx.output_mut(|o| o.cursor_icon = egui::CursorIcon::Default);
            return;
        }
        if !response.rect.contains(pointer_pos) {
            if pointer.any_released() {
                if let Some(shape) = self.active_shape.take() {
                    self.push_shape(shape);
                }
            }
            return;
        }

        let img_pos_vec = (pointer_pos - response.rect.min) * scale;
        let img_pos = egui::pos2(img_pos_vec.x, img_pos_vec.y);
        let img_pos = egui::pos2(
            img_pos.x.clamp(0.0, self.image_size().x),
            img_pos.y.clamp(0.0, self.image_size().y),
        );

        if self.tool == Tool::Select {
            let icon = self.cursor_icon_for_selection(&pointer, img_pos, scale);
            response.ctx.output_mut(|o| o.cursor_icon = icon);
            self.handle_selection_input(&pointer, img_pos, scale);
            return;
        }

        if let Some(sel) = self.selection {
            if !sel.rect.contains(img_pos) {
                return;
            }
        } else {
            return;
        }

        if pointer.primary_pressed() {
            self.active_shape = Some(match self.tool {
                Tool::Select => return,
                Tool::Pencil => Shape::Stroke(crate::shapes::StrokeShape {
                    points: vec![img_pos],
                    color: self.color,
                    size: self.size,
                }),
                Tool::Marker => Shape::Stroke(crate::shapes::StrokeShape {
                    points: vec![img_pos],
                    color: with_alpha(self.color, 120),
                    size: self.size.max(6.0),
                }),
                Tool::MarkerLine => Shape::Line(crate::shapes::LineShape {
                    start: img_pos,
                    end: img_pos,
                    color: with_alpha(self.color, 120),
                    size: self.size.max(6.0),
                }),
                Tool::CircleCount => Shape::CircleCount(CircleCountShape {
                    center: img_pos,
                    pointer: img_pos,
                    color: self.color,
                    size: self.size,
                    count: self.next_circle_count(),
                }),
                Tool::Line => Shape::Line(crate::shapes::LineShape {
                    start: img_pos,
                    end: img_pos,
                    color: self.color,
                    size: self.size,
                }),
                Tool::Arrow => Shape::Arrow(crate::shapes::ArrowShape {
                    start: img_pos,
                    end: img_pos,
                    color: self.color,
                    size: self.size,
                }),
                Tool::Rect => Shape::Rect(crate::shapes::RectShape {
                    start: img_pos,
                    end: img_pos,
                    color: self.color,
                    size: self.size,
                }),
                Tool::Circle => Shape::Circle(crate::shapes::CircleShape {
                    start: img_pos,
                    end: img_pos,
                    color: self.color,
                    size: self.size,
                }),
                Tool::Pixelate => Shape::Effect(EffectShape {
                    start: img_pos,
                    end: img_pos,
                    size: self.size,
                    kind: EffectKind::Pixelate,
                }),
                Tool::Blur => Shape::Effect(EffectShape {
                    start: img_pos,
                    end: img_pos,
                    size: self.size,
                    kind: EffectKind::Blur,
                }),
                Tool::Text => {
                    self.text_input = Some(TextInput {
                        pos: img_pos,
                        text: String::new(),
                    });
                    return;
                }
            });
        } else if pointer.primary_down() {
            if let Some(active) = &mut self.active_shape {
                match active {
                    Shape::Stroke(stroke) => {
                        stroke.points.push(img_pos);
                    }
                    Shape::Line(line) => {
                        line.end = img_pos;
                    }
                    Shape::Arrow(arrow) => {
                        arrow.end = img_pos;
                    }
                    Shape::Rect(rect) => {
                        rect.end = img_pos;
                    }
                    Shape::Circle(circle) => {
                        circle.end = img_pos;
                    }
                    Shape::CircleCount(counter) => {
                        counter.pointer = img_pos;
                    }
                    Shape::Effect(effect) => {
                        effect.end = img_pos;
                    }
                    Shape::Text(_) => {}
                }
            }
        } else if pointer.primary_released() {
            if let Some(shape) = self.active_shape.take() {
                self.push_shape(shape);
            }
        }
    }

    fn handle_selection_input(
        &mut self,
        pointer: &egui::PointerState,
        img_pos: egui::Pos2,
        scale: f32,
    ) {
        let handle_radius = 6.0 * scale;
        let image_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, self.image_size());

        if pointer.primary_pressed() {
            if let Some(sel) = self.selection {
                if let Some(corner) = hit_corner(sel.rect, img_pos, handle_radius) {
                    self.selection_drag = Some(SelectionDrag::Resizing { corner });
                } else if sel.rect.contains(img_pos) {
                    self.selection_drag =
                        Some(SelectionDrag::Moving { offset: img_pos - sel.rect.min });
                } else {
                    self.selection_drag = Some(SelectionDrag::Creating { start: img_pos });
                    self.selection = Some(SelectionRect {
                        rect: egui::Rect::from_two_pos(img_pos, img_pos),
                    });
                }
            } else {
                self.selection_drag = Some(SelectionDrag::Creating { start: img_pos });
                self.selection = Some(SelectionRect {
                    rect: egui::Rect::from_two_pos(img_pos, img_pos),
                });
            }
        } else if pointer.primary_down() {
            if let Some(drag) = self.selection_drag {
                match drag {
                    SelectionDrag::Creating { start } => {
                        let rect = egui::Rect::from_two_pos(start, img_pos);
                        self.selection = Some(SelectionRect { rect: rect.intersect(image_rect) });
                    }
                    SelectionDrag::Moving { offset } => {
                        if let Some(sel) = self.selection {
                            let size = sel.rect.size();
                            let mut min = img_pos - offset;
                            let max_x = (self.image_size().x - size.x).max(0.0);
                            let max_y = (self.image_size().y - size.y).max(0.0);
                            min.x = min.x.clamp(0.0, max_x);
                            min.y = min.y.clamp(0.0, max_y);
                            let rect = egui::Rect::from_min_size(min, size);
                            self.selection = Some(SelectionRect { rect });
                        }
                    }
                    SelectionDrag::Resizing { corner } => {
                        if let Some(sel) = self.selection {
                            let mut rect = sel.rect;
                            match corner {
                                SelectionCorner::TopLeft => {
                                    rect.min = img_pos;
                                }
                                SelectionCorner::TopRight => {
                                    rect.min.y = img_pos.y;
                                    rect.max.x = img_pos.x;
                                }
                                SelectionCorner::BottomLeft => {
                                    rect.min.x = img_pos.x;
                                    rect.max.y = img_pos.y;
                                }
                                SelectionCorner::BottomRight => {
                                    rect.max = img_pos;
                                }
                            }
                            rect = normalize_rect(rect);
                            rect = rect.intersect(image_rect);
                            rect = normalize_rect(rect);
                            self.selection = Some(SelectionRect { rect });
                        }
                    }
                }
            }
        } else if pointer.primary_released() {
            self.selection_drag = None;
            if let Some(sel) = self.selection {
                if sel.rect.width() < 1.0 || sel.rect.height() < 1.0 {
                    self.selection = None;
                }
            }
        }
    }

    fn cursor_icon_for_selection(
        &self,
        pointer: &egui::PointerState,
        img_pos: egui::Pos2,
        scale: f32,
    ) -> egui::CursorIcon {
        if let Some(drag) = self.selection_drag {
            return match drag {
                SelectionDrag::Moving { .. } => egui::CursorIcon::Grabbing,
                SelectionDrag::Resizing { corner } => match corner {
                    SelectionCorner::TopLeft | SelectionCorner::BottomRight => {
                        egui::CursorIcon::ResizeNwSe
                    }
                    SelectionCorner::TopRight | SelectionCorner::BottomLeft => {
                        egui::CursorIcon::ResizeNeSw
                    }
                },
                SelectionDrag::Creating { .. } => egui::CursorIcon::Crosshair,
            };
        }

        let handle_radius = 6.0 * scale;
        if let Some(sel) = self.selection {
            if let Some(corner) = hit_corner(sel.rect, img_pos, handle_radius) {
                return match corner {
                    SelectionCorner::TopLeft | SelectionCorner::BottomRight => {
                        egui::CursorIcon::ResizeNwSe
                    }
                    SelectionCorner::TopRight | SelectionCorner::BottomLeft => {
                        egui::CursorIcon::ResizeNeSw
                    }
                };
            }
            if sel.rect.contains(img_pos) {
                return if pointer.primary_down() {
                    egui::CursorIcon::Grabbing
                } else {
                    egui::CursorIcon::Grab
                };
            }
        }
        egui::CursorIcon::Crosshair
    }

    fn is_over_ui(&self, pos: egui::Pos2) -> bool {
        if self.tool_button_rects.iter().any(|rect| rect.contains(pos)) {
            return true;
        }
        if let Some(rect) = self.tool_controls_rect {
            if rect.contains(pos) {
                return true;
            }
        }
        if let Some(rect) = self.text_editor_rect {
            if rect.contains(pos) {
                return true;
            }
        }
        false
    }

    fn draw_overlay(&mut self, response: &egui::Response, painter: &egui::Painter) {
        let scale = response.ctx.pixels_per_point();
        let to_screen = |p: egui::Pos2| {
            response.rect.min + egui::vec2(p.x / scale, p.y / scale)
        };
        let has_effects = self
            .shapes
            .iter()
            .any(|s| matches!(s, Shape::Effect(_)))
            || matches!(self.active_shape, Some(Shape::Effect(_)));
        let base_preview = if has_effects {
            Some(self.render_full_image_without_effects())
        } else {
            None
        };
        let mut effect_index = 0usize;
        let idle_dim = egui::Color32::from_rgba_premultiplied(0, 0, 0, 70);
        let selection_dim = egui::Color32::from_rgba_premultiplied(0, 0, 0, 110);

        if let Some(sel) = self.selection {
            let img_rect = response.rect;
            let sel_rect = egui::Rect::from_two_pos(to_screen(sel.rect.min), to_screen(sel.rect.max));

            let top = egui::Rect::from_min_max(img_rect.min, egui::pos2(img_rect.max.x, sel_rect.min.y));
            let bottom =
                egui::Rect::from_min_max(egui::pos2(img_rect.min.x, sel_rect.max.y), img_rect.max);
            let left = egui::Rect::from_min_max(
                egui::pos2(img_rect.min.x, sel_rect.min.y),
                egui::pos2(sel_rect.min.x, sel_rect.max.y),
            );
            let right = egui::Rect::from_min_max(
                egui::pos2(sel_rect.max.x, sel_rect.min.y),
                egui::pos2(img_rect.max.x, sel_rect.max.y),
            );

            painter.rect_filled(top, 0.0, selection_dim);
            painter.rect_filled(bottom, 0.0, selection_dim);
            painter.rect_filled(left, 0.0, selection_dim);
            painter.rect_filled(right, 0.0, selection_dim);

            painter.rect_stroke(sel_rect, 0.0, egui::Stroke::new(1.5, egui::Color32::WHITE));
            draw_handles(painter, sel_rect, 4.0, egui::Color32::WHITE);
            draw_selection_hud(painter, sel_rect, sel.rect, response.rect);
        } else if !self.file_dialog_open {
            painter.rect_filled(response.rect, 0.0, idle_dim);
            self.draw_help_overlay(&response.ctx, painter, response.rect);
        }

        self.draw_cursor_brush_preview(response, scale, painter);
        let shapes = self.shapes.clone();
        for shape in &shapes {
            self.draw_shape_preview(
                shape,
                painter,
                &to_screen,
                scale,
                base_preview.as_ref(),
                &mut effect_index,
                &response.ctx,
            );
        }
        if let Some(active) = self.active_shape.clone() {
            self.draw_shape_preview(
                &active,
                painter,
                &to_screen,
                scale,
                base_preview.as_ref(),
                &mut effect_index,
                &response.ctx,
            );
        }
    }

    fn draw_help_overlay(
        &self,
        ctx: &egui::Context,
        painter: &egui::Painter,
        rect: egui::Rect,
    ) {
        let title = "Click and drag to select area";
        let hints = [
            "Ctrl+C: copy",
            "Ctrl+S: save",
            "Ctrl+Z / Ctrl+Shift+Z: undo/redo",
            "Mouse wheel: change tool size",
            "Esc: close",
        ];
        let font = egui::FontId::proportional(18.0);
        let title_color = egui::Color32::from_rgb(245, 245, 245);
        let hint_color = egui::Color32::from_rgb(220, 220, 220);

        let title_galley =
            ctx.fonts(|f| f.layout_no_wrap(title.into(), font.clone(), title_color));
        let hint_galleys: Vec<_> = hints
            .iter()
            .map(|text| ctx.fonts(|f| f.layout_no_wrap((*text).into(), font.clone(), hint_color)))
            .collect();

        let mut width = title_galley.size().x;
        let mut height = title_galley.size().y;
        let spacing = 6.0;
        for galley in &hint_galleys {
            width = width.max(galley.size().x);
            height += spacing + galley.size().y;
        }

        let padding = egui::vec2(18.0, 14.0);
        let box_size = egui::vec2(width + padding.x * 2.0, height + padding.y * 2.0);
        let box_rect = egui::Rect::from_center_size(rect.center(), box_size);
        painter.rect_filled(box_rect, 10.0, egui::Color32::from_rgb(12, 12, 12));
        painter.rect_stroke(
            box_rect,
            10.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30)),
        );

        let x = box_rect.min.x + padding.x;
        let mut y = box_rect.min.y + padding.y;
        painter.text(
            egui::pos2(x, y),
            egui::Align2::LEFT_TOP,
            title,
            font.clone(),
            title_color,
        );
        y += title_galley.size().y + spacing;
        for (idx, hint) in hints.iter().enumerate() {
            painter.text(
                egui::pos2(x, y),
                egui::Align2::LEFT_TOP,
                *hint,
                font.clone(),
                hint_color,
            );
            y += hint_galleys[idx].size().y + spacing;
        }
    }

    fn draw_cursor_brush_preview(
        &self,
        response: &egui::Response,
        scale: f32,
        painter: &egui::Painter,
    ) {
        if matches!(self.tool, Tool::Select) || self.text_input.is_some() {
            return;
        }
        let Some(pointer_pos) = response.ctx.input(|i| i.pointer.hover_pos()) else {
            return;
        };
        if self.is_over_ui(pointer_pos) {
            return;
        }
        if !response.rect.contains(pointer_pos) {
            return;
        }
        let img_pos_vec = (pointer_pos - response.rect.min) * scale;
        let img_pos = egui::pos2(img_pos_vec.x, img_pos_vec.y);
        if let Some(sel) = self.selection {
            if !sel.rect.contains(img_pos) {
                return;
            }
        } else {
            return;
        }

        if matches!(self.tool, Tool::CircleCount) {
            let (contrast, anti) = circlecount_contrast_colors(self.color);
            let bubble_size = circlecount_bubble_size(self.size);
            let outer_radius = (bubble_size + CIRCLECOUNT_PADDING) / scale;
            let inner_radius = bubble_size / scale;
            painter.circle_filled(pointer_pos, outer_radius, anti);
            painter.circle_stroke(
                pointer_pos,
                outer_radius,
                egui::Stroke::new(1.0, contrast),
            );
            painter.circle_filled(pointer_pos, inner_radius, self.color);
            return;
        }

        let mut color = self.color;
        if matches!(self.tool, Tool::Marker | Tool::MarkerLine) {
            color = with_alpha(self.color, 120);
        }
        if matches!(self.tool, Tool::Pixelate | Tool::Blur) {
            color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 200);
        }
        let radius = (self.size.max(1.0) / scale) * 0.5;
        painter.circle_stroke(pointer_pos, radius.max(1.0), egui::Stroke::new(1.0, color));
    }

    fn show_tool_buttons(&mut self, ctx: &egui::Context) {
        if self.file_dialog_open {
            return;
        }
        let Some(sel) = self.selection else {
            return;
        };
        let Some(image_rect) = self.last_image_rect else {
            return;
        };
        self.tool_button_rects.clear();
        let scale = self.last_pixels_per_point;
        let sel_rect_screen = selection_screen_rect(sel.rect, image_rect, scale);

        let button_size = egui::vec2(28.0, 28.0);
        let spacing = 6.0;
        let current_tool = self.tool;
        let buttons = [
            ("Select", ToolAction::Tool(Tool::Select), ToolIcon::Select, current_tool == Tool::Select),
            ("Pencil", ToolAction::Tool(Tool::Pencil), ToolIcon::Pencil, current_tool == Tool::Pencil),
            ("Line", ToolAction::Tool(Tool::Line), ToolIcon::Line, current_tool == Tool::Line),
            ("Arrow", ToolAction::Tool(Tool::Arrow), ToolIcon::Arrow, current_tool == Tool::Arrow),
            ("Rect", ToolAction::Tool(Tool::Rect), ToolIcon::Rect, current_tool == Tool::Rect),
            ("Circle", ToolAction::Tool(Tool::Circle), ToolIcon::Circle, current_tool == Tool::Circle),
            ("Marker", ToolAction::Tool(Tool::Marker), ToolIcon::Marker, current_tool == Tool::Marker),
            (
                "Marker Line",
                ToolAction::Tool(Tool::MarkerLine),
                ToolIcon::MarkerLine,
                current_tool == Tool::MarkerLine,
            ),
            (
                "Circle Count",
                ToolAction::Tool(Tool::CircleCount),
                ToolIcon::CircleCount,
                current_tool == Tool::CircleCount,
            ),
            ("Text", ToolAction::Tool(Tool::Text), ToolIcon::Text, current_tool == Tool::Text),
            ("Pixelate", ToolAction::Tool(Tool::Pixelate), ToolIcon::Pixelate, current_tool == Tool::Pixelate),
            ("Blur", ToolAction::Tool(Tool::Blur), ToolIcon::Blur, current_tool == Tool::Blur),
            ("Undo", ToolAction::Undo, ToolIcon::Undo, false),
            ("Copy", ToolAction::Copy, ToolIcon::Copy, false),
            ("Save", ToolAction::Save, ToolIcon::Save, false),
            ("Clear", ToolAction::Clear, ToolIcon::Clear, false),
        ];
        let positions = layout_tool_buttons(
            sel_rect_screen,
            image_rect,
            button_size,
            spacing,
            buttons.len(),
        );
        let mut index = 0;
        let mut add_tool =
            |tooltip: &str, action: ToolAction, icon: ToolIcon, selected: bool| {
                if index >= positions.len() {
                    return;
                }
                let pos = positions[index];
                index += 1;
                self.tool_button_rects
                    .push(egui::Rect::from_min_size(pos, button_size));
                let id = format!("tool_btn_{:?}", action);
                egui::Area::new(id.into())
                    .order(egui::Order::Foreground)
                    .fixed_pos(pos)
                    .show(ctx, |ui| {
                        let response = ui.add_sized(button_size, egui::Button::new(""));
                        let response = response.on_hover_text(tooltip);
                        let visuals = ui.visuals();
                        let fg = if selected {
                            visuals.selection.stroke.color
                        } else {
                            visuals.widgets.inactive.fg_stroke.color
                        };
                        let painter = ui.painter_at(response.rect);
                        if selected {
                            painter.rect_stroke(
                                response.rect.shrink(1.0),
                                4.0,
                                egui::Stroke::new(1.5, visuals.selection.stroke.color),
                            );
                        }
                        paint_tool_icon(&painter, response.rect, icon, fg);
                        if response.clicked() {
                            match action {
                                ToolAction::Tool(tool) => self.tool = tool,
                                ToolAction::Undo => {
                                    self.pop_shape();
                                }
                                ToolAction::Copy => self.copy_and_close(ctx),
                                ToolAction::Save => self.save_image(),
                                ToolAction::Clear => self.clear_shapes(),
                            }
                        }
                    });
            };

        for (tooltip, action, icon, selected) in buttons {
            add_tool(tooltip, action, icon, selected);
        }
    }

    fn show_tool_controls(&mut self, ctx: &egui::Context) {
        if self.file_dialog_open {
            return;
        }
        let Some(sel) = self.selection else {
            return;
        };
        let Some(image_rect) = self.last_image_rect else {
            return;
        };
        self.tool_controls_rect = None;
        let scale = self.last_pixels_per_point;
        let sel_rect_screen = selection_screen_rect(sel.rect, image_rect, scale);

        let panel_size = egui::vec2(240.0, 36.0);
        let spacing = 6.0;
        let candidates = [
            egui::pos2(sel_rect_screen.max.x - panel_size.x, sel_rect_screen.max.y + spacing),
            egui::pos2(sel_rect_screen.min.x, sel_rect_screen.max.y + spacing),
            egui::pos2(sel_rect_screen.max.x - panel_size.x, sel_rect_screen.min.y - panel_size.y - spacing),
            egui::pos2(sel_rect_screen.min.x, sel_rect_screen.min.y - panel_size.y - spacing),
        ];
        let mut pos = None;
        for cand in candidates {
            let mut rect = egui::Rect::from_min_size(cand, panel_size);
            if rect.min.x < image_rect.min.x {
                rect = rect.translate(egui::vec2(image_rect.min.x - rect.min.x, 0.0));
            }
            if rect.max.x > image_rect.max.x {
                rect = rect.translate(egui::vec2(image_rect.max.x - rect.max.x, 0.0));
            }
            if rect.min.y < image_rect.min.y {
                rect = rect.translate(egui::vec2(0.0, image_rect.min.y - rect.min.y));
            }
            if rect.max.y > image_rect.max.y {
                rect = rect.translate(egui::vec2(0.0, image_rect.max.y - rect.max.y));
            }
            if !rect.intersects(image_rect) {
                continue;
            }
            if self.tool_button_rects.iter().all(|b| !b.intersects(rect)) {
                pos = Some(rect.min);
                break;
            }
        }
        let pos = pos.unwrap_or_else(|| {
            let mut fallback = egui::Rect::from_min_size(
                egui::pos2(sel_rect_screen.max.x - panel_size.x, sel_rect_screen.max.y + spacing),
                panel_size,
            );
            if fallback.min.x < image_rect.min.x {
                fallback = fallback.translate(egui::vec2(image_rect.min.x - fallback.min.x, 0.0));
            }
            if fallback.max.x > image_rect.max.x {
                fallback = fallback.translate(egui::vec2(image_rect.max.x - fallback.max.x, 0.0));
            }
            if fallback.max.y > image_rect.max.y {
                fallback = fallback.translate(egui::vec2(0.0, image_rect.max.y - fallback.max.y));
            }
            fallback.min
        });
        self.tool_controls_rect = Some(egui::Rect::from_min_size(pos, panel_size));

        egui::Area::new("tool_controls".into())
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .rounding(6.0)
                    .inner_margin(egui::Margin::same(6.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.color_edit_button_srgba(&mut self.color);
                            ui.add(egui::Slider::new(&mut self.size, 1.0..=20.0).text("Size"));
                        });
                        if let Some(status) = &self.status {
                            ui.label(status);
                        }
                    });
            });
    }

    fn show_text_editor(&mut self, ctx: &egui::Context) {
        if self.file_dialog_open {
            return;
        }
        self.text_editor_rect = None;
        let Some(input) = &mut self.text_input else {
            return;
        };
        let Some(image_rect) = self.last_image_rect else {
            return;
        };
        let scale = self.last_pixels_per_point;
        let screen_pos = image_rect.min + egui::vec2(input.pos.x / scale, input.pos.y / scale);
        let editor_size = egui::vec2(220.0, 32.0);
        let mut pos = screen_pos + egui::vec2(6.0, 6.0);
        let mut rect = egui::Rect::from_min_size(pos, editor_size);
        if rect.max.x > image_rect.max.x {
            rect = rect.translate(egui::vec2(image_rect.max.x - rect.max.x, 0.0));
        }
        if rect.max.y > image_rect.max.y {
            rect = rect.translate(egui::vec2(0.0, image_rect.max.y - rect.max.y));
        }
        if rect.min.x < image_rect.min.x {
            rect = rect.translate(egui::vec2(image_rect.min.x - rect.min.x, 0.0));
        }
        if rect.min.y < image_rect.min.y {
            rect = rect.translate(egui::vec2(0.0, image_rect.min.y - rect.min.y));
        }
        pos = rect.min;
        self.text_editor_rect = Some(rect);

        egui::Area::new("text_editor".into())
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .rounding(4.0)
                    .inner_margin(egui::Margin::same(4.0))
                    .show(ui, |ui| {
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut input.text)
                                .desired_width(editor_size.x - 8.0)
                                .font(egui::TextStyle::Body),
                        );
                        response.request_focus();
                    });
            });
    }

    fn draw_shape_preview<F: Fn(egui::Pos2) -> egui::Pos2>(
        &mut self,
        shape: &Shape,
        painter: &egui::Painter,
        to_screen: &F,
        scale: f32,
        base_preview: Option<&RgbaImage>,
        effect_index: &mut usize,
        ctx: &egui::Context,
    ) {
        match shape {
            Shape::Stroke(stroke) => {
                let points: Vec<egui::Pos2> =
                    stroke.points.iter().copied().map(to_screen).collect();
                painter.add(egui::Shape::line(
                    points,
                    egui::Stroke::new(stroke.size, stroke.color),
                ));
            }
            Shape::Line(line) => {
                painter.add(egui::Shape::line_segment(
                    [to_screen(line.start), to_screen(line.end)],
                    egui::Stroke::new(line.size, line.color),
                ));
            }
            Shape::Rect(rect) => {
                let rect_area = egui::Rect::from_two_pos(to_screen(rect.start), to_screen(rect.end));
                painter.add(egui::Shape::rect_stroke(
                    rect_area,
                    0.0,
                    egui::Stroke::new(rect.size, rect.color),
                ));
            }
            Shape::Circle(circle) => {
                let rect_area =
                    egui::Rect::from_two_pos(to_screen(circle.start), to_screen(circle.end));
                let points = ellipse_points(rect_area, 40);
                painter.add(egui::Shape::line(
                    points,
                    egui::Stroke::new(circle.size, circle.color),
                ));
            }
            Shape::CircleCount(counter) => {
                draw_circle_count_preview(painter, to_screen, counter, scale);
            }
            Shape::Arrow(arrow) => {
                let start = to_screen(arrow.start);
                let end = to_screen(arrow.end);
                let (base, _, _) = arrow_head_points(start, end, arrow.size);
                painter.add(egui::Shape::line_segment(
                    [start, base],
                    egui::Stroke::new(arrow.size, arrow.color),
                ));
                draw_arrow_head(painter, start, end, arrow.size, arrow.color);
            }
            Shape::Text(text) => {
                painter.text(
                    to_screen(text.pos),
                    egui::Align2::LEFT_TOP,
                    text.text.as_str(),
                    egui::FontId::proportional(text.size),
                    text.color,
                );
            }
            Shape::Effect(effect) => {
                let rect_area =
                    egui::Rect::from_two_pos(to_screen(effect.start), to_screen(effect.end));
                let texture = base_preview
                    .and_then(|base| self.ensure_effect_preview(ctx, base, effect, *effect_index));
                if let Some(tex) = texture {
                    painter.image(
                        tex.id(),
                        rect_area,
                        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                } else {
                    painter.add(egui::Shape::rect_stroke(
                        rect_area,
                        0.0,
                        egui::Stroke::new(1.5, egui::Color32::WHITE),
                    ));
                }
                *effect_index += 1;
            }
        }
    }

    fn push_shape(&mut self, shape: Shape) {
        self.shapes.push(shape);
        self.shapes_version = self.shapes_version.wrapping_add(1);
        self.effect_previews.clear();
        self.redo_stack.clear();
    }

    fn pop_shape(&mut self) {
        if let Some(shape) = self.shapes.pop() {
            self.redo_stack.push(shape);
            self.shapes_version = self.shapes_version.wrapping_add(1);
            self.effect_previews.clear();
        }
    }

    fn clear_shapes(&mut self) {
        if !self.shapes.is_empty() {
            self.shapes.clear();
            self.shapes_version = self.shapes_version.wrapping_add(1);
            self.effect_previews.clear();
            self.redo_stack.clear();
        }
    }

    fn redo_shape(&mut self) {
        if let Some(shape) = self.redo_stack.pop() {
            self.shapes.push(shape);
            self.shapes_version = self.shapes_version.wrapping_add(1);
            self.effect_previews.clear();
        }
    }

    fn next_circle_count(&self) -> u32 {
        let mut max_count = 0;
        for shape in &self.shapes {
            if let Shape::CircleCount(counter) = shape {
                max_count = max_count.max(counter.count);
            }
        }
        max_count + 1
    }

    fn render_full_image_without_effects(&self) -> RgbaImage {
        let mut img = self.base_image.clone();
        for shape in &self.shapes {
            match shape {
                Shape::Stroke(stroke) => {
                    for win in stroke.points.windows(2) {
                        draw_line(&mut img, win[0], win[1], stroke.color, stroke.size);
                    }
                }
                Shape::Line(line) => {
                    draw_line(&mut img, line.start, line.end, line.color, line.size);
                }
                Shape::Arrow(arrow) => {
                    let (base, _, _) = arrow_head_points(arrow.start, arrow.end, arrow.size);
                    draw_line(&mut img, arrow.start, base, arrow.color, arrow.size);
                    draw_arrow_head_image(&mut img, arrow.start, arrow.end, arrow.color, arrow.size);
                }
                Shape::Rect(rect) => {
                    let a = rect.start;
                    let b = rect.end;
                    let top_left = egui::pos2(a.x.min(b.x), a.y.min(b.y));
                    let bottom_right = egui::pos2(a.x.max(b.x), a.y.max(b.y));
                    let top_right = egui::pos2(bottom_right.x, top_left.y);
                    let bottom_left = egui::pos2(top_left.x, bottom_right.y);
                    draw_line(&mut img, top_left, top_right, rect.color, rect.size);
                    draw_line(&mut img, top_right, bottom_right, rect.color, rect.size);
                    draw_line(&mut img, bottom_right, bottom_left, rect.color, rect.size);
                    draw_line(&mut img, bottom_left, top_left, rect.color, rect.size);
                }
                Shape::Circle(circle) => {
                    draw_ellipse(&mut img, circle.start, circle.end, circle.color, circle.size);
                }
                Shape::CircleCount(counter) => {
                    draw_circle_count_image(&mut img, counter);
                }
                Shape::Text(text) => {
                    let scale = (text.size / 6.0).round().max(1.0) as u32;
                    draw_text_bitmap(&mut img, text.pos, &text.text, text.color, scale);
                }
                Shape::Effect(_) => {}
            }
        }
        img
    }

    fn ensure_effect_preview(
        &mut self,
        ctx: &egui::Context,
        base: &RgbaImage,
        effect: &EffectShape,
        idx: usize,
    ) -> Option<egui::TextureHandle> {
        let rect = normalize_rect(egui::Rect::from_two_pos(effect.start, effect.end));
        let (min_x, min_y, max_x, max_y) = rect_to_u32(base, rect)?;
        let size_param = match effect.kind {
            EffectKind::Pixelate => effect.size.round().max(4.0) as u32,
            EffectKind::Blur => effect.size.round().max(2.0) as u32,
        };
        let rect_key = [min_x, min_y, max_x, max_y];
        if let Some(preview) = self.effect_previews.get_mut(idx) {
            if preview.rect == rect_key
                && preview.kind == effect.kind
                && preview.size == size_param
                && preview.shapes_version == self.shapes_version
            {
                return Some(preview.texture.clone());
            }
        }

        let mut sub = crop_image_exact(base, rect)?;
        match effect.kind {
            EffectKind::Pixelate => apply_pixelate_full(&mut sub, size_param),
            EffectKind::Blur => apply_blur_full(&mut sub, size_param.min(12)),
        }
        let size = [sub.width() as usize, sub.height() as usize];
        let pixels = sub.into_raw();
        let image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
        let texture = if let Some(preview) = self.effect_previews.get_mut(idx) {
            preview.texture.set(image, egui::TextureOptions::default());
            preview.rect = rect_key;
            preview.kind = effect.kind;
            preview.size = size_param;
            preview.shapes_version = self.shapes_version;
            preview.texture.clone()
        } else {
            let tex = ctx.load_texture(
                format!("effect_preview_{}", idx),
                image,
                egui::TextureOptions::default(),
            );
            self.effect_previews.push(EffectPreview {
                rect: rect_key,
                kind: effect.kind,
                size: size_param,
                shapes_version: self.shapes_version,
                texture: tex.clone(),
            });
            tex
        };
        Some(texture)
    }

    fn render_image(&self) -> RgbaImage {
        let mut img = self.render_full_image();
        if let Some(sel) = self.selection {
            img = crop_image(&img, sel.rect);
        }
        img
    }

    fn render_full_image(&self) -> RgbaImage {
        let mut img = self.base_image.clone();
        for shape in &self.shapes {
            match shape {
                Shape::Stroke(stroke) => {
                    for win in stroke.points.windows(2) {
                        draw_line(&mut img, win[0], win[1], stroke.color, stroke.size);
                    }
                }
                Shape::Line(line) => {
                    draw_line(&mut img, line.start, line.end, line.color, line.size);
                }
                Shape::Arrow(arrow) => {
                    let (base, _, _) = arrow_head_points(arrow.start, arrow.end, arrow.size);
                    draw_line(&mut img, arrow.start, base, arrow.color, arrow.size);
                    draw_arrow_head_image(&mut img, arrow.start, arrow.end, arrow.color, arrow.size);
                }
                Shape::Rect(rect) => {
                    let a = rect.start;
                    let b = rect.end;
                    let top_left = egui::pos2(a.x.min(b.x), a.y.min(b.y));
                    let bottom_right = egui::pos2(a.x.max(b.x), a.y.max(b.y));
                    let top_right = egui::pos2(bottom_right.x, top_left.y);
                    let bottom_left = egui::pos2(top_left.x, bottom_right.y);
                    draw_line(&mut img, top_left, top_right, rect.color, rect.size);
                    draw_line(&mut img, top_right, bottom_right, rect.color, rect.size);
                    draw_line(&mut img, bottom_right, bottom_left, rect.color, rect.size);
                    draw_line(&mut img, bottom_left, top_left, rect.color, rect.size);
                }
                Shape::Circle(circle) => {
                    draw_ellipse(&mut img, circle.start, circle.end, circle.color, circle.size);
                }
                Shape::CircleCount(counter) => {
                    draw_circle_count_image(&mut img, counter);
                }
                Shape::Text(text) => {
                    let scale = (text.size / 6.0).round().max(1.0) as u32;
                    draw_text_bitmap(&mut img, text.pos, &text.text, text.color, scale);
                }
                Shape::Effect(effect) => {
                    let rect = normalize_rect(egui::Rect::from_two_pos(effect.start, effect.end));
                    match effect.kind {
                        EffectKind::Pixelate => {
                            let block = effect.size.round().max(4.0) as u32;
                            apply_pixelate(&mut img, rect, block);
                        }
                        EffectKind::Blur => {
                            let radius = effect.size.round().max(2.0) as u32;
                            apply_blur(&mut img, rect, radius.min(12));
                        }
                    }
                }
            }
        }
        img
    }

    fn save_image(&mut self) {
        if let Some(rect) = self.last_image_rect {
            let pos = rect.center() - FILE_DIALOG_SIZE * 0.5;
            self.file_dialog = FileDialog::new()
                .default_file_name("screenshot.png")
                .default_size(FILE_DIALOG_SIZE)
                .default_pos(pos);
        }
        self.file_dialog.save_file();
        self.file_dialog_open = true;
    }

    fn copy_and_close(&mut self, ctx: &egui::Context) {
        let rendered = self.render_image();
        let mut copied = false;
        let mut method = "none";

        if is_wayland() {
            if let Ok(png) = encode_png(&rendered) {
                let wl_ok = try_wl_copy_png(&png).is_ok();
                let mut x11_ok = false;

                if try_xclip("image/png", &png).is_ok() {
                    x11_ok = true;
                } else if let Ok(bmp) = encode_bmp(&rendered) {
                    if try_xclip("image/bmp", &bmp).is_ok() {
                        x11_ok = true;
                    }
                }

                if wl_ok || x11_ok {
                    copied = true;
                    method = match (wl_ok, x11_ok) {
                        (true, true) => "wl-copy image/png + xclip image/png/bmp",
                        (true, false) => "wl-copy image/png",
                        (false, true) => "xclip image/png/bmp",
                        (false, false) => "none",
                    };
                }
            }
        }

        if copied {
            self.status = Some(format!("Copied to clipboard ({})", method));
        } else {
            self.status = Some("Clipboard copy failed".to_string());
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

impl eframe::App for EditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.texture.is_none() {
            self.texture = Some(ctx.load_texture(
                "capture",
                self.texture_image.clone(),
                egui::TextureOptions::default(),
            ));
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                if let Some(texture) = &self.texture {
                    let scale = ctx.pixels_per_point();
                    let image_size = self.image_size() / scale;
                    let response = ui.add(
                        egui::Image::new(texture)
                            .fit_to_exact_size(image_size)
                            .sense(egui::Sense::click_and_drag()),
                    );
                    let painter = ui.painter();
                    self.last_image_rect = Some(response.rect);
                    self.last_pixels_per_point = scale;
                    self.handle_input(&response);
                    self.draw_overlay(&response, painter);
                }
            });

        self.file_dialog.update(ctx);
        self.file_dialog_open = matches!(self.file_dialog.state(), DialogState::Open);

        if let Some(path) = self.file_dialog.take_selected() {
            let rendered = self.render_image();
            match rendered.save(&path) {
                Ok(()) => {
                    self.status = Some(format!("Saved {}", path.display()));
                }
                Err(err) => {
                    self.status = Some(format!("Save failed: {}", err));
                }
            }
            self.file_dialog_open = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.file_dialog_open && matches!(self.file_dialog.state(), DialogState::Closed) {
            self.file_dialog_open = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        self.show_tool_buttons(ctx);
        self.show_tool_controls(ctx);
        self.show_text_editor(ctx);

        let copy_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::C);
        let copy_shortcut_shift =
            egui::KeyboardShortcut::new(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::C);
        let copy_shortcut_cmd = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::C);
        let copy_requested = ctx.input_mut(|i| {
            let mut triggered = i.consume_shortcut(&copy_shortcut)
                || i.consume_shortcut(&copy_shortcut_shift)
                || i.consume_shortcut(&copy_shortcut_cmd);
            if !triggered {
                triggered = i
                    .events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Copy));
            }
            triggered
        });
        if copy_requested {
            self.copy_and_close(ctx);
        }

        let save_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::S);
        let save_shortcut_cmd = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::S);
        let save_requested = ctx.input_mut(|i| {
            i.consume_shortcut(&save_shortcut) || i.consume_shortcut(&save_shortcut_cmd)
        });
        if save_requested {
            self.save_image();
        }

        let undo_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::Z);
        let redo_shortcut = egui::KeyboardShortcut::new(
            egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
            egui::Key::Z,
        );
        let (undo_requested, redo_requested) = ctx.input_mut(|i| {
            let redo = i.consume_shortcut(&redo_shortcut);
            let undo = i.consume_shortcut(&undo_shortcut);
            (undo, redo)
        });
        if redo_requested {
            self.redo_shape();
        } else if undo_requested {
            self.pop_shape();
        }

        if self.tool != Tool::Select {
            self.last_draw_tool = self.tool;
        }

        let enter_pressed = ctx.input(|i| i.key_pressed(egui::Key::Enter));
        if enter_pressed {
            if let Some(input) = self.text_input.take() {
                if !input.text.trim().is_empty() {
                    self.push_shape(Shape::Text(TextShape {
                        pos: input.pos,
                        text: input.text,
                        color: self.color,
                        size: self.size.max(8.0),
                    }));
                }
            } else if self.tool == Tool::Select && self.selection.is_some() {
                self.tool = self.last_draw_tool;
            }
        }

        let esc_pressed = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if esc_pressed {
            if self.text_input.is_some() {
                self.text_input = None;
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
}

pub fn run_viewer(image: DynamicImage) -> Result<(), CaptureError> {
    let mut options = eframe::NativeOptions::default();
    options.viewport = egui::ViewportBuilder::default()
        .with_title("Fireshot (Wayland)")
        .with_app_id("org.fireshot.Fireshot")
        .with_fullscreen(true)
        .with_decorations(false)
        .with_resizable(false)
        .with_always_on_top();
    #[cfg(target_os = "linux")]
    {
        options.event_loop_builder = Some(Box::new(|builder| {
            winit::platform::wayland::EventLoopBuilderExtWayland::with_any_thread(builder, true);
            winit::platform::x11::EventLoopBuilderExtX11::with_any_thread(builder, true);
        }));
    }
    eframe::run_native(
        "Fireshot (Wayland)",
        options,
        Box::new(|_cc| Box::new(EditorApp::new(image))),
    )
    .map_err(|e| CaptureError::Io(e.to_string()))
}
