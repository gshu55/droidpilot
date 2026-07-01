use base64::{engine::general_purpose, Engine as _};
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::process::Output;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::Emitter;
use tauri::Manager;

mod scrcpy;

// ── 内嵌 ADB 二进制 ────────────────────────────────────────────────────────────
// 将各平台 adb 放入 src-tauri/binaries/<platform>/ 目录后重新编译，
// 程序将在运行时自动解压到临时目录并使用，无需系统环境中安装 adb。

#[cfg(all(has_embedded_adb, target_os = "windows"))]
static ADB_BINARY: &[u8] = include_bytes!("../binaries/windows/adb.exe");

#[cfg(all(has_embedded_adb, target_os = "windows"))]
static ADB_WIN_API_DLL: &[u8] = include_bytes!("../binaries/windows/AdbWinApi.dll");

#[cfg(all(has_embedded_adb, target_os = "windows"))]
static ADB_WIN_USB_DLL: &[u8] = include_bytes!("../binaries/windows/AdbWinUsbApi.dll");

#[cfg(all(has_embedded_adb, target_os = "linux"))]
static ADB_BINARY: &[u8] = include_bytes!("../binaries/linux/adb");

#[cfg(all(has_embedded_adb, target_os = "macos"))]
static ADB_BINARY: &[u8] = include_bytes!("../binaries/macos/adb");

#[cfg(has_embedded_adb)]
fn embedded_bytes_hash(parts: &[&[u8]]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for part in parts {
        for byte in *part {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    hash
}

#[cfg(has_embedded_adb)]
fn write_embedded_file_if_needed(path: &Path, bytes: &[u8], label: &str) {
    if matches!(fs::read(path), Ok(existing) if existing == bytes) {
        return;
    }

    fs::write(path, bytes).unwrap_or_else(|err| {
        panic!("{label}: {err}");
    });
}

/// 返回 adb 可执行文件的路径。
/// 若编译时已内嵌二进制，则首次调用时解压到临时目录；否则返回系统 PATH 中的 "adb"。
fn get_adb_path() -> &'static PathBuf {
    static ADB_PATH: OnceLock<PathBuf> = OnceLock::new();
    ADB_PATH.get_or_init(|| {
        #[cfg(has_embedded_adb)]
        {
            #[cfg(target_os = "windows")]
            let embedded_hash =
                embedded_bytes_hash(&[ADB_BINARY, ADB_WIN_API_DLL, ADB_WIN_USB_DLL]);
            #[cfg(not(target_os = "windows"))]
            let embedded_hash = embedded_bytes_hash(&[ADB_BINARY]);

            let dir = std::env::temp_dir()
                .join("droidpilot")
                .join(format!("adb-{embedded_hash:016x}"));
            std::fs::create_dir_all(&dir).expect("创建内嵌 adb 目录失败");

            #[cfg(target_os = "windows")]
            let path = dir.join("adb.exe");
            #[cfg(not(target_os = "windows"))]
            let path = dir.join("adb");

            write_embedded_file_if_needed(&path, ADB_BINARY, "提取内嵌 adb 失败");

            #[cfg(target_os = "windows")]
            {
                write_embedded_file_if_needed(
                    &dir.join("AdbWinApi.dll"),
                    ADB_WIN_API_DLL,
                    "提取 AdbWinApi.dll 失败",
                );
                write_embedded_file_if_needed(
                    &dir.join("AdbWinUsbApi.dll"),
                    ADB_WIN_USB_DLL,
                    "提取 AdbWinUsbApi.dll 失败",
                );
            }

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(meta) = std::fs::metadata(&path) {
                    let mut perm = meta.permissions();
                    perm.set_mode(0o755);
                    let _ = std::fs::set_permissions(&path, perm);
                }
            }

            #[cfg(target_os = "macos")]
            {
                let sign_result = Command::new("/usr/bin/codesign")
                    .args(["--force", "--sign", "-"])
                    .arg(&path)
                    .status();

                if !matches!(sign_result, Ok(status) if status.success()) {
                    eprintln!("内嵌 adb ad-hoc 签名失败: {sign_result:?}");
                }
            }

            path
        }
        #[cfg(not(has_embedded_adb))]
        {
            PathBuf::from("adb")
        }
    })
}

#[cfg(target_os = "windows")]
fn apply_windows_window_frame<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>, is_dark: bool) {
    use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWINDOWATTRIBUTE};

    const DWMWA_BORDER_COLOR: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(34);
    const DWMWA_CAPTION_COLOR: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(35);
    const DWMWA_TEXT_COLOR: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(36);
    const DARK_FRAME: u32 = 0x20110B;
    const LIGHT_FRAME: u32 = 0xFCFAF8;
    const DARK_TEXT: u32 = 0xF7EDE5;
    const LIGHT_TEXT: u32 = 0x2A170F;
    let frame_color = if is_dark { DARK_FRAME } else { LIGHT_FRAME };
    let text_color = if is_dark { DARK_TEXT } else { LIGHT_TEXT };

    if let Ok(hwnd) = window.hwnd() {
        unsafe {
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR,
                &frame_color as *const u32 as *const std::ffi::c_void,
                std::mem::size_of::<u32>() as u32,
            );
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_CAPTION_COLOR,
                &frame_color as *const u32 as *const std::ffi::c_void,
                std::mem::size_of::<u32>() as u32,
            );
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_TEXT_COLOR,
                &text_color as *const u32 as *const std::ffi::c_void,
                std::mem::size_of::<u32>() as u32,
            );
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn apply_windows_window_frame<R: tauri::Runtime>(
    _window: &tauri::WebviewWindow<R>,
    _is_dark: bool,
) {
}

