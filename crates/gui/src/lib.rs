use eframe::egui;
use std::io::Cursor;
use fireshot_core::CaptureError;
use image::{DynamicImage, Rgba, RgbaImage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tool {
    Select,
    Pencil,
    Line,
    Rect,
}

#[derive(Clone, Copy, Debug)]
enum ToolAction {
    Select,
    Pencil,
    Line,
    Rect,
    Undo,
    Copy,
    Save,
    Clear,
}

#[derive(Clone, Copy)]
enum ToolIcon {
    Select,
    Pencil,
    Line,
    Rect,
    Undo,
    Copy,
    Save,
    Clear,
}

#[derive(Debug, Clone)]
struct StrokeShape {
    points: Vec<egui::Pos2>,
    color: egui::Color32,
    size: f32,
}

#[derive(Debug, Clone)]
struct LineShape {
    start: egui::Pos2,
    end: egui::Pos2,
    color: egui::Color32,
    size: f32,
}

#[derive(Debug, Clone)]
struct RectShape {
    start: egui::Pos2,
    end: egui::Pos2,
    color: egui::Color32,
    size: f32,
}

#[derive(Debug, Clone)]
enum Shape {
    Stroke(StrokeShape),
    Line(LineShape),
    Rect(RectShape),
}

#[derive(Debug, Clone, Copy)]
struct SelectionRect {
    rect: egui::Rect,
}

#[derive(Debug, Clone, Copy)]
enum SelectionDrag {
    Creating { start: egui::Pos2 },
    Moving { offset: egui::Vec2 },
    Resizing { corner: SelectionCorner },
}

#[derive(Debug, Clone, Copy)]
enum SelectionCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

struct EditorApp {
    base_image: RgbaImage,
    texture_image: egui::ColorImage,
    texture: Option<egui::TextureHandle>,
    tool: Tool,
    last_draw_tool: Tool,
    color: egui::Color32,
    size: f32,
    shapes: Vec<Shape>,
    active_shape: Option<Shape>,
    selection: Option<SelectionRect>,
    selection_drag: Option<SelectionDrag>,
    status: Option<String>,
    last_image_rect: Option<egui::Rect>,
    last_pixels_per_point: f32,
    tool_button_rects: Vec<egui::Rect>,
    tool_controls_rect: Option<egui::Rect>,
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
            selection: None,
            selection_drag: None,
            status: None,
            last_image_rect: None,
            last_pixels_per_point: 1.0,
            tool_button_rects: Vec::new(),
            tool_controls_rect: None,
        }
    }

    fn image_size(&self) -> egui::Vec2 {
        egui::vec2(self.base_image.width() as f32, self.base_image.height() as f32)
    }

    fn handle_input(&mut self, response: &egui::Response) {
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
                    self.shapes.push(shape);
                }
            }
            return;
        }

        let img_pos = (pointer_pos - response.rect.min) * scale;
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
                Tool::Pencil => Shape::Stroke(StrokeShape {
                    points: vec![img_pos],
                    color: self.color,
                    size: self.size,
                }),
                Tool::Line => Shape::Line(LineShape {
                    start: img_pos,
                    end: img_pos,
                    color: self.color,
                    size: self.size,
                }),
                Tool::Rect => Shape::Rect(RectShape {
                    start: img_pos,
                    end: img_pos,
                    color: self.color,
                    size: self.size,
                }),
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
                    Shape::Rect(rect) => {
                        rect.end = img_pos;
                    }
                }
            }
        } else if pointer.primary_released() {
            if let Some(shape) = self.active_shape.take() {
                self.shapes.push(shape);
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
        false
    }

    fn draw_overlay(&self, response: &egui::Response, painter: &egui::Painter) {
        let scale = response.ctx.pixels_per_point();
        let to_screen = |p: egui::Pos2| {
            response.rect.min + egui::vec2(p.x / scale, p.y / scale)
        };
        if let Some(sel) = self.selection {
            let img_rect = response.rect;
            let sel_rect = egui::Rect::from_two_pos(to_screen(sel.rect.min), to_screen(sel.rect.max));
            let dim_color = egui::Color32::from_rgba_premultiplied(0, 0, 0, 160);

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

            painter.rect_filled(top, 0.0, dim_color);
            painter.rect_filled(bottom, 0.0, dim_color);
            painter.rect_filled(left, 0.0, dim_color);
            painter.rect_filled(right, 0.0, dim_color);

            painter.rect_stroke(sel_rect, 0.0, egui::Stroke::new(1.5, egui::Color32::WHITE));
            draw_handles(painter, sel_rect, 4.0, egui::Color32::WHITE);
            draw_selection_hud(painter, sel_rect, sel.rect, response.rect);
        }
        let draw_shape = |shape: &Shape| match shape {
            Shape::Stroke(stroke) => {
                let points: Vec<egui::Pos2> = stroke.points.iter().copied().map(to_screen).collect();
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
        };

        for shape in &self.shapes {
            draw_shape(shape);
        }
        if let Some(active) = &self.active_shape {
            draw_shape(active);
        }
    }

    fn show_tool_buttons(&mut self, ctx: &egui::Context) {
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
        let positions = layout_tool_buttons(
            sel_rect_screen,
            image_rect,
            button_size,
            spacing,
            8,
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
                            ToolAction::Select => self.tool = Tool::Select,
                            ToolAction::Pencil => self.tool = Tool::Pencil,
                            ToolAction::Line => self.tool = Tool::Line,
                            ToolAction::Rect => self.tool = Tool::Rect,
                            ToolAction::Undo => {
                                self.shapes.pop();
                            }
                            ToolAction::Copy => self.copy_and_close(ctx),
                            ToolAction::Save => self.save_image(),
                            ToolAction::Clear => self.shapes.clear(),
                        }
                    }
                });
        };

        add_tool("Select", ToolAction::Select, ToolIcon::Select, current_tool == Tool::Select);
        add_tool("Pencil", ToolAction::Pencil, ToolIcon::Pencil, current_tool == Tool::Pencil);
        add_tool("Line", ToolAction::Line, ToolIcon::Line, current_tool == Tool::Line);
        add_tool("Rect", ToolAction::Rect, ToolIcon::Rect, current_tool == Tool::Rect);
        add_tool("Undo", ToolAction::Undo, ToolIcon::Undo, false);
        add_tool("Copy", ToolAction::Copy, ToolIcon::Copy, false);
        add_tool("Save", ToolAction::Save, ToolIcon::Save, false);
        add_tool("Clear", ToolAction::Clear, ToolIcon::Clear, false);
    }

    fn show_tool_controls(&mut self, ctx: &egui::Context) {
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
            }
        }
        img
    }

    fn save_image(&mut self) {
        let file = rfd::FileDialog::new()
            .set_file_name("screenshot.png")
            .add_filter("PNG", &["png"])
            .add_filter("JPEG", &["jpg", "jpeg"])
            .save_file();
        let Some(path) = file else {
            return;
        };
        let rendered = self.render_image();
        match rendered.save(&path) {
            Ok(()) => {
                self.status = Some(format!("Saved {}", path.display()));
            }
            Err(err) => {
                self.status = Some(format!("Save failed: {}", err));
            }
        }
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

        self.show_tool_buttons(ctx);
        self.show_tool_controls(ctx);

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

        let undo_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::Z);
        let undo_requested = ctx.input_mut(|i| i.consume_shortcut(&undo_shortcut));
        if undo_requested {
            self.shapes.pop();
        }

        if self.tool != Tool::Select {
            self.last_draw_tool = self.tool;
        }

        let enter_pressed = ctx.input(|i| i.key_pressed(egui::Key::Enter));
        if enter_pressed && self.tool == Tool::Select && self.selection.is_some() {
            self.tool = self.last_draw_tool;
        }

        let esc_pressed = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if esc_pressed {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

pub fn run_viewer(image: DynamicImage) -> Result<(), CaptureError> {
    let mut options = eframe::NativeOptions::default();
    options.viewport = egui::ViewportBuilder::default()
        .with_title("Fireshot (Wayland MVP)")
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
        "Fireshot (Wayland MVP)",
        options,
        Box::new(|_cc| Box::new(EditorApp::new(image))),
    )
    .map_err(|e| CaptureError::Io(e.to_string()))
}

