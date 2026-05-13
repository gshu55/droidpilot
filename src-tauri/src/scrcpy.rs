use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Output, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, State};

const SCRCPY_VERSION: &str = "3.3.4";
const DEVICE_SERVER_PATH: &str = "/data/local/tmp/scrcpy-server.jar";
const DEVICE_NAME_FIELD_LENGTH: usize = 64;
const POINTER_ID_GENERIC_FINGER: u64 = u64::MAX - 1;
const ADB_COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

#[cfg(has_embedded_scrcpy_server)]
static SCRCPY_SERVER: &[u8] = include_bytes!("../binaries/scrcpy/scrcpy-server-v3.3.4");

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
}

fn get_scrcpy_server_path() -> &'static PathBuf {
    static SERVER_PATH: OnceLock<PathBuf> = OnceLock::new();
    SERVER_PATH.get_or_init(|| {
        #[cfg(has_embedded_scrcpy_server)]
        {
            let dir = std::env::temp_dir().join("droidpilot");
            fs::create_dir_all(&dir).expect("创建 scrcpy server 临时目录失败");
            let path = dir.join("scrcpy-server-v3.3.4");
            fs::write(&path, SCRCPY_SERVER).expect("提取 scrcpy server 失败");
            path
        }
        #[cfg(not(has_embedded_scrcpy_server))]
        {
            PathBuf::from("scrcpy-server-v3.3.4")
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
    let mut buf = [0u8; 12];
    read_exact_timeout(stream, &mut buf, "视频元数据")?;
    let codec = String::from_utf8_lossy(&buf[0..4]).to_string();
    let width = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
    let height = u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]);
    Ok((codec, width, height))
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
    stream: &mut TcpStream,
    session_id: u64,
) -> Result<ScrcpyFramePayload, String> {
    let mut header = [0u8; 12];
    stream
        .read_exact(&mut header)
        .map_err(|err| format!("读取视频包头失败: {err}"))?;

    let pts_flags = u64::from_be_bytes([
        header[0], header[1], header[2], header[3], header[4], header[5], header[6], header[7],
    ]);
    let size = u32::from_be_bytes([header[8], header[9], header[10], header[11]]) as usize;
    let is_key_frame = (pts_flags & (1u64 << 62)) != 0;
    let is_config = (pts_flags & (1u64 << 63)) != 0;
    if size == 0 || size > 16 * 1024 * 1024 {
        return Err(format!("异常视频包大小: {size}"));
    }

    let mut data = vec![0u8; size];
    stream
        .read_exact(&mut data)
        .map_err(|err| format!("读取视频包失败: {err}"))?;

    Ok(ScrcpyFramePayload {
        session_id,
        pts: pts_flags & 0x3fff_ffff_ffff_ffff,
        key_frame: is_key_frame,
        config: is_config,
        data: general_purpose::STANDARD.encode(data),
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
        "video_codec=h264".to_string(),
        "clipboard_autosync=false".to_string(),
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
    let (video_stream, max_fps) = {
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
        let video_stream = video
            .take()
            .ok_or_else(|| "scrcpy 视频通道已接入".to_string())?;
        (video_stream, session.max_fps)
    };

    emit_status(&app, session_id, "running", "scrcpy server 已连接");
    spawn_video_reader(app, registry, session_id, video_stream, max_fps);
    Ok(())
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

    let x = request.x.clamp(0, request.video_width as i32);
    let y = request.y.clamp(0, request.video_height as i32);
    let pressure = if action == 1 { 0 } else { u16::MAX };

    let mut buf = Vec::with_capacity(32);
    buf.push(2);
    buf.push(action);
    write_u64be(&mut buf, POINTER_ID_GENERIC_FINGER);
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