#[tauri::command]
fn set_window_frame_theme(window: tauri::WebviewWindow, is_dark: bool) {
    apply_windows_window_frame(&window, is_dark);
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdbAvailability {
    available: bool,
    version: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdbDevice {
    serial: String,
    status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdbExecResult {
    success: bool,
    status_code: i32,
    stdout: String,
    stderr: String,
    full_command: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeviceScreenshot {
    data_base64: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TransferRequest {
    direction: String,
    serial: Option<String>,
    local_path: String,
    remote_path: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TransferProgressPayload {
    direction: String,
    local_path: String,
    remote_path: String,
    percent: Option<u8>,
    message: String,
    done: bool,
    success: Option<bool>,
}

#[derive(Debug)]
struct TransferAdbOutput {
    success: bool,
    status_code: i32,
    stdout: String,
    stderr: String,
}

#[derive(Debug)]
struct DirectoryPushFailure {
    local_path: String,
    remote_path: String,
    status: TransferAdbOutput,
    full_command: String,
}

#[derive(Debug, Default)]
struct DirectoryPushStats {
    directories: usize,
    files: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SdcardEntry {
    name: String,
    path: String,
    is_dir: bool,
    size_bytes: Option<u64>,
    created_at_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalEntry {
    name: String,
    path: String,
    is_dir: bool,
    size_bytes: Option<u64>,
    created_at_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeleteResult {
    success: bool,
    message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TextPreviewMeta {
    path: String,
    name: String,
    size_bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TextPreviewChunk {
    offset: u64,
    next_offset: u64,
    size_bytes: u64,
    text: String,
    eof: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviewFileResult {
    path: String,
    name: String,
    size_bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImagePreviewResult {
    path: String,
    name: String,
    size_bytes: u64,
    data_url: String,
}

struct TerminalSession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
}

struct TerminalRegistry {
    next_id: AtomicU64,
    sessions: Mutex<HashMap<u64, TerminalSession>>,
}

impl Default for TerminalRegistry {
    fn default() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            sessions: Mutex::new(HashMap::new()),
        }
    }
}

impl TerminalRegistry {
    fn next_session_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    fn insert_session(&self, session_id: u64, session: TerminalSession) -> Result<(), String> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| "终端会话状态已损坏".to_string())?;
        sessions.insert(session_id, session);
        Ok(())
    }

    fn remove_session(&self, session_id: u64) -> Option<TerminalSession> {
        self.sessions.lock().ok()?.remove(&session_id)
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TerminalSpawnResult {
    session_id: u64,
    shell: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TerminalOutputPayload {
    session_id: u64,
    data_base64: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TerminalExitPayload {
    session_id: u64,
    code: Option<u32>,
    message: String,
}

const TERMINAL_OUTPUT_FLUSH_MS: u64 = 8;
const TERMINAL_OUTPUT_MAX_BATCH_BYTES: usize = 64 * 1024;
const WINDOWS_DRIVES_ROOT_SENTINEL: &str = "__DROIDPILOT_WINDOWS_DRIVES__";

fn emit_terminal_output(app: &tauri::AppHandle, session_id: u64, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }

    let payload = TerminalOutputPayload {
        session_id,
        data_base64: general_purpose::STANDARD.encode(bytes),
    };
    let _ = app.emit_to("main", "terminal-output", payload);
}

/// 创建一个预配置的 adb Command，Windows 上自动隐藏控制台窗口。
#[cfg(target_os = "windows")]
pub(crate) fn adb_command() -> Command {
    let mut cmd = Command::new(get_adb_path());
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// 创建一个预配置的 adb Command。
#[cfg(not(target_os = "windows"))]
pub(crate) fn adb_command() -> Command {
    Command::new(get_adb_path())
}

fn run_adb(args: &[&str]) -> Result<std::process::Output, String> {
    adb_command()
        .args(args)
        .output()
        .map_err(|err| format!("执行 adb 失败: {err}"))
}

fn adb_failure_message(output: &Output, fallback: &str) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !stderr.is_empty() {
        return stderr;
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !stdout.is_empty() {
        return stdout;
    }

    match output.status.code() {
        Some(code) => format!("{fallback}，退出码 {code}"),
        None => format!("{fallback}，状态 {}", output.status),
    }
}

fn contains_template_placeholder(value: &str) -> bool {
    let bytes = value.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] != b'<' {
            index += 1;
            continue;
        }

        let start = index + 1;
        let Some(relative_end) = value[start..].find('>') else {
            index += 1;
            continue;
        };
        let end = start + relative_end;
        let inner = &value[start..end];
        if is_template_placeholder_name(inner) {
            return true;
        }
        index = end + 1;
    }

    false
}

fn is_template_placeholder_name(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == ':' || ch == '-')
}

#[cfg(target_os = "windows")]
fn copy_png_to_clipboard(bytes: &[u8]) -> Result<(), String> {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Graphics::Gdi::{BITMAPINFOHEADER, BI_RGB};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

    const CF_DIB: u32 = 8;

    let started_at = Instant::now();
    let decode_started_at = Instant::now();
    let rgba = image::load_from_memory(bytes)
        .map_err(|err| format!("解析剪贴板截图失败: {err}"))?
        .to_rgba8();
    let (width, height) = rgba.dimensions();
    if width == 0 || height == 0 {
        return Err("剪贴板截图为空".to_string());
    }
    eprintln!(
        "[screenshot:save] +{}ms native clipboard png decoded, step={}ms, size={}x{}",
        started_at.elapsed().as_millis(),
        decode_started_at.elapsed().as_millis(),
        width,
        height
    );

    let prepare_started_at = Instant::now();
    let header_size = std::mem::size_of::<BITMAPINFOHEADER>();
    let pixel_bytes = width as usize * height as usize * 4;
    let total_bytes = header_size + pixel_bytes;

    let handle = unsafe { GlobalAlloc(GMEM_MOVEABLE, total_bytes) }
        .map_err(|err| format!("分配剪贴板内存失败: {err}"))?;
    let locked = unsafe { GlobalLock(handle) } as *mut u8;
    if locked.is_null() {
        unsafe {
            free_global_memory(handle);
        }
        return Err("锁定剪贴板内存失败".to_string());
    }

    unsafe {
        let header = BITMAPINFOHEADER {
            biSize: header_size as u32,
            biWidth: width as i32,
            biHeight: -(height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            biSizeImage: pixel_bytes as u32,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        };
        std::ptr::write_unaligned(locked as *mut BITMAPINFOHEADER, header);

        let pixels = std::slice::from_raw_parts_mut(locked.add(header_size), pixel_bytes);
        for (source, target) in rgba
            .as_raw()
            .chunks_exact(4)
            .zip(pixels.chunks_exact_mut(4))
        {
            target[0] = source[2];
            target[1] = source[1];
            target[2] = source[0];
            target[3] = source[3];
        }

        let _ = GlobalUnlock(handle);
    }
    eprintln!(
        "[screenshot:save] +{}ms native clipboard dib prepared, step={}ms, bytes={}",
        started_at.elapsed().as_millis(),
        prepare_started_at.elapsed().as_millis(),
        total_bytes
    );

    struct ClipboardGuard;
    impl Drop for ClipboardGuard {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseClipboard();
            }
        }
    }

    let set_started_at = Instant::now();
    let mut clipboard_owns_handle = false;
    let result = unsafe {
        OpenClipboard(None)
            .and_then(|_| {
                let _guard = ClipboardGuard;
                EmptyClipboard()?;
                SetClipboardData(CF_DIB, Some(HANDLE(handle.0)))?;
                clipboard_owns_handle = true;
                Ok(())
            })
            .map_err(|err| format!("写入剪贴板失败: {err}"))
    };

    if !clipboard_owns_handle {
        unsafe {
            free_global_memory(handle);
        }
    }

    eprintln!(
        "[screenshot:save] +{}ms native clipboard set returned, step={}ms",
        started_at.elapsed().as_millis(),
        set_started_at.elapsed().as_millis()
    );

    result
}

#[cfg(target_os = "windows")]
unsafe fn free_global_memory(handle: windows::Win32::Foundation::HGLOBAL) {
    extern "system" {
        fn GlobalFree(hmem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    }

    let _ = GlobalFree(handle.0);
}

#[cfg(not(target_os = "windows"))]
fn copy_png_to_clipboard(bytes: &[u8]) -> Result<(), String> {
    let started_at = Instant::now();
    let temp_path = std::env::temp_dir().join(format!(
        "droidpilot_clipboard_{}_{}.png",
        std::process::id(),
        chrono_like_timestamp()
    ));
    fs::write(&temp_path, bytes).map_err(|err| format!("写入临时截图失败: {err}"))?;
    eprintln!(
        "[screenshot:save] +{}ms clipboard temp written bytes={}",
        started_at.elapsed().as_millis(),
        bytes.len()
    );

    let copy_started_at = Instant::now();
    let result = copy_png_file_to_clipboard(&temp_path);
    eprintln!(
        "[screenshot:save] +{}ms clipboard copy returned, step={}ms",
        started_at.elapsed().as_millis(),
        copy_started_at.elapsed().as_millis()
    );
    let _ = fs::remove_file(&temp_path);
    result
}

#[cfg(target_os = "macos")]
fn copy_png_file_to_clipboard(path: &std::path::Path) -> Result<(), String> {
    let path_text = path
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    let script = format!(
        "set the clipboard to (read (POSIX file \"{}\") as «class PNGf»)",
        path_text
    );
    let output = Command::new("/usr/bin/osascript")
        .args(["-e", script.as_str()])
        .output()
        .map_err(|err| format!("写入剪贴板失败: {err}"))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(adb_failure_message(&output, "写入剪贴板失败"))
    }
}

#[cfg(target_os = "linux")]
fn copy_png_file_to_clipboard(path: &std::path::Path) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|err| format!("读取临时截图失败: {err}"))?;

    let wl_result = run_clipboard_stdin("wl-copy", &["--type", "image/png"], &bytes);
    if wl_result.is_ok() {
        return Ok(());
    }

    let xclip_result = run_clipboard_stdin(
        "xclip",
        &["-selection", "clipboard", "-target", "image/png", "-in"],
        &bytes,
    );
    if xclip_result.is_ok() {
        return Ok(());
    }

    Err(format!(
        "写入剪贴板失败，请安装 wl-clipboard 或 xclip。wl-copy: {}; xclip: {}",
        wl_result.err().unwrap_or_else(|| "不可用".to_string()),
        xclip_result.err().unwrap_or_else(|| "不可用".to_string())
    ))
}

#[cfg(target_os = "linux")]
fn run_clipboard_stdin(command: &str, args: &[&str], bytes: &[u8]) -> Result<(), String> {
    let mut child = Command::new(command)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("{command} 启动失败: {err}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(bytes)
            .map_err(|err| format!("{command} 写入失败: {err}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|err| format!("{command} 等待失败: {err}"))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(adb_failure_message(
            &output,
            &format!("{command} 返回失败状态"),
        ))
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn copy_png_file_to_clipboard(_path: &std::path::Path) -> Result<(), String> {
    Err("当前平台暂不支持截图复制到剪贴板".to_string())
}

fn chrono_like_timestamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or_default()
}

fn next_available_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }

    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("file");
    let extension = path.extension().and_then(|value| value.to_str());

    for index in 1..10_000 {
        let candidate_name = match extension {
            Some(ext) if !ext.is_empty() => format!("{stem}_{index:03}.{ext}"),
            _ => format!("{stem}_{index:03}"),
        };
        let candidate = parent.join(candidate_name);
        if !candidate.exists() {
            return candidate;
        }
    }

    let fallback_name = match extension {
        Some(ext) if !ext.is_empty() => format!("{stem}_{}.{ext}", chrono_like_timestamp()),
        _ => format!("{stem}_{}", chrono_like_timestamp()),
    };
    parent.join(fallback_name)
}

fn remote_entry_name(path: &str) -> Option<&str> {
    let trimmed = path.trim().trim_end_matches('/');
    trimmed
        .rsplit('/')
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn join_remote_path(base: &str, child: &str) -> String {
    let base = base.trim().trim_end_matches('/');
    let child = child.trim_start_matches('/').trim_end_matches('/');

    if child.is_empty() {
        return if base.is_empty() {
            "/".to_string()
        } else {
            base.to_string()
        };
    }

    if base.is_empty() || base == "/" {
        format!("/{child}")
    } else {
        format!("{base}/{child}")
    }
}

fn sanitize_remote_directory_name(value: &str) -> String {
    let sanitized = value
        .chars()
        .filter(|ch| ch.is_alphanumeric() || matches!(ch, '.' | '-' | '_'))
        .collect::<String>();

    if sanitized.is_empty() || sanitized == "." || sanitized == ".." {
        "folder".to_string()
    } else {
        sanitized
    }
}

fn join_remote_directory(base: &str, directory_name: &str) -> String {
    join_remote_path(base, &sanitize_remote_directory_name(directory_name))
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn system_time_to_millis(value: SystemTime) -> Option<u64> {
    value
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
}

fn metadata_created_at_ms(metadata: &fs::Metadata) -> Option<u64> {
    metadata
        .created()
        .or_else(|_| metadata.modified())
        .ok()
        .and_then(system_time_to_millis)
}

fn unix_seconds_to_millis(value: &str) -> Option<u64> {
    let seconds = value.trim().parse::<i64>().ok()?;
    if seconds <= 0 {
        return None;
    }
    Some((seconds as u64).saturating_mul(1000))
}

fn is_allowed_device_storage_path(path: &str) -> bool {
    ["/storage/emulated/0", "/sdcard", "/data/local/tmp"]
        .iter()
        .any(|root| path == *root || path.starts_with(&format!("{root}/")))
}

fn is_device_storage_root(path: &str) -> bool {
    ["/storage/emulated/0", "/sdcard", "/data/local/tmp"]
        .iter()
        .any(|root| path == *root || path == format!("{root}/"))
}

#[tauri::command]
fn get_cargo_package_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[tauri::command]
async fn transfer_file(
    app: tauri::AppHandle,
    request: TransferRequest,
) -> Result<AdbExecResult, String> {
    tauri::async_runtime::spawn_blocking(move || transfer_file_impl(app, request))
        .await
        .map_err(|err| format!("传输任务执行失败: {err}"))?
}

#[tauri::command]
async fn save_local_recording(
    local_dir: String,
    file_name: String,
    data_base64: String,
) -> Result<AdbExecResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let local_dir = local_dir.trim();
        if local_dir.is_empty() {
            return Err("本地保存目录不能为空".to_string());
        }

        fs::create_dir_all(local_dir).map_err(|err| format!("创建本地目录失败: {err}"))?;

        let safe_file_name = Path::new(file_name.trim())
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("droidpilot_record.webm");
        let target_path = next_available_path(&Path::new(local_dir).join(safe_file_name));
        let bytes = general_purpose::STANDARD
            .decode(data_base64.trim())
            .map_err(|err| format!("录屏数据解码失败: {err}"))?;

        if bytes.is_empty() {
            return Err("录屏数据为空".to_string());
        }

        fs::write(&target_path, bytes).map_err(|err| format!("写入录屏文件失败: {err}"))?;
        let target_path_text = target_path.to_string_lossy().to_string();

        Ok(AdbExecResult {
            success: true,
            status_code: 0,
            stdout: format!("录屏已保存到 {target_path_text}"),
            stderr: String::new(),
            full_command: format!("save-local-recording \"{target_path_text}\""),
        })
    })
    .await
    .map_err(|err| format!("保存录屏任务执行失败: {err}"))?
}

#[tauri::command]
async fn save_local_screenshot(
    local_dir: Option<String>,
    file_name: String,
    data_base64: String,
    copy_to_clipboard: bool,
) -> Result<AdbExecResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let started_at = Instant::now();
        eprintln!(
            "[screenshot:save] start copy_to_clipboard={} local_dir={}",
            copy_to_clipboard,
            local_dir.as_deref().unwrap_or("")
        );
        let decode_started_at = Instant::now();
        let bytes = general_purpose::STANDARD
            .decode(data_base64.trim())
            .map_err(|err| format!("截图数据解码失败: {err}"))?;
        eprintln!(
            "[screenshot:save] +{}ms base64 decoded, step={}ms, bytes={}",
            started_at.elapsed().as_millis(),
            decode_started_at.elapsed().as_millis(),
            bytes.len()
        );

        if bytes.is_empty() {
            return Err("截图数据为空".to_string());
        }

        let mut messages = Vec::new();
        let mut target_path_text = String::new();

        if let Some(local_dir) = local_dir
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let write_started_at = Instant::now();
            fs::create_dir_all(local_dir).map_err(|err| format!("创建本地目录失败: {err}"))?;
            let safe_file_name = Path::new(file_name.trim())
                .file_name()
                .and_then(|value| value.to_str())
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("droidpilot_region.png");
            let target_path = next_available_path(&Path::new(local_dir).join(safe_file_name));
            fs::write(&target_path, &bytes).map_err(|err| format!("写入截图文件失败: {err}"))?;
            target_path_text = target_path.to_string_lossy().to_string();
            eprintln!(
                "[screenshot:save] +{}ms file written, step={}ms",
                started_at.elapsed().as_millis(),
                write_started_at.elapsed().as_millis()
            );
            messages.push(format!("截图已保存到 {target_path_text}"));
        }

        if copy_to_clipboard {
            let copy_started_at = Instant::now();
            copy_png_to_clipboard(&bytes)?;
            eprintln!(
                "[screenshot:save] +{}ms copy complete, step={}ms",
                started_at.elapsed().as_millis(),
                copy_started_at.elapsed().as_millis()
            );
            messages.push("截图已复制到剪贴板".to_string());
        }

        if messages.is_empty() {
            return Err("未选择截图保存或复制方式".to_string());
        }

        let result = AdbExecResult {
            success: true,
            status_code: 0,
            stdout: messages.join("；"),
            stderr: String::new(),
            full_command: if target_path_text.is_empty() {
                "save-local-screenshot | clipboard".to_string()
            } else if copy_to_clipboard {
                format!("save-local-screenshot \"{target_path_text}\" | clipboard")
            } else {
                format!("save-local-screenshot \"{target_path_text}\"")
            },
        };
        eprintln!(
            "[screenshot:save] +{}ms command done",
            started_at.elapsed().as_millis()
        );
        Ok(result)
    })
    .await
    .map_err(|err| format!("保存截图任务执行失败: {err}"))?
}

