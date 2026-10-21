use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Output, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, State};

const SCRCPY_VERSION: &str = "4.1";
const DEVICE_SERVER_PATH: &str = "/data/local/tmp/scrcpy-server.jar";
const DEVICE_NAME_FIELD_LENGTH: usize = 64;
const POINTER_ID_GENERIC_FINGER: u64 = u64::MAX - 1;
const ADB_COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
const INJECT_TEXT_MAX_BYTES: usize = 300;
const CONTROL_MESSAGE_MAX_BYTES: usize = 1 << 18;

#[cfg(has_embedded_scrcpy_server)]
static SCRCPY_SERVER: &[u8] = include_bytes!("../binaries/scrcpy/scrcpy-server-v4.1");

#[derive(Clone, Default)]
pub struct ScrcpyRegistry {
    sessions: Arc<Mutex<HashMap<u64, ScrcpySession>>>,
}

struct ScrcpySession {
    serial: String,
    scid: String,
    video: Arc<Mutex<Option<TcpStream>>>,
    control: Arc<Mutex<TcpStream>>,
    child: Arc<Mutex<Child>>,
    max_fps: Option<u16>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrcpyStartRequest {
    serial: String,
    max_size: Option<u16>,
    video_bit_rate: Option<u32>,
    max_fps: Option<u16>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrcpyStartResult {
    session_id: u64,
    serial: String,
    device_name: String,
    codec: String,
    width: u32,
    height: u32,
    version: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrcpyTouchRequest {
    session_id: u64,
    action: String,
    pointer_id: Option<u64>,
    x: i32,
    y: i32,
    video_width: u16,
    video_height: u16,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ScrcpyStatusPayload {
    session_id: u64,
    status: String,
    message: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ScrcpyFramePayload {
    session_id: u64,
    pts: u64,
    key_frame: bool,
    config: bool,
    data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    height: Option<u32>,
}

fn get_scrcpy_server_path() -> &'static PathBuf {
    static SERVER_PATH: OnceLock<PathBuf> = OnceLock::new();
    SERVER_PATH.get_or_init(|| {
        #[cfg(has_embedded_scrcpy_server)]
        {
            let dir = std::env::temp_dir().join("droidpilot");
            fs::create_dir_all(&dir).expect("创建 scrcpy server 临时目录失败");
            let path = dir.join("scrcpy-server-v4.1");
            fs::write(&path, SCRCPY_SERVER).expect("提取 scrcpy server 失败");
            path
        }
        #[cfg(not(has_embedded_scrcpy_server))]
        {
            PathBuf::from("scrcpy-server-v4.1")
        }
    })
}

fn make_scid() -> u32 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or_default();
    ((now as u32) ^ std::process::id()) & 0x7fff_ffff
}

fn wait_child_output_with_timeout(
    mut child: Child,
    timeout: Duration,
    label: &str,
) -> Result<Output, String> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .map_err(|err| format!("读取 {label} 输出失败: {err}"));
            }
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let output = child
                    .wait_with_output()
                    .map_err(|err| format!("{label} 超时且清理失败: {err}"))?;
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                return Err(if stderr.is_empty() {
                    format!("{label} 超时")
                } else {
                    format!("{label} 超时: {stderr}")
                });
            }
            Ok(None) => thread::sleep(Duration::from_millis(40)),
            Err(err) => return Err(format!("等待 {label} 结束失败: {err}")),
        }
    }
}

fn run_adb_status(args: &[String]) -> Result<(), String> {
    let child = crate::adb_command()
        .args(args.iter().map(String::as_str))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("执行 adb 失败: {err}"))?;
    let output = wait_child_output_with_timeout(child, ADB_COMMAND_TIMEOUT, "adb 命令")?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Err(if stderr.is_empty() {
        if stdout.is_empty() {
            format!(
                "adb 命令执行失败，退出码 {}",
                output.status.code().unwrap_or(-1)
            )
        } else {
            stdout
        }
    } else {
        stderr
    })
}

