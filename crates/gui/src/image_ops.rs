use eframe::egui;
use image::RgbaImage;

pub(crate) fn rect_to_u32(img: &RgbaImage, rect: egui::Rect) -> Option<(u32, u32, u32, u32)> {
    let width = img.width() as f32;
    let height = img.height() as f32;
    let min_x = rect.min.x.floor().clamp(0.0, width) as u32;
    let min_y = rect.min.y.floor().clamp(0.0, height) as u32;
    let max_x = rect.max.x.ceil().clamp(0.0, width) as u32;
    let max_y = rect.max.y.ceil().clamp(0.0, height) as u32;
    if max_x <= min_x || max_y <= min_y {
        return None;
    }
    Some((min_x, min_y, max_x, max_y))
}

pub(crate) fn crop_image_exact(img: &RgbaImage, rect: egui::Rect) -> Option<RgbaImage> {
    let (min_x, min_y, max_x, max_y) = rect_to_u32(img, rect)?;
    let out_w = max_x - min_x;
    let out_h = max_y - min_y;
    if out_w == 0 || out_h == 0 {
        return None;
    }
    let mut out = RgbaImage::new(out_w, out_h);
    for y in 0..out_h {
        for x in 0..out_w {
            let px = img.get_pixel(min_x + x, min_y + y);
            out.put_pixel(x, y, *px);
        }
    }
    Some(out)
}

pub(crate) fn crop_image(img: &RgbaImage, rect: egui::Rect) -> RgbaImage {
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
