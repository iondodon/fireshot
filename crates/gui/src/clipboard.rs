use std::io::Cursor;

use image::RgbaImage;

pub(crate) fn encode_png(image: &RgbaImage) -> Result<Vec<u8>, image::ImageError> {
    let mut bytes = Vec::new();
    let dyn_img = image::DynamicImage::ImageRgba8(image.clone());
    dyn_img.write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)?;
    Ok(bytes)
}

pub(crate) fn encode_bmp(image: &RgbaImage) -> Result<Vec<u8>, image::ImageError> {
    let mut bytes = Vec::new();
    let dyn_img = image::DynamicImage::ImageRgba8(image.clone());
    dyn_img.write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Bmp)?;
    Ok(bytes)
}

pub(crate) fn try_wl_copy_png(bytes: &[u8]) -> Result<(), String> {
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

pub(crate) fn try_xclip(mime: &str, bytes: &[u8]) -> Result<(), String> {
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

pub(crate) fn is_wayland() -> bool {
    std::env::var("WAYLAND_DISPLAY").is_ok()
}
