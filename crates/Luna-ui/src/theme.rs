pub const TAB_BAR_HEIGHT: f32 = 32.0;

pub const TAB_ACTIVE_BG: [f32; 4] = [181.0 / 255.0, 48.0 / 255.0, 126.0 / 255.0, 1.0]; // #b5307e
pub const TAB_INACTIVE_BG: [f32; 4] = [106.0 / 255.0, 42.0 / 255.0, 152.0 / 255.0, 1.0]; // #6a2a98
pub const TAB_BAR_BG: [f32; 4] = [106.0 / 255.0, 42.0 / 255.0, 152.0 / 255.0, 1.0]; // #6a2a98
pub const TAB_HOVER_BG: [f32; 4] = [1.0, 0.239, 0.58, 0.133]; // #ff3d9422
pub const TAB_TEXT: [f32; 4] = [1.0, 1.0, 1.0, 1.0]; // #ffffff
pub const TAB_TEXT_INACTIVE: [f32; 4] = [0.8, 0.8, 0.8, 1.0]; // #cccccc
pub const TAB_BUTTON_TEXT: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
pub const TAB_SEPARATOR: [f32; 4] = [63.0 / 255.0, 28.0 / 255.0, 109.0 / 255.0, 1.0]; // #3f1c6d
pub const BG_COLOR: [f32; 4] = [33.0 / 255.0, 11.0 / 255.0, 75.0 / 255.0, 1.0]; // #210b4b
pub const PANEL_ACTIVE_BORDER: [f32; 4] = [181.0 / 255.0, 48.0 / 255.0, 126.0 / 255.0, 1.0]; // #b5307e
pub const PANEL_INACTIVE_BORDER: [f32; 4] = [63.0 / 255.0, 28.0 / 255.0, 109.0 / 255.0, 1.0]; // #3f1c6d
pub const PANEL_DIVIDER: [f32; 4] = [106.0 / 255.0, 42.0 / 255.0, 152.0 / 255.0, 1.0]; // #6a2a98

pub struct Theme {}

impl Theme {
    pub fn new() -> Self {
        Self {}
    }
}
