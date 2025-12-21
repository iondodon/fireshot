use fireshot_core::CaptureError;
use image::DynamicImage;

pub struct CapturedImage {
    pub image: DynamicImage,
    pub uri: String,
}

pub async fn capture_interactive() -> Result<CapturedImage, CaptureError> {
    // Wayland compositor-independent capture via xdg-desktop-portal screenshot.
    let response = ashpd::desktop::screenshot::Screenshot::request()
        .interactive(true)
        .send()
        .await
        .map_err(|e| CaptureError::Portal(e.to_string()))?
        .response()
        .map_err(|e| CaptureError::Portal(e.to_string()))?;

    let uri = response.uri().to_string();
    let url = url::Url::parse(&uri).map_err(|e| CaptureError::Portal(e.to_string()))?;
    let path = url
        .to_file_path()
        .map_err(|_| CaptureError::Portal("invalid portal file uri".to_string()))?;

    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|e| CaptureError::Io(e.to_string()))?;
    let image = image::load_from_memory(&bytes)
        .map_err(|e| CaptureError::Io(e.to_string()))?;

    Ok(CapturedImage { image, uri })
}

pub async fn probe_screenshot() -> Result<String, CaptureError> {
    let response = ashpd::desktop::screenshot::Screenshot::request()
        .interactive(true)
        .send()
        .await
        .map_err(|e| CaptureError::Portal(e.to_string()))?
        .response()
        .map_err(|e| CaptureError::Portal(e.to_string()))?;

    Ok(response.uri().to_string())
}