fn transfer_file_impl(
    app: tauri::AppHandle,
    request: TransferRequest,
) -> Result<AdbExecResult, String> {
    let serial = request
        .serial
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "请先选择目标设备".to_string())?;

    let local_path = request.local_path.trim();
    let remote_path = request.remote_path.trim();

    if local_path.is_empty() || remote_path.is_empty() {
        return Err("本地路径和远端路径不能为空".to_string());
    }

    if remote_path.contains('<')
        || remote_path.contains('>')
        || local_path.contains('<')
        || local_path.contains('>')
    {
        return Err("路径中包含占位符，请先填写完整参数".to_string());
    }

    let push_metadata = if request.direction == "push" {
        Some(fs::metadata(local_path).map_err(|_| format!("本地路径不存在: {local_path}"))?)
    } else {
        None
    };

    if request.direction == "pull" {
        fs::create_dir_all(local_path).map_err(|err| format!("创建本地目录失败: {err}"))?;
    }

    let effective_local_path = if request.direction == "pull" {
        remote_entry_name(remote_path)
            .map(|name| {
                next_available_path(&Path::new(local_path).join(name))
                    .to_string_lossy()
                    .to_string()
            })
            .unwrap_or_else(|| local_path.to_string())
    } else {
        local_path.to_string()
    };

    let mut args = vec!["-s".to_string(), serial.to_string()];

    match request.direction.as_str() {
        "push" => {
            if push_metadata
                .as_ref()
                .map(|metadata| metadata.is_dir())
                .unwrap_or(false)
            {
                return push_directory_to_remote(&app, serial, Path::new(local_path), remote_path);
            }
            args.push("push".to_string());
            args.push(local_path.to_string());
            args.push(remote_path.to_string());
        }
        "pull" => {
            args.push("pull".to_string());
            args.push(remote_path.to_string());
            args.push(effective_local_path.clone());
        }
        _ => {
            return Err("不支持的传输方向，仅支持 push / pull".to_string());
        }
    }

    emit_transfer_progress(
        &app,
        &request.direction,
        &effective_local_path,
        remote_path,
        None,
        "准备复制",
        false,
        None,
    );

    let adb_output = run_transfer_adb_command(
        &app,
        &request.direction,
        &effective_local_path,
        remote_path,
        &args,
    )?;

    let success = adb_output.success;
    let status_code = adb_output.status_code;
    let stdout = if success {
        if request.direction == "push" {
            "上传完成".to_string()
        } else {
            format!("下载完成: {effective_local_path}")
        }
    } else {
        adb_output.stdout.trim().to_string()
    };
    let stderr = if success {
        String::new()
    } else {
        let message = adb_output.stderr.trim();
        if message.is_empty() {
            format!("传输失败，退出码 {status_code}")
        } else {
            message.to_string()
        }
    };

    emit_transfer_progress(
        &app,
        &request.direction,
        &effective_local_path,
        remote_path,
        if success { Some(100) } else { None },
        if success {
            "复制完成"
        } else {
            "复制失败"
        },
        true,
        Some(success),
    );

    Ok(AdbExecResult {
        success,
        status_code,
        stdout,
        stderr,
        full_command: format!("adb {}", args.join(" ")),
    })
}

fn run_transfer_adb_command(
    app: &tauri::AppHandle,
    direction: &str,
    local_path: &str,
    remote_path: &str,
    args: &[String],
) -> Result<TransferAdbOutput, String> {
    let mut child = adb_command()
        .args(args.iter().map(String::as_str))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("执行 adb 失败: {err}"))?;

    let stdout_handle = child.stdout.take().map(|stdout| {
        read_transfer_output(
            stdout,
            app.clone(),
            direction.to_string(),
            local_path.to_string(),
            remote_path.to_string(),
        )
    });
    let stderr_handle = child.stderr.take().map(|stderr| {
        read_transfer_output(
            stderr,
            app.clone(),
            direction.to_string(),
            local_path.to_string(),
            remote_path.to_string(),
        )
    });

    let status = child
        .wait()
        .map_err(|err| format!("等待 adb 结束失败: {err}"))?;

    Ok(TransferAdbOutput {
        success: status.success(),
        status_code: status.code().unwrap_or(-1),
        stdout: stdout_handle
            .and_then(|handle| handle.join().ok())
            .unwrap_or_default(),
        stderr: stderr_handle
            .and_then(|handle| handle.join().ok())
            .unwrap_or_default(),
    })
}

fn adb_full_command(args: &[String]) -> String {
    format!("adb {}", args.join(" "))
}

fn remote_mkdir_args(serial: &str, remote_dir: &str) -> Vec<String> {
    vec![
        "-s".to_string(),
        serial.to_string(),
        "shell".to_string(),
        "sh".to_string(),
        "-c".to_string(),
        format!("mkdir -p {}", shell_quote(remote_dir)),
    ]
}