fn accept_with_timeout(
    listener: &TcpListener,
    child: &Arc<Mutex<Child>>,
    label: &str,
) -> Result<TcpStream, String> {
    let deadline = Instant::now() + Duration::from_secs(12);
    listener
        .set_nonblocking(true)
        .map_err(|err| format!("设置 socket 非阻塞失败: {err}"))?;

    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream
                    .set_nonblocking(false)
                    .map_err(|err| format!("设置 {label} socket 阻塞模式失败: {err}"))?;
                stream
                    .set_nodelay(true)
                    .map_err(|err| format!("设置 {label} socket 失败: {err}"))?;
                return Ok(stream);
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                if let Ok(mut child) = child.lock() {
                    if let Ok(Some(status)) = child.try_wait() {
                        return Err(format!("scrcpy server 已退出，状态: {status}"));
                    }
                }
                if Instant::now() >= deadline {
                    return Err(format!("等待 scrcpy {label} 连接超时"));
                }
                thread::sleep(Duration::from_millis(40));
            }
            Err(err) => return Err(format!("接受 scrcpy {label} 连接失败: {err}")),
        }
    }
}

fn read_exact_timeout(stream: &mut TcpStream, buf: &mut [u8], label: &str) -> Result<(), String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(8)))
        .map_err(|err| format!("设置 {label} 读取超时失败: {err}"))?;
    let result = stream
        .read_exact(buf)
        .map_err(|err| format!("读取 scrcpy {label} 失败: {err}"));
    let _ = stream.set_read_timeout(None);
    result
}

fn read_device_name(stream: &mut TcpStream) -> Result<String, String> {
    let mut buf = [0u8; DEVICE_NAME_FIELD_LENGTH];
    read_exact_timeout(stream, &mut buf, "设备信息")?;
    let end = buf
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(buf.len());
    Ok(String::from_utf8_lossy(&buf[..end]).trim().to_string())
}

fn read_video_meta(stream: &mut TcpStream) -> Result<(String, u32, u32), String> {
    // v4: codec id followed by a 12-byte session header (flags, width, height).
    let mut buf = [0u8; 16];
    read_exact_timeout(stream, &mut buf, "视频元数据")?;
    let codec = String::from_utf8_lossy(&buf[0..4]).to_string();
    let (width, height) = parse_session_meta(&buf[4..])?;
    Ok((codec, width, height))
}

fn parse_session_meta(header: &[u8]) -> Result<(u32, u32), String> {
    if header.len() != 12 || header[0] & 0x80 == 0 {
        return Err("无效的 scrcpy 会话元数据".to_string());
    }
    let width = u32::from_be_bytes(header[4..8].try_into().unwrap());
    let height = u32::from_be_bytes(header[8..12].try_into().unwrap());
    if width == 0 || height == 0 || width > u16::MAX as u32 || height > u16::MAX as u32 {
        return Err(format!("无效的 scrcpy 视频尺寸: {width}x{height}"));
    }
    Ok((width, height))
}

fn emit_status(app: &AppHandle, session_id: u64, status: &str, message: impl Into<String>) {
    let _ = app.emit_to(
        "main",
        "scrcpy://status",
        ScrcpyStatusPayload {
            session_id,
            status: status.to_string(),
            message: message.into(),
        },
    );
}

fn read_video_packet(
    stream: &mut impl Read,
    session_id: u64,
) -> Result<ScrcpyFramePayload, String> {
    let mut header = [0u8; 12];
    stream
        .read_exact(&mut header)
        .map_err(|err| format!("读取视频包头失败: {err}"))?;

    if header[0] & 0x80 != 0 {
        let (width, height) = parse_session_meta(&header)?;
        return Ok(ScrcpyFramePayload {
            session_id,
            pts: 0,
            key_frame: false,
            config: false,
            data: String::new(),
            width: Some(width),
            height: Some(height),
        });
    }
    let pts_flags = u64::from_be_bytes([
        header[0], header[1], header[2], header[3], header[4], header[5], header[6], header[7],
    ]);
    let size = u32::from_be_bytes([header[8], header[9], header[10], header[11]]) as usize;
    let is_key_frame = (pts_flags & (1u64 << 61)) != 0;
    let is_config = (pts_flags & (1u64 << 62)) != 0;
    if size == 0 || size > 16 * 1024 * 1024 {
        return Err(format!("异常视频包大小: {size}"));
    }

    let mut data = vec![0u8; size];
    stream
        .read_exact(&mut data)
        .map_err(|err| format!("读取视频包失败: {err}"))?;

    Ok(ScrcpyFramePayload {
        session_id,
        pts: pts_flags & 0x1fff_ffff_ffff_ffff,
        key_frame: is_key_frame,
        config: is_config,
        data: general_purpose::STANDARD.encode(data),
        width: None,
        height: None,
    })
}

