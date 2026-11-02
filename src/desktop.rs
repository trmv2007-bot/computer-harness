use anyhow::Result;
use serde_json::{json, Value};

const MAX_VISIBLE_WINDOWS: usize = 200;
const MAX_TITLE_CHARS: usize = 512;
const MAX_CLASS_CHARS: usize = 256;

#[cfg(windows)]
mod windows {
    use super::*;
    use windows_sys::Win32::{
        Foundation::{BOOL, HWND, LPARAM},
        UI::WindowsAndMessaging::{
            EnumWindows, GetClassNameW, GetWindowTextLengthW, GetWindowTextW,
            GetWindowThreadProcessId, IsWindowVisible,
        },
    };

    struct Enumeration {
        windows: Vec<Value>,
        truncated: bool,
    }

    unsafe extern "system" fn enumerate_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
        if lparam == 0 {
            return 0;
        }

        // SAFETY: list_windows passes a live pointer to Enumeration for the
        // duration of the synchronous EnumWindows call.
        let state = unsafe { &mut *(lparam as *mut Enumeration) };

        if unsafe { IsWindowVisible(hwnd) } == 0 {
            return 1;
        }

        let reported_title_len = unsafe { GetWindowTextLengthW(hwnd) };
        if reported_title_len <= 0 {
            return 1;
        }

        let title_capacity = (reported_title_len as usize).min(MAX_TITLE_CHARS);
        let mut title_buffer = vec![0u16; title_capacity + 1];
        let title_len = unsafe {
            GetWindowTextW(
                hwnd,
                title_buffer.as_mut_ptr(),
                title_buffer.len() as i32,
            )
        };
        if title_len <= 0 {
            return 1;
        }
        let title = String::from_utf16_lossy(&title_buffer[..title_len as usize]);
        if title.trim().is_empty() {
            return 1;
        }

        let mut class_buffer = vec![0u16; MAX_CLASS_CHARS + 1];
        let class_len = unsafe {
            GetClassNameW(
                hwnd,
                class_buffer.as_mut_ptr(),
                class_buffer.len() as i32,
            )
        };
        let class_name = if class_len > 0 {
            String::from_utf16_lossy(&class_buffer[..class_len as usize])
        } else {
            String::new()
        };

        let mut process_id = 0u32;
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut process_id);
        }

        if state.windows.len() >= MAX_VISIBLE_WINDOWS {
            state.truncated = true;
            return 0;
        }

        state.windows.push(json!({
            "title": title,
            "class_name": class_name,
            "process_id": process_id
        }));
        1
    }

    pub(super) fn list_windows() -> Result<Value> {
        let mut state = Enumeration {
            windows: Vec::with_capacity(MAX_VISIBLE_WINDOWS),
            truncated: false,
        };
        // SAFETY: callback only accesses the live state pointer passed as LPARAM.
        let enumerated = unsafe {
            EnumWindows(
                Some(enumerate_window),
                &mut state as *mut Enumeration as LPARAM,
            )
        };
        anyhow::ensure!(
            enumerated != 0 || state.truncated,
            "Windows could not enumerate top-level windows"
        );

        Ok(json!({
            "platform": "windows",
            "window_count": state.windows.len(),
            "truncated": state.truncated,
            "windows": state.windows
        }))
    }
}

#[cfg(windows)]
pub fn list_windows() -> Result<Value> {
    windows::list_windows()
}

#[cfg(not(windows))]
pub fn list_windows() -> Result<Value> {
    anyhow::bail!("desktop window enumeration is available only on Windows")
}