fn push_file_args(serial: &str, local_file: &str, remote_file: &str) -> Vec<String> {
    vec![
        "-s".to_string(),
        serial.to_string(),
        "push".to_string(),
        local_file.to_string(),
        remote_file.to_string(),
    ]
}

fn push_directory_to_remote(
    app: &tauri::AppHandle,
    serial: &str,
    local_dir: &Path,
    remote_base_path: &str,
) -> Result<AdbExecResult, String> {
    let local_path = local_dir.to_string_lossy().to_string();
    let root_name = local_dir
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "folder".to_string());
    let remote_root = join_remote_directory(remote_base_path, &root_name);
    let full_command = format!("adb -s {serial} push {local_path} {remote_root}");
    let mut stats = DirectoryPushStats::default();

    emit_transfer_progress(
        app,
        "push",
        &local_path,
        &remote_root,
        None,
        "准备复制目录",
        false,
        None,
    );

    let mkdir_args = remote_mkdir_args(serial, &remote_root);
    let mkdir_output =
        run_transfer_adb_command(app, "push", &local_path, &remote_root, &mkdir_args)?;
    if !mkdir_output.success {
        return Ok(directory_push_failure_result(
            app,
            DirectoryPushFailure {
                local_path,
                remote_path: remote_root,
                status: mkdir_output,
                full_command: adb_full_command(&mkdir_args),
            },
        ));
    }
    stats.directories += 1;

    if let Some(failure) =
        push_directory_contents_to_remote(app, serial, local_dir, &remote_root, &mut stats)?
    {
        return Ok(directory_push_failure_result(app, failure));
    }

    emit_transfer_progress(
        app,
        "push",
        &local_path,
        &remote_root,
        Some(100),
        "复制完成",
        true,
        Some(true),
    );

    Ok(AdbExecResult {
        success: true,
        status_code: 0,
        stdout: format!(
            "上传完成: {local_path} -> {remote_root}，目录 {} 个，文件 {} 个",
            stats.directories, stats.files
        ),
        stderr: String::new(),
        full_command,
    })
}

fn push_directory_contents_to_remote(
    app: &tauri::AppHandle,
    serial: &str,
    local_dir: &Path,
    remote_dir: &str,
    stats: &mut DirectoryPushStats,
) -> Result<Option<DirectoryPushFailure>, String> {
    let mut entries = fs::read_dir(local_dir)
        .map_err(|err| format!("读取本地目录失败 {}: {err}", local_dir.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("读取本地目录失败 {}: {err}", local_dir.display()))?;

    entries.sort_by_key(|entry| entry.file_name().to_string_lossy().to_string());

    for entry in entries {
        let file_type = entry
            .file_type()
            .map_err(|err| format!("读取本地条目类型失败 {}: {err}", entry.path().display()))?;
        let entry_path = entry.path();
        let entry_name = entry.file_name().to_string_lossy().to_string();

        if file_type.is_dir() {
            let remote_child = join_remote_directory(remote_dir, &entry_name);
            let local_child = entry_path.to_string_lossy().to_string();
            let mkdir_args = remote_mkdir_args(serial, &remote_child);

            emit_transfer_progress(
                app,
                "push",
                &local_child,
                &remote_child,
                None,
                "准备复制目录",
                false,
                None,
            );

            let mkdir_output =
                run_transfer_adb_command(app, "push", &local_child, &remote_child, &mkdir_args)?;
            if !mkdir_output.success {
                return Ok(Some(DirectoryPushFailure {
                    local_path: local_child,
                    remote_path: remote_child,
                    status: mkdir_output,
                    full_command: adb_full_command(&mkdir_args),
                }));
            }

            stats.directories += 1;
            if let Some(failure) =
                push_directory_contents_to_remote(app, serial, &entry_path, &remote_child, stats)?
            {
                return Ok(Some(failure));
            }
        } else if file_type.is_file() {
            let local_file = entry_path.to_string_lossy().to_string();
            let remote_file = join_remote_path(remote_dir, &entry_name);
            let push_args = push_file_args(serial, &local_file, &remote_file);

            emit_transfer_progress(
                app,
                "push",
                &local_file,
                &remote_file,
                None,
                "准备复制",
                false,
                None,
            );

            let push_output =
                run_transfer_adb_command(app, "push", &local_file, &remote_file, &push_args)?;
            if !push_output.success {
                return Ok(Some(DirectoryPushFailure {
                    local_path: local_file,
                    remote_path: remote_file,
                    status: push_output,
                    full_command: adb_full_command(&push_args),
                }));
            }

            stats.files += 1;
        }
    }

    Ok(None)
}

fn directory_push_failure_result(
    app: &tauri::AppHandle,
    failure: DirectoryPushFailure,
) -> AdbExecResult {
    let DirectoryPushFailure {
        local_path,
        remote_path,
        status,
        full_command,
    } = failure;
    let status_code = status.status_code;
    let stdout = status.stdout.trim().to_string();
    let stderr = {
        let message = status.stderr.trim();
        if message.is_empty() {
            format!("传输失败，退出码 {status_code}")
        } else {
            message.to_string()
        }
    };

    emit_transfer_progress(
        app,
        "push",
        &local_path,
        &remote_path,
        None,
        "复制失败",
        true,
        Some(false),
    );

    AdbExecResult {
        success: false,
        status_code,
        stdout,
        stderr,
        full_command,
    }
}

fn emit_transfer_progress(
    app: &tauri::AppHandle,
    direction: &str,
    local_path: &str,
    remote_path: &str,
    percent: Option<u8>,
    message: &str,
    done: bool,
    success: Option<bool>,
) {
    let _ = app.emit_to(
        "main",
        "adb://transfer-progress",
        TransferProgressPayload {
            direction: direction.to_string(),
            local_path: local_path.to_string(),
            remote_path: remote_path.to_string(),
            percent,
            message: message.to_string(),
            done,
            success,
        },
    );
}

fn read_transfer_output<R: Read + Send + 'static>(
    mut reader: R,
    app: tauri::AppHandle,
    direction: String,
    local_path: String,
    remote_path: String,
) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut output = String::new();
        let mut buffer = [0_u8; 512];
        let mut last_percent: Option<u8> = None;

        loop {
            let read_count = match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(value) => value,
                Err(_) => break,
            };

            let text = String::from_utf8_lossy(&buffer[..read_count]).to_string();
            output.push_str(&text);

            let percent = parse_transfer_percent(&text);
            if percent.is_some() && percent != last_percent {
                last_percent = percent;
                let message = compact_transfer_message(&text);
                emit_transfer_progress(
                    &app,
                    &direction,
                    &local_path,
                    &remote_path,
                    percent,
                    if message.is_empty() {
                        "正在复制"
                    } else {
                        &message
                    },
                    false,
                    None,
                );
            }
        }

        output
    })
}

fn parse_transfer_percent(text: &str) -> Option<u8> {
    let bytes = text.as_bytes();
    let mut result = None;

    for (index, byte) in bytes.iter().enumerate() {
        if *byte != b'%' {
            continue;
        }

        let mut start = index;
        while start > 0 && bytes[start - 1].is_ascii_whitespace() {
            start -= 1;
        }
        let mut digit_start = start;
        while digit_start > 0 && bytes[digit_start - 1].is_ascii_digit() {
            digit_start -= 1;
        }

        if digit_start < start {
            if let Ok(value) = text[digit_start..start].parse::<u8>() {
                result = Some(value.min(100));
            }
        }
    }

    result
}

fn compact_transfer_message(text: &str) -> String {
    let normalized = text.replace('\r', " ").replace('\n', " ");
    let trimmed = normalized.trim();
    if trimmed.chars().count() > 120 {
        format!("{}...", trimmed.chars().take(120).collect::<String>())
    } else {
        trimmed.to_string()
    }
}

#[tauri::command]
async fn install_apk_file(
    serial: Option<String>,
    apk_path: String,
) -> Result<AdbExecResult, String> {
    tauri::async_runtime::spawn_blocking(move || install_apk_file_impl(serial, apk_path))
        .await
        .map_err(|err| format!("安装任务执行失败: {err}"))?
}

fn install_apk_file_impl(
    serial: Option<String>,
    apk_path: String,
) -> Result<AdbExecResult, String> {
    let serial_value = serial
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "请先选择目标设备".to_string())?;

    let apk_path = apk_path.trim();
    if apk_path.is_empty() {
        return Err("APK 路径不能为空".to_string());
    }
    if apk_path.contains('<') || apk_path.contains('>') {
        return Err("APK 路径包含占位符，请先填写完整参数".to_string());
    }

    let metadata = fs::metadata(apk_path).map_err(|_| format!("本地文件不存在: {apk_path}"))?;
    if !metadata.is_file() {
        return Err("安装仅支持单个 APK 文件".to_string());
    }

    let output = adb_command()
        .args(["-s", serial_value, "install", apk_path])
        .output()
        .map_err(|err| format!("执行 adb 失败: {err}"))?;

    let success = output.status.success();
    let status_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    Ok(AdbExecResult {
        success,
        status_code,
        stdout,
        stderr,
        full_command: format!("adb -s {} install \"{}\"", serial_value, apk_path),
    })
}

#[tauri::command]
async fn execute_adb_command(
    command_input: String,
    serial: Option<String>,
) -> Result<AdbExecResult, String> {
    tauri::async_runtime::spawn_blocking(move || execute_adb_command_impl(command_input, serial))
        .await
        .map_err(|err| format!("命令任务执行失败: {err}"))?
}

