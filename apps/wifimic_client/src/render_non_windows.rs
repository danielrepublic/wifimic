use wifimic_protocol::AudioFrame;

use super::{RenderConfig, RenderError};

/// Placeholder renderer so non-Windows workspace builds retain the same seam.
pub struct Renderer;

impl Renderer {
    pub fn open(_config: RenderConfig) -> Result<Self, RenderError> {
        Err(RenderError::UnsupportedPlatform)
    }

    pub fn render_frame(&self, _frame: &AudioFrame) -> Result<(), RenderError> {
        Err(RenderError::UnsupportedPlatform)
    }

    pub fn stop(&mut self) -> Result<(), RenderError> {
        Err(RenderError::UnsupportedPlatform)
    }
}

pub fn enumerate_render_endpoints() -> Result<Vec<String>, RenderError> {
    Err(RenderError::UnsupportedPlatform)
}
