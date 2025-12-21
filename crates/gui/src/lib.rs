use eframe::egui;
use std::io::Cursor;
use fireshot_core::CaptureError;
use image::{DynamicImage, Rgba, RgbaImage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tool {
    Pencil,
    Line,
    Rect,
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

struct EditorApp {
    base_image: RgbaImage,
    texture_image: egui::ColorImage,
    texture: Option<egui::TextureHandle>,
    tool: Tool,
    color: egui::Color32,
    size: f32,
    shapes: Vec<Shape>,
    active_shape: Option<Shape>,
    status: Option<String>,
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
            tool: Tool::Pencil,
            color: egui::Color32::from_rgb(255, 0, 0),
            size: 3.0,
            shapes: Vec::new(),
            active_shape: None,
            status: None,
        }
    }

    fn image_size(&self) -> egui::Vec2 {
        egui::vec2(self.base_image.width() as f32, self.base_image.height() as f32)
    }

    fn handle_input(&mut self, response: &egui::Response) {
        let pointer = response.ctx.input(|i| i.pointer.clone());
        let Some(pointer_pos) = pointer.hover_pos() else {
            return;
        };
        if !response.rect.contains(pointer_pos) {
            if pointer.any_released() {
                if let Some(shape) = self.active_shape.take() {
                    self.shapes.push(shape);
                }
            }
            return;
        }

        let img_pos = pointer_pos - response.rect.min;
        let img_pos = egui::pos2(
            img_pos.x.clamp(0.0, self.image_size().x),
            img_pos.y.clamp(0.0, self.image_size().y),
        );

        if pointer.primary_pressed() {
            self.active_shape = Some(match self.tool {
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

    fn draw_overlay(&self, response: &egui::Response, painter: &egui::Painter) {
        let to_screen = |p: egui::Pos2| p + response.rect.min.to_vec2();
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

    fn render_image(&self) -> RgbaImage {
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

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Fireshot");
                ui.separator();
                ui.selectable_value(&mut self.tool, Tool::Pencil, "Pencil");
                ui.selectable_value(&mut self.tool, Tool::Line, "Line");
                ui.selectable_value(&mut self.tool, Tool::Rect, "Rect");
                ui.separator();
                ui.color_edit_button_srgba(&mut self.color);
                ui.add(egui::Slider::new(&mut self.size, 1.0..=20.0).text("Size"));
                if ui.button("Undo").clicked() {
                    self.shapes.pop();
                }
                if ui.button("Clear").clicked() {
                    self.shapes.clear();
                }
                if ui.button("Copy & Close").clicked() {
                    self.copy_and_close(ctx);
                }
                if ui.button("Save As").clicked() {
                    self.save_image();
                }
            });
            if let Some(status) = &self.status {
                ui.label(status);
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::both().show(ui, |ui| {
                if let Some(texture) = &self.texture {
                    let avail = ui.available_size();
                    let image_size = texture.size_vec2();
                    let offset = (avail - image_size) * 0.5;
                    let offset = egui::vec2(offset.x.max(0.0), offset.y.max(0.0));
                    if offset.y > 0.0 {
                        ui.add_space(offset.y);
                    }
                    let response = ui.horizontal(|ui| {
                        if offset.x > 0.0 {
                            ui.add_space(offset.x);
                        }
                        ui.image(texture)
                    }).inner;
                    let painter = ui.painter();
                    self.handle_input(&response);
                    self.draw_overlay(&response, painter);
                }
            });
        });

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
    }
}

pub fn run_viewer(image: DynamicImage) -> Result<(), CaptureError> {
    let options = eframe::NativeOptions::default();
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

fn is_wayland() -> bool {
    std::env::var("WAYLAND_DISPLAY").is_ok()
}