fn execute_adb_command_impl(
    command_input: String,
    serial: Option<String>,
) -> Result<AdbExecResult, String> {
    let trimmed = command_input.trim();
    if trimmed.is_empty() {
        return Err("命令不能为空".to_string());
    }

    let mut parsed =
        shlex::split(trimmed).ok_or_else(|| "命令格式不合法，请检查引号".to_string())?;
    if parsed.is_empty() {
        return Err("命令不能为空".to_string());
    }

    let first = parsed[0].to_lowercase();
    if first == "adb" {
        parsed.remove(0);
    }

    if parsed.is_empty() {
        return Err("请输入 adb 子命令，例如 devices 或 shell getprop".to_string());
    }

    if parsed
        .iter()
        .any(|item| contains_template_placeholder(item))
    {
        return Err("命令包含占位符，请先填写完整参数".to_string());
    }

    if parsed[0] == "pull-latest-recording" {
        if parsed.len() < 4 {
            return Err("pull-latest-recording 命令参数不足，应包含本地目录、设备路径记录文件和默认录屏路径".to_string());
        }

        let serial_value = serial
            .as_ref()
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "该命令需要选择目标设备，请先在设备列表中选择设备".to_string())?;

        let local_dir = parsed[1].trim();
        let path_file = parsed[2].trim();
        let fallback_remote_path = parsed[3].trim();
        if local_dir.is_empty() || path_file.is_empty() || fallback_remote_path.is_empty() {
            return Err("本地目录、设备路径记录文件和默认录屏路径不能为空".to_string());
        }

        fs::create_dir_all(local_dir).map_err(|err| format!("创建本地目录失败: {err}"))?;

        let path_output = adb_command()
            .args(["-s", serial_value, "shell", "cat", path_file])
            .output()
            .map_err(|err| format!("读取录屏路径失败: {err}"))?;
        let recorded_path = String::from_utf8_lossy(&path_output.stdout)
            .lines()
            .map(str::trim)
            .find(|line| line.starts_with('/'))
            .map(str::to_string)
            .unwrap_or_else(|| fallback_remote_path.to_string());
        let target_name = remote_entry_name(&recorded_path).unwrap_or("screen.mp4");
        let target_path = next_available_path(&Path::new(local_dir).join(target_name));
        let target_path_text = target_path.to_string_lossy();

        let wait_script = format!(
            "for i in 1 2 3 4 5; do [ -s {} ] && exit 0; sleep 1; done; [ -e {} ]",
            shell_quote(&recorded_path),
            shell_quote(&recorded_path)
        );
        let wait_output = adb_command()
            .args(["-s", serial_value, "shell", "sh", "-c", &wait_script])
            .output()
            .map_err(|err| format!("检查录屏文件失败: {err}"))?;
        if !wait_output.status.success() {
            return Ok(AdbExecResult {
                success: false,
                status_code: wait_output.status.code().unwrap_or(-1),
                stdout: String::new(),
                stderr: format!("录屏文件还未生成或不存在: {recorded_path}"),
                full_command: format!("adb -s {} shell ls \"{}\"", serial_value, recorded_path),
            });
        }

        let output = adb_command()
            .args([
                "-s",
                serial_value,
                "pull",
                &recorded_path,
                target_path_text.as_ref(),
            ])
            .output()
            .map_err(|err| format!("执行 adb 失败: {err}"))?;

        let success = output.status.success();
        let status_code = output.status.code().unwrap_or(-1);
        let stdout = if success {
            format!("下载完成: {}", target_path_text)
        } else {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        };
        let stderr = if success {
            String::new()
        } else {
            adb_failure_message(&output, "拉取录屏失败")
        };

        return Ok(AdbExecResult {
            success,
            status_code,
            stdout,
            stderr,
            full_command: format!(
                "adb -s {} pull {} \"{}\"",
                serial_value, recorded_path, target_path_text
            ),
        });
    }

    if parsed[0] == "capture-to-local" {
        if parsed.len() < 2 {
            return Err("capture-to-local 命令参数不足，应包含本地文件路径".to_string());
        }

        let serial_value = serial
            .as_ref()
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "该命令需要选择目标设备，请先在设备列表中选择设备".to_string())?;

        let local_path = parsed[1].trim();
        if local_path.is_empty() {
            return Err("本地文件路径不能为空".to_string());
        }

        let local_path = Path::new(local_path);
        if let Some(parent) = local_path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|err| format!("创建本地目录失败: {err}"))?;
            }
        }

        let args = ["-s", serial_value, "exec-out", "screencap", "-p"];
        let output = adb_command()
            .args(args)
            .output()
            .map_err(|err| format!("执行 adb 失败: {err}"))?;

        let success = output.status.success();
        let status_code = output.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        if !success {
            return Ok(AdbExecResult {
                success,
                status_code,
                stdout: String::new(),
                stderr,
                full_command: format!("adb -s {} exec-out screencap -p", serial_value),
            });
        }

        let target_path = next_available_path(local_path);
        fs::write(&target_path, &output.stdout)
            .map_err(|err| format!("写入截图文件失败: {err}"))?;
        let target_path_text = target_path.to_string_lossy();

        return Ok(AdbExecResult {
            success: true,
            status_code,
            stdout: format!("截图已保存到 {}", target_path_text),
            stderr: String::new(),
            full_command: format!(
                "adb -s {} exec-out screencap -p > \"{}\"",
                serial_value, target_path_text
            ),
        });
    }

    if parsed[0] == "capture-to-clipboard" {
        let serial_value = serial
            .as_ref()
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "该命令需要选择目标设备，请先在设备列表中选择设备".to_string())?;

        let args = ["-s", serial_value, "exec-out", "screencap", "-p"];
        let output = adb_command()
            .args(args)
            .output()
            .map_err(|err| format!("执行 adb 失败: {err}"))?;

        let success = output.status.success();
        let status_code = output.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        if !success {
            return Ok(AdbExecResult {
                success,
                status_code,
                stdout: String::new(),
                stderr,
                full_command: format!("adb -s {} exec-out screencap -p", serial_value),
            });
        }

        if let Err(message) = copy_png_to_clipboard(&output.stdout) {
            return Ok(AdbExecResult {
                success: false,
                status_code: -1,
                stdout: String::new(),
                stderr: message,
                full_command: format!("adb -s {} exec-out screencap -p | clipboard", serial_value),
            });
        }

        return Ok(AdbExecResult {
            success: true,
            status_code,
            stdout: "截图已复制到剪贴板".to_string(),
            stderr: String::new(),
            full_command: format!("adb -s {} exec-out screencap -p | clipboard", serial_value),
        });
    }

    let head = parsed[0].as_str();
    let global_commands = [
        "devices",
        "connect",
        "disconnect",
        "pair",
        "start-server",
        "kill-server",
        "version",
    ];
    let is_global = global_commands.contains(&head);

    match head {
        "install" | "uninstall" => {
            if parsed.len() < 2 {
                return Err(format!("{} 命令参数不足", head));
            }
        }
        "push" | "pull" => {
            if parsed.len() < 3 {
                return Err(format!("{} 命令参数不足，应包含源路径和目标路径", head));
            }
        }
        _ => {}
    }

    let mut args: Vec<String> = Vec::new();
    if !is_global {
        let serial_value = serial
            .as_ref()
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "该命令需要选择目标设备，请先在设备列表中选择设备".to_string())?;

        args.push("-s".to_string());
        args.push(serial_value.to_string());
    }

    args.extend(parsed);

    let output = adb_command()
        .args(args.iter().map(String::as_str))
        .output()
        .map_err(|err| format!("执行 adb 失败: {err}"))?;

    let success = output.status.success();
    let status_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    Ok(AdbExecResult {
        success,
        status_code,
        stdout,
        stderr,
        full_command: format!("adb {}", args.join(" ")),
    })
}

#[cfg(target_os = "windows")]
fn find_windows_executable(name: &str) -> Option<String> {
    let candidate = Path::new(name);
    if candidate.is_absolute() && candidate.is_file() {
        return Some(name.to_string());
    }

    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|path| path.join(name))
            .find(|path| path.is_file())
            .map(|path| path.to_string_lossy().to_string())
    })
}

#[cfg(target_os = "windows")]
fn default_terminal_shell() -> (String, Vec<String>) {
    for shell in ["pwsh.exe", "powershell.exe"] {
        if let Some(path) = find_windows_executable(shell) {
            return (path, vec!["-NoLogo".to_string()]);
        }
    }

    let shell = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());
    (shell, Vec::new())
}

