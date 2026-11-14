use super::Platform;
use crate::frame::{Image, PixelRect};
use crate::{Error, Result};
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct StubPlatform;

impl Platform for StubPlatform {
    fn supports_capture(&self) -> bool {
        false
    }
    fn supports_input(&self) -> bool {
        false
    }
    fn supports_tts(&self) -> bool {
        false
    }
    fn process_running(&self, _name: &str) -> Result<bool> {
        Ok(false)
    }
    fn is_foreground(&self, _process_name: &str) -> Result<bool> {
        Ok(false)
    }
    fn monitor_size(&self) -> Result<(u32, u32)> {
        Err(Error::Platform(
            "screen metrics are Windows-only in this build".into(),
        ))
    }
    fn capture_rect(&self, _rect: PixelRect) -> Result<Image> {
        Err(Error::Platform(
            "screen capture is Windows-only in this build".into(),
        ))
    }
    fn send_chat(
        &self,
        _text: &str,
        _modifier: Option<u16>,
        _chat_key: u16,
        _key_delay: Duration,
        _open_delay: Duration,
    ) -> Result<()> {
        Err(Error::Platform(
            "typing into the game is Windows-only in this build".into(),
        ))
    }
    fn speak(&self, _text: &str) -> Result<()> {
        Ok(())
    }
    fn hotkey_is_down(&self, _vk: u16) -> Result<bool> {
        Ok(false)
    }
}
