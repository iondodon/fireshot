use eframe::egui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tool {
    Select,
    Pencil,
    Line,
    Arrow,
    Rect,
    Circle,
    Marker,
    MarkerLine,
    CircleCount,
    Text,
    Pixelate,
    Blur,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ToolAction {
    Tool(Tool),
    Undo,
    Copy,
    Save,
    Clear,
}

#[derive(Clone, Copy)]
pub(crate) enum ToolIcon {
    Select,
    Pencil,
    Line,
    Arrow,
    Rect,
    Circle,
    Marker,
    MarkerLine,
    CircleCount,
    Text,
    Pixelate,
    Blur,
    Undo,
    Copy,
    Save,
    Clear,
}

#[derive(Debug, Clone)]
pub(crate) struct StrokeShape {
    pub(crate) points: Vec<egui::Pos2>,
    pub(crate) color: egui::Color32,
    pub(crate) size: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct LineShape {
    pub(crate) start: egui::Pos2,
    pub(crate) end: egui::Pos2,
    pub(crate) color: egui::Color32,
    pub(crate) size: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct RectShape {
    pub(crate) start: egui::Pos2,
    pub(crate) end: egui::Pos2,
    pub(crate) color: egui::Color32,
    pub(crate) size: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct CircleShape {
    pub(crate) start: egui::Pos2,
    pub(crate) end: egui::Pos2,
    pub(crate) color: egui::Color32,
    pub(crate) size: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct ArrowShape {
    pub(crate) start: egui::Pos2,
    pub(crate) end: egui::Pos2,
    pub(crate) color: egui::Color32,
    pub(crate) size: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct CircleCountShape {
    pub(crate) center: egui::Pos2,
    pub(crate) pointer: egui::Pos2,
    pub(crate) color: egui::Color32,
    pub(crate) size: f32,
    pub(crate) count: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct TextShape {
    pub(crate) pos: egui::Pos2,
    pub(crate) text: String,
    pub(crate) color: egui::Color32,
    pub(crate) size: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct EffectShape {
    pub(crate) start: egui::Pos2,
    pub(crate) end: egui::Pos2,
    pub(crate) size: f32,
    pub(crate) kind: EffectKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EffectKind {
    Pixelate,
    Blur,
}

#[derive(Debug, Clone)]
pub(crate) enum Shape {
    Stroke(StrokeShape),
    Line(LineShape),
    Arrow(ArrowShape),
    Rect(RectShape),
    Circle(CircleShape),
    CircleCount(CircleCountShape),
    Text(TextShape),
    Effect(EffectShape),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SelectionRect {
    pub(crate) rect: egui::Rect,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum SelectionDrag {
    Creating { start: egui::Pos2 },
    Moving { offset: egui::Vec2 },
    Resizing { corner: SelectionCorner },
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum SelectionCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

pub(crate) const FILE_DIALOG_SIZE: egui::Vec2 = egui::Vec2 { x: 720.0, y: 480.0 };

pub(crate) struct TextInput {
    pub(crate) pos: egui::Pos2,
    pub(crate) text: String,
}

pub(crate) struct EffectPreview {
    pub(crate) rect: [u32; 4],
    pub(crate) kind: EffectKind,
    pub(crate) size: u32,
    pub(crate) shapes_version: u64,
    pub(crate) texture: egui::TextureHandle,
}