fn draw_line(
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

fn color32_to_rgba(color: egui::Color32) -> Rgba<u8> {
    Rgba([color.r(), color.g(), color.b(), color.a()])
}

fn encode_png(image: &RgbaImage) -> Result<Vec<u8>, image::ImageError> {
    let mut bytes = Vec::new();
    let dyn_img = image::DynamicImage::ImageRgba8(image.clone());
    dyn_img.write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)?;
    Ok(bytes)
}

fn encode_bmp(image: &RgbaImage) -> Result<Vec<u8>, image::ImageError> {
    let mut bytes = Vec::new();
    let dyn_img = image::DynamicImage::ImageRgba8(image.clone());
    dyn_img.write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Bmp)?;
    Ok(bytes)
}

fn try_wl_copy_png(bytes: &[u8]) -> Result<(), String> {
    let mut child = std::process::Command::new("wl-copy")
        .arg("--type")
        .arg("image/png")
        .arg("--foreground")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        std::io::Write::write_all(&mut stdin, bytes).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn try_xclip(mime: &str, bytes: &[u8]) -> Result<(), String> {
    let mut child = std::process::Command::new("xclip")
        .arg("-selection")
        .arg("clipboard")
        .arg("-t")
        .arg(mime)
        .arg("-i")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        std::io::Write::write_all(&mut stdin, bytes).map_err(|e| e.to_string())?;
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("xclip exited with {}", status))
    }
}

