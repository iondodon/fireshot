use eframe::egui;
use image::RgbaImage;

use crate::geometry::normalize_rect;

pub(crate) fn apply_pixelate(img: &mut RgbaImage, rect: egui::Rect, block: u32) {
    let rect = normalize_rect(rect);
    let min_x = rect.min.x.floor().max(0.0) as u32;
    let min_y = rect.min.y.floor().max(0.0) as u32;
    let max_x = rect.max.x.ceil().min(img.width() as f32) as u32;
    let max_y = rect.max.y.ceil().min(img.height() as f32) as u32;
    let block = block.max(2);

    let mut y = min_y;
    while y < max_y {
        let mut x = min_x;
        while x < max_x {
            let bx = (x + block).min(max_x);
            let by = (y + block).min(max_y);
            let mut r = 0u64;
            let mut g = 0u64;
            let mut b = 0u64;
            let mut a = 0u64;
            let mut count = 0u64;
            for yy in y..by {
                for xx in x..bx {
                    let p = img.get_pixel(xx, yy);
                    r += p[0] as u64;
                    g += p[1] as u64;
                    b += p[2] as u64;
                    a += p[3] as u64;
                    count += 1;
                }
            }
            if count > 0 {
                let avg = image::Rgba([
                    (r / count) as u8,
                    (g / count) as u8,
                    (b / count) as u8,
                    (a / count) as u8,
                ]);
                for yy in y..by {
                    for xx in x..bx {
                        img.put_pixel(xx, yy, avg);
                    }
                }
            }
            x += block;
        }
        y += block;
    }
}

pub(crate) fn apply_blur(img: &mut RgbaImage, rect: egui::Rect, radius: u32) {
    let rect = normalize_rect(rect);
    let min_x = rect.min.x.floor().max(0.0) as i32;
    let min_y = rect.min.y.floor().max(0.0) as i32;
    let max_x = rect.max.x.ceil().min(img.width() as f32) as i32;
    let max_y = rect.max.y.ceil().min(img.height() as f32) as i32;
    let radius = radius.max(1) as i32;

    let original = img.clone();
    for y in min_y..max_y {
        for x in min_x..max_x {
            let mut r = 0u64;
            let mut g = 0u64;
            let mut b = 0u64;
            let mut a = 0u64;
            let mut count = 0u64;
            let y0 = (y - radius).max(0);
            let y1 = (y + radius).min(max_y - 1);
            let x0 = (x - radius).max(0);
            let x1 = (x + radius).min(max_x - 1);
            for yy in y0..=y1 {
                for xx in x0..=x1 {
                    let p = original.get_pixel(xx as u32, yy as u32);
                    r += p[0] as u64;
                    g += p[1] as u64;
                    b += p[2] as u64;
                    a += p[3] as u64;
                    count += 1;
                }
            }
            if count > 0 {
                let avg = image::Rgba([
                    (r / count) as u8,
                    (g / count) as u8,
                    (b / count) as u8,
                    (a / count) as u8,
                ]);
                img.put_pixel(x as u32, y as u32, avg);
            }
        }
    }
}

pub(crate) fn apply_pixelate_full(img: &mut RgbaImage, block: u32) {
    let rect = egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(img.width() as f32, img.height() as f32),
    );
    apply_pixelate(img, rect, block);
}

pub(crate) fn apply_blur_full(img: &mut RgbaImage, radius: u32) {
    let rect = egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(img.width() as f32, img.height() as f32),
    );
    apply_blur(img, rect, radius);
}