#[cfg(target_os = "macos")]
fn default_terminal_shell() -> (String, Vec<String>) {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    (shell, Vec::new())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn default_terminal_shell() -> (String, Vec<String>) {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    (shell, Vec::new())
}

fn add_adb_to_terminal_env(cmd: &mut CommandBuilder) {
    let adb_path = get_adb_path();
    cmd.env("ADBTOOLS_ADB_PATH", adb_path.as_os_str());

    let Some(parent) = adb_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    else {
        return;
    };

    let separator = if cfg!(target_os = "windows") {
        ";"
    } else {
        ":"
    };
    let mut next_path = OsString::from(parent.as_os_str());
    if let Some(existing) = std::env::var_os("PATH").filter(|value| !value.is_empty()) {
        next_path.push(separator);
        next_path.push(existing);
    }
    cmd.env("PATH", next_path);
}

#[tauri::command]
async fn terminal_spawn(
    app: tauri::AppHandle,
    registry: tauri::State<'_, TerminalRegistry>,
    rows: Option<u16>,
    cols: Option<u16>,
) -> Result<TerminalSpawnResult, String> {
    let rows = rows.unwrap_or(24).clamp(8, 200);
    let cols = cols.unwrap_or(80).clamp(20, 400);
    let session_id = registry.next_session_id();
    let (shell, shell_args) = default_terminal_shell();

    let pty_system = NativePtySystem::default();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|err| format!("创建终端失败: {err}"))?;

    let mut cmd = CommandBuilder::new(&shell);
    cmd.args(shell_args);
    add_adb_to_terminal_env(&mut cmd);

    let mut child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|err| format!("启动终端失败: {err}"))?;
    let killer = child.clone_killer();
    drop(pair.slave);

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|err| format!("读取终端输出失败: {err}"))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|err| format!("连接终端输入失败: {err}"))?;

    registry.insert_session(
        session_id,
        TerminalSession {
            master: pair.master,
            writer,
            killer,
        },
    )?;

    let output_app = app.clone();
    let (output_tx, output_rx) = mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut buffer = [0_u8; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(size) => {
                    if output_tx.send(buffer[..size].to_vec()).is_err() {
                        break;
                    }
                }
                Err(err) => {
                    let _ = output_tx
                        .send(format!("\r\n[terminal read error: {err}]\r\n").into_bytes());
                    break;
                }
            }
        }
    });

    std::thread::spawn(move || {
        let mut pending = Vec::with_capacity(TERMINAL_OUTPUT_MAX_BATCH_BYTES);
        loop {
            let first_chunk = match output_rx.recv() {
                Ok(chunk) => chunk,
                Err(_) => break,
            };

            pending.extend_from_slice(&first_chunk);
            let mut disconnected = false;
            while pending.len() < TERMINAL_OUTPUT_MAX_BATCH_BYTES {
                match output_rx.recv_timeout(Duration::from_millis(TERMINAL_OUTPUT_FLUSH_MS)) {
                    Ok(chunk) => pending.extend_from_slice(&chunk),
                    Err(mpsc::RecvTimeoutError::Timeout) => break,
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }

            emit_terminal_output(&output_app, session_id, &pending);
            pending.clear();

            if disconnected {
                break;
            }
        }
    });

    let exit_app = app.clone();
    std::thread::spawn(move || {
        let payload = match child.wait() {
            Ok(status) => TerminalExitPayload {
                session_id,
                code: Some(status.exit_code()),
                message: status.to_string(),
            },
            Err(err) => TerminalExitPayload {
                session_id,
                code: None,
                message: format!("终端退出状态读取失败: {err}"),
            },
        };

        if let Some(registry) = exit_app.try_state::<TerminalRegistry>() {
            let _ = registry.remove_session(session_id);
        }
        let _ = exit_app.emit_to("main", "terminal-exit", payload);
    });

    Ok(TerminalSpawnResult { session_id, shell })
}

#[tauri::command]
fn terminal_write(
    registry: tauri::State<'_, TerminalRegistry>,
    session_id: u64,
    data: String,
) -> Result<(), String> {
    let mut sessions = registry
        .sessions
        .lock()
        .map_err(|_| "终端会话状态已损坏".to_string())?;
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| "终端会话不存在".to_string())?;
    session
        .writer
        .write_all(data.as_bytes())
        .map_err(|err| format!("写入终端失败: {err}"))?;
    session
        .writer
        .flush()
        .map_err(|err| format!("刷新终端输入失败: {err}"))
}

#[tauri::command]
fn terminal_resize(
    registry: tauri::State<'_, TerminalRegistry>,
    session_id: u64,
    rows: u16,
    cols: u16,
) -> Result<(), String> {
    let rows = rows.clamp(8, 200);
    let cols = cols.clamp(20, 400);
    let sessions = registry
        .sessions
        .lock()
        .map_err(|_| "终端会话状态已损坏".to_string())?;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| "终端会话不存在".to_string())?;
    session
        .master
        .resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|err| format!("调整终端大小失败: {err}"))
}

#[tauri::command]
fn terminal_close(
    registry: tauri::State<'_, TerminalRegistry>,
    session_id: u64,
) -> Result<(), String> {
    if let Some(mut session) = registry.remove_session(session_id) {
        if let Err(err) = session.killer.kill() {
            if err.raw_os_error() != Some(0) {
                return Err(format!("关闭终端失败: {err}"));
            }
        }
    }
    Ok(())
}

#[tauri::command]
async fn check_adb_availability() -> AdbAvailability {
    match tauri::async_runtime::spawn_blocking(check_adb_availability_impl).await {
        Ok(result) => result,
        Err(err) => AdbAvailability {
            available: false,
            version: None,
            error: Some(format!("检测任务执行失败: {err}")),
        },
    }
}

fn check_adb_availability_impl() -> AdbAvailability {
    match run_adb(&["version"]) {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let version = stdout.lines().next().map(|line| line.trim().to_string());
                AdbAvailability {
                    available: true,
                    version,
                    error: None,
                }
            } else {
                AdbAvailability {
                    available: false,
                    version: None,
                    error: Some(adb_failure_message(&output, "adb 命令返回失败状态")),
                }
            }
        }
        Err(error) => AdbAvailability {
            available: false,
            version: None,
            error: Some(error),
        },
    }
}

#[tauri::command]
async fn list_adb_devices() -> Result<Vec<AdbDevice>, String> {
    tauri::async_runtime::spawn_blocking(list_adb_devices_impl)
        .await
        .map_err(|err| format!("设备查询任务执行失败: {err}"))?
}

fn list_adb_devices_impl() -> Result<Vec<AdbDevice>, String> {
    let output = run_adb(&["devices"])?;

    if !output.status.success() {
        return Err(adb_failure_message(&output, "获取设备列表失败"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut devices = Vec::new();

    for line in stdout.lines().skip(1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let mut segments = trimmed.split_whitespace();
        let serial = segments.next().unwrap_or_default();
        let status = segments.next().unwrap_or("unknown");

        if serial.is_empty() {
            continue;
        }

        devices.push(AdbDevice {
            serial: serial.to_string(),
            status: status.to_string(),
        });
    }

    Ok(devices)
}

#[tauri::command]
async fn list_sdcard_entries(
    serial: Option<String>,
    path: Option<String>,
) -> Result<Vec<SdcardEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || list_sdcard_entries_impl(serial, path))
        .await
        .map_err(|err| format!("目录读取任务执行失败: {err}"))?
}

#[tauri::command]
async fn list_local_entries(path: Option<String>) -> Result<Vec<LocalEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || list_local_entries_impl(path))
        .await
        .map_err(|err| format!("本地目录读取任务执行失败: {err}"))?
}

#[tauri::command]
async fn capture_device_screenshot(serial: String) -> Result<DeviceScreenshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let serial_value = serial.trim();
        if serial_value.is_empty() {
            return Err("请先选择设备".to_string());
        }

        let output = adb_command()
            .args(["-s", serial_value, "exec-out", "screencap", "-p"])
            .output()
            .map_err(|err| format!("执行 adb 失败: {err}"))?;

        if !output.status.success() || output.stdout.is_empty() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(if stderr.is_empty() {
                "设备截图失败".to_string()
            } else {
                stderr
            });
        }

        Ok(DeviceScreenshot {
            data_base64: general_purpose::STANDARD.encode(output.stdout),
        })
    })
    .await
    .map_err(|err| format!("设备截图任务执行失败: {err}"))?
}

#[tauri::command]
async fn delete_local_entry(path: String) -> Result<DeleteResult, String> {
    tauri::async_runtime::spawn_blocking(move || delete_local_entry_impl(path))
        .await
        .map_err(|err| format!("本地删除任务执行失败: {err}"))?
}

fn delete_local_entry_impl(path: String) -> Result<DeleteResult, String> {
    let target = PathBuf::from(path.trim());
    if target.as_os_str().is_empty() {
        return Err("请选择要删除的本地文件或目录".to_string());
    }

    let metadata = fs::metadata(&target).map_err(|err| format!("读取本地路径失败: {err}"))?;
    if metadata.is_dir() {
        fs::remove_dir_all(&target).map_err(|err| format!("删除本地目录失败: {err}"))?;
    } else {
        fs::remove_file(&target).map_err(|err| format!("删除本地文件失败: {err}"))?;
    }

    Ok(DeleteResult {
        success: true,
        message: "本地删除完成".to_string(),
    })
}

#[tauri::command]
async fn delete_sdcard_entry(serial: Option<String>, path: String) -> Result<DeleteResult, String> {
    tauri::async_runtime::spawn_blocking(move || delete_sdcard_entry_impl(serial, path))
        .await
        .map_err(|err| format!("设备删除任务执行失败: {err}"))?
}

fn delete_sdcard_entry_impl(serial: Option<String>, path: String) -> Result<DeleteResult, String> {
    let serial_value = serial
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "请先选择目标设备".to_string())?;
    let target_path = path.trim().replace('\\', "/");

    if target_path.is_empty() || is_device_storage_root(&target_path) {
        return Err("不能删除设备存储根目录".to_string());
    }
    if !is_allowed_device_storage_path(&target_path) {
        return Err("仅支持删除设备存储目录下的文件或目录".to_string());
    }

    let output = adb_command()
        .args([
            "-s",
            serial_value,
            "shell",
            "rm",
            "-rf",
            "--",
            target_path.as_str(),
        ])
        .output()
        .map_err(|err| format!("执行 adb 删除失败: {err}"))?;

    if !output.status.success() {
        return Err(adb_failure_message(&output, "设备删除失败"));
    }

    Ok(DeleteResult {
        success: true,
        message: "设备删除完成".to_string(),
    })
}

#[tauri::command]
async fn get_local_text_preview_meta(path: String) -> Result<TextPreviewMeta, String> {
    tauri::async_runtime::spawn_blocking(move || get_local_text_preview_meta_impl(path))
        .await
        .map_err(|err| format!("读取本地文件信息失败: {err}"))?
}