fn normalize_rect(rect: egui::Rect) -> egui::Rect {
    let min = egui::pos2(rect.min.x.min(rect.max.x), rect.min.y.min(rect.max.y));
    let max = egui::pos2(rect.min.x.max(rect.max.x), rect.min.y.max(rect.max.y));
    egui::Rect::from_min_max(min, max)
}

fn hit_corner(rect: egui::Rect, pos: egui::Pos2, radius: f32) -> Option<SelectionCorner> {
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

fn draw_handles(painter: &egui::Painter, rect: egui::Rect, radius: f32, color: egui::Color32) {
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

fn draw_selection_hud(
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

fn selection_screen_rect(
    sel_rect_image: egui::Rect,
    image_rect: egui::Rect,
    scale: f32,
) -> egui::Rect {
    let min = image_rect.min + egui::vec2(sel_rect_image.min.x / scale, sel_rect_image.min.y / scale);
    let max = image_rect.min + egui::vec2(sel_rect_image.max.x / scale, sel_rect_image.max.y / scale);
    egui::Rect::from_min_max(min, max)
}

fn paint_tool_icon(painter: &egui::Painter, rect: egui::Rect, icon: ToolIcon, color: egui::Color32) {
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
        ToolIcon::Rect => {
            painter.rect_stroke(inner, 2.0, stroke);
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

fn layout_tool_buttons(
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
            let row =
                row_positions(selection.center().x, row_y, count_here, button_size, spacing, bounds);
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

fn row_positions(
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

fn col_positions(
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

fn crop_image(img: &RgbaImage, rect: egui::Rect) -> RgbaImage {
    let width = img.width() as f32;
    let height = img.height() as f32;
    let min_x = rect.min.x.floor().clamp(0.0, width) as u32;
    let min_y = rect.min.y.floor().clamp(0.0, height) as u32;
    let max_x = rect.max.x.ceil().clamp(0.0, width) as u32;
    let max_y = rect.max.y.ceil().clamp(0.0, height) as u32;
    let out_w = max_x.saturating_sub(min_x);
    let out_h = max_y.saturating_sub(min_y);
    if out_w == 0 || out_h == 0 {
        return img.clone();
    }

    let mut out = RgbaImage::new(out_w, out_h);
    for y in 0..out_h {
        for x in 0..out_w {
            let px = img.get_pixel(min_x + x, min_y + y);
            out.put_pixel(x, y, *px);
        }
    }
    out
}

fn is_wayland() -> bool {
    std::env::var("WAYLAND_DISPLAY").is_ok()
}