fn spawn_video_reader(
    app: AppHandle,
    registry: ScrcpyRegistry,
    session_id: u64,
    mut stream: TcpStream,
    _max_fps: Option<u16>,
) {
    thread::spawn(move || {
        loop {
            let payload = match read_video_packet(&mut stream, session_id) {
                Ok(payload) => payload,
                Err(err) => {
                    emit_status(&app, session_id, "stopped", format!("视频流已结束: {err}"));
                    break;
                }
            };

            if app.emit_to("main", "scrcpy://frame", payload).is_err() {
                break;
            }
        }

        let session = registry
            .sessions
            .lock()
            .ok()
            .and_then(|mut sessions| sessions.remove(&session_id));
        if let Some(session) = session {
            cleanup_session(session);
        }
    });
}

fn cleanup_session(session: ScrcpySession) {
    if let Ok(control) = session.control.lock() {
        let _ = control.shutdown(Shutdown::Both);
    }
    if let Ok(mut child) = session.child.lock() {
        let _ = child.kill();
        let _ = child.wait();
    }

    let _ = run_adb_status(&[
        "-s".to_string(),
        session.serial,
        "reverse".to_string(),
        "--remove".to_string(),
        format!("localabstract:scrcpy_{}", session.scid),
    ]);
}

fn start_scrcpy_mirror_impl(
    _app: AppHandle,
    registry: ScrcpyRegistry,
    request: ScrcpyStartRequest,
) -> Result<ScrcpyStartResult, String> {
    let serial = request.serial.trim().to_string();
    if serial.is_empty() {
        return Err("请先选择目标设备".to_string());
    }
    let server_path = get_scrcpy_server_path().clone();
    if !server_path.is_file() {
        return Err(format!(
            "scrcpy server 不存在: {}",
            server_path.to_string_lossy()
        ));
    }

    let scid_value = make_scid();
    let scid = format!("{scid_value:08x}");
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|err| format!("创建本地 scrcpy 监听失败: {err}"))?;
    let port = listener
        .local_addr()
        .map_err(|err| format!("读取 scrcpy 本地端口失败: {err}"))?
        .port();

    let reverse_name = format!("localabstract:scrcpy_{scid}");
    run_adb_status(&[
        "-s".to_string(),
        serial.clone(),
        "reverse".to_string(),
        reverse_name.clone(),
        format!("tcp:{port}"),
    ])?;
    if let Err(err) = run_adb_status(&[
        "-s".to_string(),
        serial.clone(),
        "push".to_string(),
        server_path.to_string_lossy().to_string(),
        DEVICE_SERVER_PATH.to_string(),
    ]) {
        let _ = run_adb_status(&[
            "-s".to_string(),
            serial.clone(),
            "reverse".to_string(),
            "--remove".to_string(),
            reverse_name.clone(),
        ]);
        return Err(err);
    }
    let mut args = vec![
        "-s".to_string(),
        serial.clone(),
        "shell".to_string(),
        format!("CLASSPATH={DEVICE_SERVER_PATH}"),
        "app_process".to_string(),
        "/".to_string(),
        "com.genymobile.scrcpy.Server".to_string(),
        SCRCPY_VERSION.to_string(),
        format!("scid={scid}"),
        "log_level=info".to_string(),
        "audio=false".to_string(),
        "control=true".to_string(),
        "video_codec=h264".to_string(),
        "clipboard_autosync=true".to_string(),
        "cleanup=false".to_string(),
    ];

    if let Some(max_size) = request.max_size.filter(|value| *value > 0) {
        args.push(format!("max_size={max_size}"));
    }
    if let Some(bit_rate) = request.video_bit_rate.filter(|value| *value > 0) {
        args.push(format!("video_bit_rate={bit_rate}"));
    }
    if let Some(max_fps) = request.max_fps.filter(|value| *value > 0) {
        args.push(format!("max_fps={max_fps}"));
    }

    let child = match crate::adb_command()
        .args(args.iter().map(String::as_str))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(err) => {
            let _ = run_adb_status(&[
                "-s".to_string(),
                serial.clone(),
                "reverse".to_string(),
                "--remove".to_string(),
                reverse_name.clone(),
            ]);
            return Err(format!("启动 scrcpy server 失败: {err}"));
        }
    };
    let child = Arc::new(Mutex::new(child));

    let startup_result = (|| {
        let mut video_stream = accept_with_timeout(&listener, &child, "video")?;
        let control_stream = accept_with_timeout(&listener, &child, "control")?;
        let device_name = read_device_name(&mut video_stream)?;
        let (codec, width, height) = read_video_meta(&mut video_stream)?;
        Ok((
            video_stream,
            control_stream,
            device_name,
            codec,
            width,
            height,
        ))
    })();
    let (video_stream, control_stream, device_name, codec, width, height) = match startup_result {
        Ok(result) => result,
        Err(err) => {
            if let Ok(mut child) = child.lock() {
                let _ = child.kill();
                let _ = child.wait();
            }
            let _ = run_adb_status(&[
                "-s".to_string(),
                serial.clone(),
                "reverse".to_string(),
                "--remove".to_string(),
                reverse_name,
            ]);
            return Err(err);
        }
    };

    let session = ScrcpySession {
        serial: serial.clone(),
        scid: scid.clone(),
        video: Arc::new(Mutex::new(Some(video_stream))),
        control: Arc::new(Mutex::new(control_stream)),
        child,
        max_fps: request.max_fps,
    };

    {
        let mut sessions = registry
            .sessions
            .lock()
            .map_err(|_| "scrcpy 会话状态已损坏".to_string())?;
        sessions.insert(scid_value as u64, session);
    }
    Ok(ScrcpyStartResult {
        session_id: scid_value as u64,
        serial,
        device_name,
        codec,
        width,
        height,
        version: SCRCPY_VERSION.to_string(),
    })
}