fn get_local_text_preview_meta_impl(path: String) -> Result<TextPreviewMeta, String> {
    let target = PathBuf::from(path.trim());
    if target.as_os_str().is_empty() {
        return Err("文件路径不能为空".to_string());
    }
    let metadata = fs::metadata(&target).map_err(|err| format!("读取文件信息失败: {err}"))?;
    if !metadata.is_file() {
        return Err("仅支持预览文本文件".to_string());
    }
    let name = target
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("文本文件")
        .to_string();
    Ok(TextPreviewMeta {
        path: target.to_string_lossy().to_string(),
        name,
        size_bytes: metadata.len(),
    })
}

#[tauri::command]
async fn read_local_text_chunk(
    path: String,
    offset: u64,
    length: u64,
) -> Result<TextPreviewChunk, String> {
    tauri::async_runtime::spawn_blocking(move || read_local_text_chunk_impl(path, offset, length))
        .await
        .map_err(|err| format!("读取本地文本分片失败: {err}"))?
}

fn read_local_text_chunk_impl(
    path: String,
    offset: u64,
    length: u64,
) -> Result<TextPreviewChunk, String> {
    let target = PathBuf::from(path.trim());
    let metadata = fs::metadata(&target).map_err(|err| format!("读取文件信息失败: {err}"))?;
    if !metadata.is_file() {
        return Err("仅支持预览文本文件".to_string());
    }
    let size_bytes = metadata.len();
    let start = offset.min(size_bytes);
    let read_len = length.clamp(1, 1024 * 1024) as usize;
    let mut file = fs::File::open(&target).map_err(|err| format!("打开文件失败: {err}"))?;
    file.seek(SeekFrom::Start(start))
        .map_err(|err| format!("定位文件失败: {err}"))?;
    let mut buffer = vec![0_u8; read_len];
    let bytes_read = file
        .read(&mut buffer)
        .map_err(|err| format!("读取文件失败: {err}"))?;
    buffer.truncate(bytes_read);
    let next_offset = start + bytes_read as u64;
    Ok(TextPreviewChunk {
        offset: start,
        next_offset,
        size_bytes,
        text: String::from_utf8_lossy(&buffer).to_string(),
        eof: next_offset >= size_bytes,
    })
}

#[tauri::command]
async fn get_remote_text_preview_meta(
    serial: Option<String>,
    path: String,
) -> Result<TextPreviewMeta, String> {
    tauri::async_runtime::spawn_blocking(move || get_remote_text_preview_meta_impl(serial, path))
        .await
        .map_err(|err| format!("读取设备文件信息失败: {err}"))?
}

fn get_remote_text_preview_meta_impl(
    serial: Option<String>,
    path: String,
) -> Result<TextPreviewMeta, String> {
    let serial_value = selected_serial_or_err(serial)?;
    let target_path = normalize_allowed_remote_file_path(path)?;
    let script = format!("wc -c < {}", shell_quote(&target_path));
    let output = adb_command()
        .args([
            "-s",
            serial_value.as_str(),
            "exec-out",
            "sh",
            "-c",
            script.as_str(),
        ])
        .output()
        .map_err(|err| format!("执行 adb 失败: {err}"))?;
    if !output.status.success() {
        return Err(adb_failure_message(&output, "读取设备文件信息失败"));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let size_bytes = stdout
        .split_whitespace()
        .next()
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| "解析设备文件大小失败".to_string())?;
    let name = remote_entry_name(&target_path)
        .unwrap_or("设备文本")
        .to_string();
    Ok(TextPreviewMeta {
        path: target_path,
        name,
        size_bytes,
    })
}

#[tauri::command]
async fn read_remote_text_chunk(
    serial: Option<String>,
    path: String,
    offset: u64,
    length: u64,
) -> Result<TextPreviewChunk, String> {
    tauri::async_runtime::spawn_blocking(move || {
        read_remote_text_chunk_impl(serial, path, offset, length)
    })
    .await
    .map_err(|err| format!("读取设备文本分片失败: {err}"))?
}

fn read_remote_text_chunk_impl(
    serial: Option<String>,
    path: String,
    offset: u64,
    length: u64,
) -> Result<TextPreviewChunk, String> {
    let serial_value = selected_serial_or_err(serial)?;
    let target_path = normalize_allowed_remote_file_path(path)?;
    let meta = get_remote_text_preview_meta_impl(Some(serial_value.clone()), target_path.clone())?;
    let start = offset.min(meta.size_bytes);
    let read_len = length.clamp(1, 1024 * 1024);
    let script = format!(
        "dd if={} bs=1 skip={} count={} 2>/dev/null",
        shell_quote(&target_path),
        start,
        read_len
    );
    let output = adb_command()
        .args([
            "-s",
            serial_value.as_str(),
            "exec-out",
            "sh",
            "-c",
            script.as_str(),
        ])
        .output()
        .map_err(|err| format!("执行 adb 失败: {err}"))?;
    if !output.status.success() {
        return Err(adb_failure_message(&output, "读取设备文本失败"));
    }
    let next_offset = start + output.stdout.len() as u64;
    Ok(TextPreviewChunk {
        offset: start,
        next_offset,
        size_bytes: meta.size_bytes,
        text: String::from_utf8_lossy(&output.stdout).to_string(),
        eof: next_offset >= meta.size_bytes,
    })
}

#[tauri::command]
async fn prepare_remote_preview_file(
    serial: Option<String>,
    path: String,
    local_dir: Option<String>,
) -> Result<PreviewFileResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        prepare_remote_preview_file_impl(serial, path, local_dir)
    })
    .await
    .map_err(|err| format!("准备设备预览文件失败: {err}"))?
}

fn prepare_remote_preview_file_impl(
    serial: Option<String>,
    path: String,
    local_dir: Option<String>,
) -> Result<PreviewFileResult, String> {
    let serial_value = selected_serial_or_err(serial)?;
    let target_path = normalize_allowed_remote_file_path(path)?;
    let name = remote_entry_name(&target_path)
        .unwrap_or("preview-file")
        .to_string();
    let safe_name = Path::new(&name)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("preview-file");
    let target_dir = local_dir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("droidpilot-preview-cache");
    fs::create_dir_all(&target_dir).map_err(|err| format!("创建预览临时目录失败: {err}"))?;
    let local_path = next_available_path(&target_dir.join(safe_name));
    let output = adb_command()
        .args(["-s", serial_value.as_str(), "pull", target_path.as_str()])
        .arg(&local_path)
        .output()
        .map_err(|err| format!("执行 adb 失败: {err}"))?;
    if !output.status.success() {
        return Err(adb_failure_message(&output, "导出设备文件失败"));
    }
    let size_bytes = fs::metadata(&local_path)
        .map_err(|err| format!("读取预览文件失败: {err}"))?
        .len();
    Ok(PreviewFileResult {
        path: local_path.to_string_lossy().to_string(),
        name,
        size_bytes,
    })
}

#[tauri::command]
async fn read_preview_image_data_url(path: String) -> Result<ImagePreviewResult, String> {
    tauri::async_runtime::spawn_blocking(move || read_preview_image_data_url_impl(path))
        .await
        .map_err(|err| format!("读取图片预览失败: {err}"))?
}

fn read_preview_image_data_url_impl(path: String) -> Result<ImagePreviewResult, String> {
    let target = PathBuf::from(path.trim());
    if target.as_os_str().is_empty() {
        return Err("图片路径不能为空".to_string());
    }
    let metadata = fs::metadata(&target).map_err(|err| format!("读取图片信息失败: {err}"))?;
    if !metadata.is_file() {
        return Err("仅支持预览图片文件".to_string());
    }
    const MAX_IMAGE_PREVIEW_BYTES: u64 = 60 * 1024 * 1024;
    if metadata.len() > MAX_IMAGE_PREVIEW_BYTES {
        return Err("图片过大，暂不支持直接预览".to_string());
    }
    let mime = image_mime_for_path(&target).ok_or_else(|| "不支持的图片格式".to_string())?;
    let bytes = fs::read(&target).map_err(|err| format!("读取图片失败: {err}"))?;
    let name = target
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("图片预览")
        .to_string();
    Ok(ImagePreviewResult {
        path: target.to_string_lossy().to_string(),
        name,
        size_bytes: metadata.len(),
        data_url: format!(
            "data:{};base64,{}",
            mime,
            general_purpose::STANDARD.encode(bytes)
        ),
    })
}

fn image_mime_for_path(path: &Path) -> Option<&'static str> {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        "gif" => Some("image/gif"),
        "bmp" => Some("image/bmp"),
        "svg" => Some("image/svg+xml"),
        _ => None,
    }
}

fn selected_serial_or_err(serial: Option<String>) -> Result<String, String> {
    serial
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| "请先选择目标设备".to_string())
}

fn normalize_allowed_remote_file_path(path: String) -> Result<String, String> {
    let target_path = path.trim().replace('\\', "/");
    if target_path.is_empty() {
        return Err("设备文件路径不能为空".to_string());
    }
    if !is_allowed_device_storage_path(&target_path) || target_path.ends_with('/') {
        return Err("仅支持预览设备存储中的文件".to_string());
    }
    Ok(target_path)
}

#[cfg(target_os = "windows")]
fn list_windows_drive_entries() -> Vec<LocalEntry> {
    let mut entries = Vec::new();
    for letter in b'A'..=b'Z' {
        let name = format!("{}:", letter as char);
        let path = format!("{name}\\");
        if matches!(fs::metadata(&path), Ok(metadata) if metadata.is_dir()) {
            entries.push(LocalEntry {
                name,
                path,
                is_dir: true,
                size_bytes: None,
                created_at_ms: None,
            });
        }
    }
    entries
}

