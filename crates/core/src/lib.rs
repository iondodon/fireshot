use bitflags::bitflags;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CaptureMode {
    Graphical,
    Fullscreen,
    Screen,
}

bitflags! {
    #[derive(Debug, Clone, Copy, Serialize, Deserialize)]
    pub struct ExportTask: u32 {
        const NONE = 0;
        const COPY = 1 << 0;
        const SAVE = 1 << 1;
        const PIN = 1 << 2;
        const UPLOAD = 1 << 3;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureRequest {
    pub mode: CaptureMode,
    pub delay_ms: u64,
    pub tasks: ExportTask,
    pub save_path: Option<String>,
}

impl Default for CaptureRequest {
    fn default() -> Self {
        Self {
            mode: CaptureMode::Graphical,
            delay_ms: 0,
            tasks: ExportTask::NONE,
            save_path: None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("portal error: {0}")]
    Portal(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
}