fn attach_scrcpy_mirror_impl(
    app: AppHandle,
    registry: ScrcpyRegistry,
    session_id: u64,
) -> Result<(), String> {
    let (video_stream, control_stream, max_fps) = {
        let sessions = registry
            .sessions
            .lock()
            .map_err(|_| "scrcpy 会话状态已损坏".to_string())?;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| "scrcpy 会话不存在或已停止".to_string())?;
        let mut video = session
            .video
            .lock()
            .map_err(|_| "scrcpy 视频通道已损坏".to_string())?;
        let control_stream = session
            .control
            .lock()
            .map_err(|_| "scrcpy 控制通道已损坏".to_string())?
            .try_clone()
            .map_err(|err| format!("复制控制通道失败: {err}"))?;
        let video_stream = video
            .take()
            .ok_or_else(|| "scrcpy 视频通道已接入".to_string())?;
        (video_stream, control_stream, session.max_fps)
    };

    emit_status(&app, session_id, "running", "scrcpy server 已连接");
    spawn_control_reader(app.clone(), session_id, control_stream);
    spawn_video_reader(app, registry, session_id, video_stream, max_fps);
    Ok(())
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScrcpyClipboardPayload {
    session_id: u64,
    text: String,
}

// Device messages are independent of outgoing control messages on the same socket.
fn read_device_message(stream: &mut impl Read) -> Result<Option<String>, String> {
    let read = |stream: &mut dyn Read, buf: &mut [u8]| {
        stream
            .read_exact(buf)
            .map_err(|err| format!("读取剪贴板消息失败: {err}"))
    };
    let mut kind = [0];
    read(stream, &mut kind)?;
    match kind[0] {
        0 => {
            let mut length = [0; 4];
            read(stream, &mut length)?;
            let length = u32::from_be_bytes(length) as usize;
            if length > CONTROL_MESSAGE_MAX_BYTES - 5 {
                return Err("手机剪贴板消息过大".into());
            }
            let mut text = vec![0; length];
            read(stream, &mut text)?;
            String::from_utf8(text)
                .map(Some)
                .map_err(|_| "手机剪贴板不是有效 UTF-8 文本".into())
        }
        1 => {
            read(stream, &mut [0; 8])?; // clipboard acknowledgement
            Ok(None)
        }
        2 => {
            let mut header = [0; 4]; // UHID id + payload length
            read(stream, &mut header)?;
            let mut data = vec![0; u16::from_be_bytes([header[2], header[3]]) as usize];
            read(stream, &mut data)?;
            Ok(None)
        }
        value => Err(format!("未知 scrcpy 设备消息: {value}")),
    }
}