#[cfg(not(target_os = "windows"))]
fn list_windows_drive_entries() -> Vec<LocalEntry> {
    Vec::new()
}

fn list_local_entries_impl(path: Option<String>) -> Result<Vec<LocalEntry>, String> {
    if path
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| value == WINDOWS_DRIVES_ROOT_SENTINEL)
    {
        return Ok(list_windows_drive_entries());
    }

    let target_path = path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(default_home_dir)
        .ok_or_else(|| "无法定位默认系统目录".to_string())?;

    let metadata = fs::metadata(&target_path).map_err(|err| format!("读取本地目录失败: {err}"))?;
    if !metadata.is_dir() {
        return Err("请选择本地目录".to_string());
    }

    let mut entries = Vec::new();
    for item in fs::read_dir(&target_path).map_err(|err| format!("读取本地目录失败: {err}"))?
    {
        let item = match item {
            Ok(value) => value,
            Err(_) => continue,
        };
        let metadata = match item.metadata() {
            Ok(value) => value,
            Err(_) => continue,
        };
        let path = item.path();
        let name = item.file_name().to_string_lossy().to_string();
        if name.is_empty() || name.starts_with('.') {
            continue;
        }

        entries.push(LocalEntry {
            name,
            path: path.to_string_lossy().to_string(),
            is_dir: metadata.is_dir(),
            size_bytes: if metadata.is_file() {
                Some(metadata.len())
            } else {
                None
            },
            created_at_ms: metadata_created_at_ms(&metadata),
        });
    }

    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(entries)
}

fn default_home_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }

    #[cfg(not(target_os = "windows"))]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

#[derive(Debug, Default)]
struct RemoteEntryStat {
    size_bytes: Option<u64>,
    created_at_ms: Option<u64>,
}

const REMOTE_ENTRY_STAT_FORMAT: &str = "%s\t%W\t%Y\t%n";
const REMOTE_ENTRY_STAT_FALLBACK_FORMAT: &str = "%s\t0\t%Y\t%n";

fn remote_entry_stat_key(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        "/".to_string()
    } else {
        trimmed.to_string()
    }
}

fn remote_entry_stat_args(
    serial_value: &str,
    entries: &[SdcardEntry],
    include_birth_time: bool,
) -> Option<Vec<String>> {
    if entries.is_empty() {
        return None;
    }

    let format = if include_birth_time {
        REMOTE_ENTRY_STAT_FORMAT
    } else {
        REMOTE_ENTRY_STAT_FALLBACK_FORMAT
    };
    let mut args = Vec::with_capacity(entries.len() + 6);
    args.push("-s".to_string());
    args.push(serial_value.to_string());
    args.push("exec-out".to_string());
    args.push("stat".to_string());
    args.push("-c".to_string());
    args.push(format.to_string());
    args.extend(
        entries
            .iter()
            .map(|entry| remote_entry_stat_key(&entry.path)),
    );
    Some(args)
}

fn run_remote_entry_stat(
    serial_value: &str,
    entries: &[SdcardEntry],
    include_birth_time: bool,
) -> Option<Output> {
    let args = remote_entry_stat_args(serial_value, entries, include_birth_time)?;
    let output = adb_command().args(args).output().ok()?;
    if output.status.success() || !output.stdout.is_empty() {
        Some(output)
    } else {
        None
    }
}

fn load_remote_entry_stats(
    serial_value: &str,
    entries: &[SdcardEntry],
) -> HashMap<String, RemoteEntryStat> {
    let output = match run_remote_entry_stat(serial_value, entries, true)
        .or_else(|| run_remote_entry_stat(serial_value, entries, false))
    {
        Some(value) => value,
        None => return HashMap::new(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .filter_map(|line| {
            let mut parts = line.trim_end_matches('\r').splitn(4, '\t');
            let size_bytes = parts.next()?.trim().parse::<u64>().ok();
            let birth_at_ms = parts.next().and_then(unix_seconds_to_millis);
            let modified_at_ms = parts.next().and_then(unix_seconds_to_millis);
            let path = parts.next().map(remote_entry_stat_key)?;
            Some((
                path,
                RemoteEntryStat {
                    size_bytes,
                    created_at_ms: birth_at_ms.or(modified_at_ms),
                },
            ))
        })
        .collect()
}

fn list_sdcard_entries_impl(
    serial: Option<String>,
    path: Option<String>,
) -> Result<Vec<SdcardEntry>, String> {
    let serial_value = serial
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "请先选择目标设备".to_string())?;

    let mut target_path = path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("/storage/emulated/0/")
        .replace('\\', "/");

    if !is_allowed_device_storage_path(&target_path) && !is_device_storage_root(&target_path) {
        return Err("仅支持浏览设备存储目录".to_string());
    }

    if target_path == "/storage/emulated/0"
        || target_path == "/sdcard"
        || target_path == "/data/local/tmp"
    {
        target_path.push('/');
    }

    let output = adb_command()
        .args([
            "-s",
            serial_value,
            "shell",
            "ls",
            "-1ap",
            target_path.as_str(),
        ])
        .output()
        .map_err(|err| format!("执行 adb 失败: {err}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "读取目录失败".to_string()
        } else {
            stderr
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut entries: Vec<SdcardEntry> = Vec::new();

    for line in stdout.lines() {
        let raw = line.trim();
        if raw.is_empty() || raw == "." || raw == ".." {
            continue;
        }

        let is_dir = raw.ends_with('/');
        let name = raw.trim_end_matches('/').to_string();
        if name.is_empty() {
            continue;
        }

        let base = target_path.trim_end_matches('/');
        let mut full_path = if base.is_empty() {
            format!("/{}", name)
        } else {
            format!("{}/{}", base, name)
        };
        if is_dir {
            full_path.push('/');
        }

        entries.push(SdcardEntry {
            name,
            path: full_path,
            is_dir,
            size_bytes: None,
            created_at_ms: None,
        });
    }

    let stat_map = load_remote_entry_stats(serial_value, &entries);
    for entry in &mut entries {
        if let Some(stat) = stat_map.get(&remote_entry_stat_key(&entry.path)) {
            entry.size_bytes = if entry.is_dir { None } else { stat.size_bytes };
            entry.created_at_ms = stat.created_at_ms;
        }
    }

    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote_entry(name: &str, path: &str, is_dir: bool) -> SdcardEntry {
        SdcardEntry {
            name: name.to_string(),
            path: path.to_string(),
            is_dir,
            size_bytes: None,
            created_at_ms: None,
        }
    }

    #[test]
    fn remote_entry_stat_args_use_exec_out_and_preserve_format_and_paths() {
        let entries = vec![remote_entry(
            "file with spaces.txt",
            "/storage/emulated/0/Download/file with spaces.txt",
            false,
        )];

        let args = remote_entry_stat_args("device-1", &entries, true).expect("stat args");

        assert_eq!(
            args,
            vec![
                "-s",
                "device-1",
                "exec-out",
                "stat",
                "-c",
                "%s\t%W\t%Y\t%n",
                "/storage/emulated/0/Download/file with spaces.txt",
            ]
        );
        assert!(!args.iter().any(|arg| arg == "shell" || arg == "sh"));
    }

    #[test]
    fn join_remote_path_handles_roots_and_slashes() {
        assert_eq!(
            join_remote_path("/sdcard/Download", "folder"),
            "/sdcard/Download/folder"
        );
        assert_eq!(
            join_remote_path("/sdcard/Download/", "folder"),
            "/sdcard/Download/folder"
        );
        assert_eq!(join_remote_path("/", "folder"), "/folder");
        assert_eq!(join_remote_path("/sdcard/Download", ""), "/sdcard/Download");
    }

    #[test]
    fn sanitize_remote_directory_name_removes_special_characters() {
        assert_eq!(
            sanitize_remote_directory_name("测试 folder@2026#!"),
            "测试folder2026"
        );
        assert_eq!(sanitize_remote_directory_name("a.b-c_d"), "a.b-c_d");
        assert_eq!(sanitize_remote_directory_name("!!!"), "folder");
        assert_eq!(sanitize_remote_directory_name(".."), "folder");
    }

    #[test]
    fn join_remote_directory_sanitizes_each_segment() {
        let first = join_remote_directory("/sdcard/Download", "A @dir");
        let second = join_remote_directory(&first, "子 目录#1");

        assert_eq!(first, "/sdcard/Download/Adir");
        assert_eq!(second, "/sdcard/Download/Adir/子目录1");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                apply_windows_window_frame(&window, false);
            }
            Ok(())
        })
        .manage(scrcpy::ScrcpyRegistry::default())
        .manage(TerminalRegistry::default())
        .invoke_handler(tauri::generate_handler![
            check_adb_availability,
            capture_device_screenshot,
            delete_local_entry,
            delete_sdcard_entry,
            get_cargo_package_version,
            set_window_frame_theme,
            get_local_text_preview_meta,
            get_remote_text_preview_meta,
            list_adb_devices,
            list_local_entries,
            list_sdcard_entries,
            prepare_remote_preview_file,
            read_preview_image_data_url,
            read_local_text_chunk,
            read_remote_text_chunk,
            install_apk_file,
            execute_adb_command,
            save_local_recording,
            save_local_screenshot,
            terminal_close,
            terminal_resize,
            terminal_spawn,
            terminal_write,
            transfer_file,
            scrcpy::start_scrcpy_mirror,
            scrcpy::attach_scrcpy_mirror,
            scrcpy::stop_scrcpy_mirror,
            scrcpy::scrcpy_touch,
            scrcpy::scrcpy_keyevent,
            scrcpy::scrcpy_text
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
