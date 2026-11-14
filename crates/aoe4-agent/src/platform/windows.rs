//! Win32 process watch, GDI screen copy, `SendInput` chat, and SAPI speech.
//!
//! GDI `BitBlt` (not DXGI desktop duplication) is the 0.1.0 capture path
//! because it links with both the MSVC toolchain and the mingw cross
//! compiler. It sees borderless and windowed frames. Exclusive fullscreen
//! can come back black; Age of Empires IV should stay on borderless.
//!
//! # Safety
//!
//! Every `unsafe` block is a Win32 call. Handles created here are released
//! by `Drop` guards, including on the error paths.

#![allow(unsafe_code)]

use super::Platform;
use crate::frame::{Image, PixelRect};
use crate::{Error, Result};
use std::sync::Once;
use std::time::Duration;
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HDC,
    SRCCOPY,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, VIRTUAL_KEY,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetSystemMetrics, GetWindowThreadProcessId, SetProcessDPIAware,
    SM_CXSCREEN, SM_CYSCREEN,
};

#[derive(Debug, Clone, Copy)]
pub struct WindowsPlatform;

impl Platform for WindowsPlatform {
    fn prepare(&self) {
        ensure_dpi();
    }

    fn supports_capture(&self) -> bool {
        true
    }
    fn supports_input(&self) -> bool {
        true
    }
    fn supports_tts(&self) -> bool {
        true
    }

    fn process_running(&self, name: &str) -> Result<bool> {
        Ok(!process_ids(name)?.is_empty())
    }

    fn is_foreground(&self, process_name: &str) -> Result<bool> {
        let ids = process_ids(process_name)?;
        if ids.is_empty() {
            return Ok(false);
        }
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.is_null() {
            return Ok(false);
        }
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        Ok(ids.contains(&pid))
    }

    fn monitor_size(&self) -> Result<(u32, u32)> {
        ensure_dpi();
        let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        if width <= 0 || height <= 0 {
            return Err(Error::Platform(format!(
                "primary monitor size {width}x{height} is unusable"
            )));
        }
        Ok((width as u32, height as u32))
    }

    fn capture_rect(&self, rect: PixelRect) -> Result<Image> {
        ensure_dpi();
        capture_gdi(rect)
    }

    fn send_chat(
        &self,
        text: &str,
        modifier: Option<u16>,
        chat_key: u16,
        key_delay: Duration,
        open_delay: Duration,
    ) -> Result<()> {
        if text.is_empty() {
            return Err(Error::Platform(
                "refusing to send an empty chat line".into(),
            ));
        }
        // The chord comes from config (`chat_key`, and `team_modifier` when
        // chat is `team`). The shipped values are Enter, and Shift+Enter.
        let modified = modifier.is_some();
        let modifier = modifier.unwrap_or(0);
        tap(modifier, chat_key, modified)?;
        std::thread::sleep(open_delay);
        for ch in text.chars() {
            if ch.is_control() {
                continue;
            }
            type_char(ch)?;
            if !key_delay.is_zero() {
                std::thread::sleep(key_delay);
            }
        }
        tap(modifier, chat_key, modified)?;
        Ok(())
    }

    fn speak(&self, text: &str) -> Result<()> {
        speak_sapi(text)
    }

    fn hotkey_is_down(&self, vk: u16) -> Result<bool> {
        // High bit set means the key is down. This is a poll, not a hook.
        let state = unsafe { GetAsyncKeyState(vk as i32) };
        Ok(state < 0)
    }
}

fn ensure_dpi() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        let ok = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        if ok == 0 {
            SetProcessDPIAware();
        }
    });
}

fn process_ids(name: &str) -> Result<Vec<u32>> {
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE || snap.is_null() {
            return Err(last_error("CreateToolhelp32Snapshot"));
        }
        let _guard = Snapshot(snap);
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        if Process32FirstW(snap, &mut entry) == 0 {
            return Ok(Vec::new());
        }
        let mut ids = Vec::new();
        loop {
            let exe = wide_to_string(&entry.szExeFile);
            if exe.eq_ignore_ascii_case(name) {
                ids.push(entry.th32ProcessID);
            }
            if Process32NextW(snap, &mut entry) == 0 {
                break;
            }
        }
        Ok(ids)
    }
}