fn spawn_control_reader(app: AppHandle, session_id: u64, mut stream: TcpStream) {
    thread::spawn(move || loop {
        match read_device_message(&mut stream) {
            Ok(Some(text)) => {
                if app
                    .emit_to(
                        "main",
                        "scrcpy://clipboard",
                        ScrcpyClipboardPayload { session_id, text },
                    )
                    .is_err()
                {
                    break;
                }
            }
            Ok(None) => {}
            Err(error) => {
                emit_status(&app, session_id, "control-error", error);
                break;
            }
        }
    });
}

fn serialize_clipboard(text: &str) -> Result<Vec<u8>, String> {
    if text.len() > CONTROL_MESSAGE_MAX_BYTES - 14 {
        return Err("剪贴板文本超过 256 KiB 协议限制，请分段粘贴".into());
    }
    let mut buf = Vec::with_capacity(14 + text.len());
    buf.push(9); // SET_CLIPBOARD
    write_u64be(&mut buf, 0); // no acknowledgement requested
    buf.push(1); // paste after setting clipboard; preserves Unicode
    write_u32be(&mut buf, text.len() as u32);
    buf.extend_from_slice(text.as_bytes());
    Ok(buf)
}

#[tauri::command]
pub async fn scrcpy_clipboard(
    registry: State<'_, ScrcpyRegistry>,
    session_id: u64,
    action: String,
) -> Result<(), String> {
    let registry = registry.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let payload = match action.as_str() {
            "copy" => vec![8, 1], // GET_CLIPBOARD + COPY
            "cut" => vec![8, 2],
            "paste" => serialize_clipboard(&crate::read_text_from_clipboard()?)?,
            _ => return Err("不支持的剪贴板操作".into()),
        };
        with_control_stream(&registry, session_id, |stream| {
            stream
                .write_all(&payload)
                .map_err(|err| format!("发送剪贴板操作失败: {err}"))
        })
    })
    .await
    .map_err(|err| format!("剪贴板任务失败: {err}"))?
}

#[tauri::command]
pub async fn start_scrcpy_mirror(
    app: AppHandle,
    registry: State<'_, ScrcpyRegistry>,
    request: ScrcpyStartRequest,
) -> Result<ScrcpyStartResult, String> {
    let registry = registry.inner().clone();
    tauri::async_runtime::spawn_blocking(move || start_scrcpy_mirror_impl(app, registry, request))
        .await
        .map_err(|err| format!("scrcpy 启动任务失败: {err}"))?
}

#[tauri::command]
pub async fn attach_scrcpy_mirror(
    app: AppHandle,
    registry: State<'_, ScrcpyRegistry>,
    session_id: u64,
) -> Result<(), String> {
    let registry = registry.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        attach_scrcpy_mirror_impl(app, registry, session_id)
    })
    .await
    .map_err(|err| format!("scrcpy 接入任务失败: {err}"))?
}

#[tauri::command]
pub async fn stop_scrcpy_mirror(
    registry: State<'_, ScrcpyRegistry>,
    session_id: u64,
) -> Result<(), String> {
    let registry = registry.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let session = {
            let mut sessions = registry
                .sessions
                .lock()
                .map_err(|_| "scrcpy 会话状态已损坏".to_string())?;
            sessions.remove(&session_id)
        };

        if let Some(session) = session {
            cleanup_session(session);
        }
        Ok(())
    })
    .await
    .map_err(|err| format!("scrcpy 停止任务失败: {err}"))?
}

fn with_control_stream<F>(
    registry: &ScrcpyRegistry,
    session_id: u64,
    mut write: F,
) -> Result<(), String>
where
    F: FnMut(&mut TcpStream) -> Result<(), String>,
{
    let control = {
        let sessions = registry
            .sessions
            .lock()
            .map_err(|_| "scrcpy 会话状态已损坏".to_string())?;
        sessions
            .get(&session_id)
            .map(|session| session.control.clone())
            .ok_or_else(|| "scrcpy 会话不存在或已停止".to_string())?
    };

    let mut stream = control
        .lock()
        .map_err(|_| "scrcpy 控制通道已损坏".to_string())?;
    write(&mut stream)
}

