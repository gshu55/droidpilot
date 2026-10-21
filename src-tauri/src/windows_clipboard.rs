use std::{sync::Mutex, thread, time::Duration};
use windows::{
    core::w,
    Win32::{
        Foundation::{HANDLE, HGLOBAL, HWND},
        System::{
            DataExchange::{
                CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable,
                OpenClipboard, SetClipboardData,
            },
            Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE},
        },
        UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, HWND_MESSAGE, WINDOW_EX_STYLE, WINDOW_STYLE,
        },
    },
};

const CF_UNICODETEXT: u32 = 13;
static ACCESS: Mutex<()> = Mutex::new(());

struct Clipboard(HWND);
impl Clipboard {
    fn open() -> Result<Self, String> {
        // A real, invisible owner is required when EmptyClipboard is followed by SetClipboardData.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!("DroidPilot clipboard"),
                WINDOW_STYLE::default(),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                None,
                None,
            )
        }
        .map_err(|e| e.to_string())?;
        for attempt in 0..10 {
            match unsafe { OpenClipboard(Some(hwnd)) } {
                Ok(()) => return Ok(Self(hwnd)),
                Err(e) if attempt == 9 => {
                    unsafe {
                        let _ = DestroyWindow(hwnd);
                    }
                    return Err(format!("剪贴板被占用，请重试: {e}"));
                }
                Err(_) => thread::sleep(Duration::from_millis(10)),
            }
        }
        unreachable!()
    }
}
impl Drop for Clipboard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseClipboard();
            let _ = DestroyWindow(self.0);
        }
    }
}

pub fn write(text: &str) -> Result<(), String> {
    if text.contains('\0') {
        return Err("剪贴板文本不能包含空字符".into());
    }
    let data: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    let _access = ACCESS.lock().map_err(|e| e.to_string())?;
    let _clipboard = Clipboard::open()?;
    unsafe {
        let memory = GlobalAlloc(GMEM_MOVEABLE, data.len() * 2).map_err(|e| e.to_string())?;
        let pointer = GlobalLock(memory) as *mut u16;
        if pointer.is_null() {
            crate::free_global_memory(memory);
            return Err("锁定剪贴板内存失败".into());
        }
        std::ptr::copy_nonoverlapping(data.as_ptr(), pointer, data.len());
        let _ = GlobalUnlock(memory);
        let result =
            EmptyClipboard().and_then(|_| SetClipboardData(CF_UNICODETEXT, Some(HANDLE(memory.0))));
        if let Err(error) = result {
            crate::free_global_memory(memory);
            return Err(format!("写入剪贴板失败: {error}"));
        }
        // Windows owns memory after SetClipboardData succeeds.
    }
    Ok(())
}

pub fn read() -> Result<String, String> {
    let _access = ACCESS.lock().map_err(|e| e.to_string())?;
    let _clipboard = Clipboard::open()?;
    unsafe {
        if IsClipboardFormatAvailable(CF_UNICODETEXT).is_err() {
            return Ok(String::new());
        }
        let handle = GetClipboardData(CF_UNICODETEXT).map_err(|e| e.to_string())?;
        let memory = HGLOBAL(handle.0);
        let count = GlobalSize(memory) / 2;
        let pointer = GlobalLock(memory) as *const u16;
        if pointer.is_null() {
            return Err("锁定剪贴板内存失败".into());
        }
        let data = std::slice::from_raw_parts(pointer, count);
        let end = data.iter().position(|&c| c == 0).unwrap_or(data.len());
        let text = String::from_utf16_lossy(&data[..end]);
        let _ = GlobalUnlock(memory);
        Ok(text)
    }
}
