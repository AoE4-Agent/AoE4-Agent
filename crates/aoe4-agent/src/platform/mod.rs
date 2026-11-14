//! Operating-system edges: process watch, screen grab, key input, speech.
//!
//! The live implementations are Windows-only. Other targets get [`StubPlatform`],
//! which refuses capture and input so the crate still builds and the pure
//! logic tests still run.

use crate::frame::{Image, PixelRect};
use crate::Result;
use std::time::Duration;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::WindowsPlatform as HostPlatform;

#[cfg(not(windows))]
mod stub;
#[cfg(not(windows))]
pub use stub::StubPlatform as HostPlatform;

pub trait Platform {
    fn prepare(&self) {}
    fn supports_capture(&self) -> bool;
    fn supports_input(&self) -> bool;
    fn supports_tts(&self) -> bool;
    fn process_running(&self, name: &str) -> Result<bool>;
    fn is_foreground(&self, process_name: &str) -> Result<bool>;
    fn monitor_size(&self) -> Result<(u32, u32)>;
    fn capture_rect(&self, rect: PixelRect) -> Result<Image>;
    fn send_chat(
        &self,
        text: &str,
        // Held around `chat_key` when team chat is selected. `None` sends to all chat.
        modifier: Option<u16>,
        chat_key: u16,
        key_delay: Duration,
        open_delay: Duration,
    ) -> Result<()>;
    fn speak(&self, text: &str) -> Result<()>;
    fn hotkey_is_down(&self, vk: u16) -> Result<bool>;
}