fn write_u16be(buf: &mut Vec<u8>, value: u16) {
    buf.extend_from_slice(&value.to_be_bytes());
}

fn write_u32be(buf: &mut Vec<u8>, value: u32) {
    buf.extend_from_slice(&value.to_be_bytes());
}

fn write_u64be(buf: &mut Vec<u8>, value: u64) {
    buf.extend_from_slice(&value.to_be_bytes());
}

fn serialize_touch(request: ScrcpyTouchRequest) -> Result<Vec<u8>, String> {
    let action = match request.action.as_str() {
        "down" => 0u8,
        "up" => 1u8,
        "move" => 2u8,
        _ => return Err("不支持的触控动作".to_string()),
    };

    if request.video_width == 0 || request.video_height == 0 {
        return Err("触控区域尺寸不能为空".to_string());
    }
    let x = request.x.clamp(0, request.video_width as i32 - 1);
    let y = request.y.clamp(0, request.video_height as i32 - 1);
    let pressure = if action == 1 { 0 } else { u16::MAX };

    let mut buf = Vec::with_capacity(32);
    buf.push(2);
    buf.push(action);
    write_u64be(
        &mut buf,
        request.pointer_id.unwrap_or(POINTER_ID_GENERIC_FINGER),
    );
    write_u32be(&mut buf, x as u32);
    write_u32be(&mut buf, y as u32);
    write_u16be(&mut buf, request.video_width);
    write_u16be(&mut buf, request.video_height);
    write_u16be(&mut buf, pressure);
    write_u32be(&mut buf, 0);
    write_u32be(&mut buf, 0);
    Ok(buf)
}

#[tauri::command]
pub async fn scrcpy_touch(
    registry: State<'_, ScrcpyRegistry>,
    request: ScrcpyTouchRequest,
) -> Result<(), String> {
    let registry = registry.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let session_id = request.session_id;
        let payload = serialize_touch(request)?;
        with_control_stream(&registry, session_id, |stream| {
            stream
                .write_all(&payload)
                .map_err(|err| format!("发送触控事件失败: {err}"))
        })
    })
    .await
    .map_err(|err| format!("触控任务失败: {err}"))?
}

fn serialize_keycode(action: u8, keycode: u32) -> Vec<u8> {
    let mut buf = Vec::with_capacity(14);
    buf.push(0);
    buf.push(action);
    write_u32be(&mut buf, keycode);
    write_u32be(&mut buf, 0);
    write_u32be(&mut buf, 0);
    buf
}