struct Snapshot(HANDLE);
impl Drop for Snapshot {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

fn capture_gdi(rect: PixelRect) -> Result<Image> {
    let width = rect.width as usize;
    let height = rect.height as usize;
    if width == 0 || height == 0 || width > 10_000 || height > 10_000 {
        return Err(Error::Platform(format!(
            "capture rectangle {width}x{height} is outside 1..=10000"
        )));
    }
    unsafe {
        let screen = GetDC(std::ptr::null_mut());
        if screen.is_null() {
            return Err(Error::Platform("GetDC failed".into()));
        }
        let _screen = ScreenDc(screen);
        let memory = CreateCompatibleDC(screen);
        if memory.is_null() {
            return Err(last_error("CreateCompatibleDC"));
        }
        let _memory = MemoryDc(memory);
        let bitmap = CreateCompatibleBitmap(screen, rect.width as i32, rect.height as i32);
        if bitmap.is_null() {
            return Err(last_error("CreateCompatibleBitmap"));
        }
        let _bitmap = Bitmap(bitmap);
        let previous = SelectObject(memory, bitmap as _);
        if previous.is_null() {
            return Err(last_error("SelectObject"));
        }
        let copied = BitBlt(
            memory,
            0,
            0,
            rect.width as i32,
            rect.height as i32,
            screen,
            rect.x as i32,
            rect.y as i32,
            SRCCOPY,
        );
        SelectObject(memory, previous);
        if copied == 0 {
            return Err(last_error("BitBlt"));
        }
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = rect.width as i32;
        info.bmiHeader.biHeight = rect.height as i32;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let byte_len = width
            .checked_mul(height)
            .and_then(|n| n.checked_mul(4))
            .ok_or_else(|| Error::Platform("capture buffer overflow".into()))?;
        let mut bgra = vec![0u8; byte_len];
        let rows = GetDIBits(
            memory,
            bitmap,
            0,
            rect.height,
            bgra.as_mut_ptr() as *mut _,
            &mut info,
            DIB_RGB_COLORS,
        );
        if rows == 0 {
            return Err(last_error("GetDIBits"));
        }
        // Positive biHeight is bottom-up. Flip into top-down RGB.
        let mut rgb = Vec::with_capacity(width * height * 3);
        for y in 0..height {
            let src_y = height - 1 - y;
            let row = &bgra[src_y * width * 4..(src_y + 1) * width * 4];
            for px in row.as_chunks::<4>().0 {
                rgb.push(px[2]);
                rgb.push(px[1]);
                rgb.push(px[0]);
            }
        }
        Image::new(width, height, rgb).map_err(|err| Error::Platform(err.to_string()))
    }
}

struct ScreenDc(HDC);
impl Drop for ScreenDc {
    fn drop(&mut self) {
        unsafe {
            ReleaseDC(std::ptr::null_mut(), self.0);
        }
    }
}
struct MemoryDc(HDC);
impl Drop for MemoryDc {
    fn drop(&mut self) {
        unsafe {
            DeleteDC(self.0);
        }
    }
}
struct Bitmap(HBITMAP);
impl Drop for Bitmap {
    fn drop(&mut self) {
        unsafe {
            DeleteObject(self.0 as _);
        }
    }
}

fn tap(modifier: VIRTUAL_KEY, key: VIRTUAL_KEY, with_modifier: bool) -> Result<()> {
    if with_modifier {
        key_event(modifier, 0, 0)?;
    }
    key_event(key, 0, 0)?;
    key_event(key, 0, KEYEVENTF_KEYUP)?;
    if with_modifier {
        key_event(modifier, 0, KEYEVENTF_KEYUP)?;
    }
    Ok(())
}

fn type_char(ch: char) -> Result<()> {
    let mut buf = [0u16; 2];
    let encoded = ch.encode_utf16(&mut buf);
    if encoded.len() != 1 {
        return Ok(());
    }
    key_event(0, encoded[0], KEYEVENTF_UNICODE)?;
    key_event(0, encoded[0], KEYEVENTF_UNICODE | KEYEVENTF_KEYUP)?;
    Ok(())
}

fn key_event(vk: VIRTUAL_KEY, scan: u16, flags: u32) -> Result<()> {
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let sent = unsafe { SendInput(1, &input, std::mem::size_of::<INPUT>() as i32) };
    if sent != 1 {
        return Err(last_error("SendInput"));
    }
    Ok(())
}

fn speak_sapi(text: &str) -> Result<()> {
    let spoken: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let spoken = spoken.replace("'@", "' @");
    let script = format!(
        "$ErrorActionPreference = 'Stop'\r\n\
         Add-Type -AssemblyName System.Speech\r\n\
         $synth = New-Object System.Speech.Synthesis.SpeechSynthesizer\r\n\
         $synth.Speak(@'\r\n{spoken}\r\n'@)\r\n"
    );
    let path = std::env::temp_dir().join(format!("aoe4-agent-say-{}.ps1", std::process::id()));
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend(script.as_bytes());
    std::fs::write(&path, bytes).map_err(|source| Error::io(&path, source))?;
    let _remove = RemoveFile(path.clone());
    let mut command = std::process::Command::new("powershell.exe");
    command
        .args(["-NoProfile", "-NonInteractive", "-File"])
        .arg(&path)
        .stdin(std::process::Stdio::null());
    // CREATE_NO_WINDOW. Keeps the speech helper from flashing a console.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let output = command
        .output()
        .map_err(|source| Error::io(std::path::Path::new("powershell.exe"), source))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(Error::Platform(format!(
            "SAPI speech failed: {}",
            stderr.trim()
        )))
    }
}

struct RemoveFile(std::path::PathBuf);
impl Drop for RemoveFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn wide_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

fn last_error(op: &str) -> Error {
    let code = unsafe { GetLastError() };
    Error::Platform(format!("{op} failed (win32 {code})"))
}