fn truncate_utf8(value: &str, max_bytes: usize) -> &str {
    if value.len() <= max_bytes {
        return value;
    }

    let mut end = max_bytes;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

fn serialize_text(text: &str) -> Vec<u8> {
    let text = truncate_utf8(text, INJECT_TEXT_MAX_BYTES);
    let bytes = text.as_bytes();
    let mut buf = Vec::with_capacity(1 + 4 + bytes.len());
    buf.push(1);
    write_u32be(&mut buf, bytes.len() as u32);
    buf.extend_from_slice(bytes);
    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn clipboard_unicode_wire_format_and_limits() {
        let text = "中文🙂\nhello";
        let packet = serialize_clipboard(text).unwrap();
        assert_eq!(&packet[..10], &[9, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(
            u32::from_be_bytes(packet[10..14].try_into().unwrap()) as usize,
            text.len()
        );
        assert_eq!(&packet[14..], text.as_bytes());
        assert!(serialize_clipboard(&"a".repeat(CONTROL_MESSAGE_MAX_BYTES - 14)).is_ok());
        assert!(serialize_clipboard(&"a".repeat(CONTROL_MESSAGE_MAX_BYTES - 13)).is_err());
        let mut wire = vec![1, 0, 0, 0, 0, 0, 0, 0, 1, 2, 0, 1, 0, 2, 42, 43, 0];
        wire.extend_from_slice(&(text.len() as u32).to_be_bytes());
        wire.extend_from_slice(text.as_bytes());
        wire.extend_from_slice(&[0, 0, 0, 0, 0]);
        let mut stream = Cursor::new(wire);
        assert_eq!(read_device_message(&mut stream).unwrap(), None);
        assert_eq!(read_device_message(&mut stream).unwrap(), None);
        assert_eq!(read_device_message(&mut stream).unwrap(), Some(text.into()));
        assert_eq!(
            read_device_message(&mut stream).unwrap(),
            Some(String::new())
        );
    }

    #[test]
    fn rejects_invalid_device_messages() {
        for bytes in [
            vec![3],
            vec![0, 0, 4, 0, 0],
            vec![0, 0, 0, 0, 2, 65],
            vec![0, 0, 0, 0, 1, 255],
        ] {
            assert!(read_device_message(&mut Cursor::new(bytes)).is_err());
        }
    }

    #[test]
    fn v4_session_header_does_not_consume_next_packet() {
        let mut wire = vec![0x80, 0, 0, 1, 0, 0, 4, 0, 0, 0, 3, 0];
        wire.extend_from_slice(&((1u64 << 61) | 123).to_be_bytes());
        wire.extend_from_slice(&3u32.to_be_bytes());
        wire.extend_from_slice(&[0, 0, 1]);
        let mut stream = Cursor::new(wire);
        let session = read_video_packet(&mut stream, 7).unwrap();
        assert_eq!((session.width, session.height), (Some(1024), Some(768)));
        let frame = read_video_packet(&mut stream, 7).unwrap();
        assert!(frame.key_frame);
        assert!(!frame.config);
        assert_eq!(frame.pts, 123);
        assert_eq!(frame.data, "AAAB");
    }

    #[test]
    fn v4_config_flag_and_invalid_packets() {
        let mut wire = (1u64 << 62).to_be_bytes().to_vec();
        wire.extend_from_slice(&1u32.to_be_bytes());
        wire.push(0x67);
        let frame = read_video_packet(&mut Cursor::new(wire), 1).unwrap();
        assert!(frame.config);
        assert!(!frame.key_frame);
        assert_eq!(frame.pts, 0);
        assert!(read_video_packet(&mut Cursor::new(vec![0; 12]), 1).is_err());
        assert!(parse_session_meta(&[0x80; 12]).is_err());
        assert!(parse_session_meta(&[0; 12]).is_err());
    }

    #[test]
    fn touch_matches_upstream_wire_layout() {
        let request = |action: &str| ScrcpyTouchRequest {
            session_id: 1,
            action: action.into(),
            pointer_id: Some(2),
            x: 1080,
            y: -1,
            video_width: 1080,
            video_height: 1920,
        };
        let down = serialize_touch(request("down")).unwrap();
        assert_eq!(
            down,
            vec![
                2, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 4, 55, 0, 0, 0, 0, 4, 56, 7, 128, 255, 255, 0,
                0, 0, 0, 0, 0, 0, 0,
            ]
        );
        let up = serialize_touch(request("up")).unwrap();
        assert_eq!(up[1], 1);
        assert_eq!(&up[22..24], &[0, 0]);
        assert!(serialize_touch(request("invalid")).is_err());
    }
}

#[tauri::command]
pub async fn scrcpy_keyevent(
    registry: State<'_, ScrcpyRegistry>,
    session_id: u64,
    keycode: u32,
) -> Result<(), String> {
    let registry = registry.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let down = serialize_keycode(0, keycode);
        let up = serialize_keycode(1, keycode);
        with_control_stream(&registry, session_id, |stream| {
            stream
                .write_all(&down)
                .and_then(|_| {
                    thread::sleep(Duration::from_millis(16));
                    stream.write_all(&up)
                })
                .map_err(|err| format!("发送按键事件失败: {err}"))
        })
    })
    .await
    .map_err(|err| format!("按键任务失败: {err}"))?
}

#[tauri::command]
pub async fn scrcpy_text(
    registry: State<'_, ScrcpyRegistry>,
    session_id: u64,
    text: String,
) -> Result<(), String> {
    if text.is_empty() {
        return Ok(());
    }

    let registry = registry.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let payload = serialize_text(&text);
        with_control_stream(&registry, session_id, |stream| {
            stream
                .write_all(&payload)
                .map_err(|err| format!("发送文本输入失败: {err}"))
        })
    })
    .await
    .map_err(|err| format!("文本输入任务失败: {err}"))?
}
