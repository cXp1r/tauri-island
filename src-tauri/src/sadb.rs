//! ADB screen mirroring module.
//!
//! Provides Tauri commands for Android screen mirroring via scrcpy protocol,
//! reusing the `sadb-core` library.

use std::io::{BufReader, Read};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use sadb_core::control::{
    InjectKeycodeEvent, InjectScrollEvent, InjectTextEvent, InjectTouchEvent,
    KeyEventAction, MotionEventAction, MotionEventButtons, POINTER_ID_MOUSE,
    SetClipboard,
};
use sadb_core::protocol::VideoCodec;
use sadb_core::{Config, DeviceMessage, ScrcpyClient};
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use crate::logger;
use crate::IslandState;

/// Event emitted to the frontend over the IPC channel.
#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum PacketEvent {
    Meta {
        device_name: String,
        codec: String,
        width: u32,
        height: u32,
    },
    Packet {
        pts: u64,
        key_frame: bool,
        config: bool,
        data: String,
    },
    AudioPacket {
        pts: u64,
        config: bool,
        data: String,
    },
    Error { message: String },
    Closed,
    Clipboard { text: String },
}

/// Per-session handle kept in app state.
pub(crate) struct SessionHandle {
    pub stop: Arc<AtomicBool>,
    pub join_video: Option<std::thread::JoinHandle<()>>,
    pub join_audio: Option<std::thread::JoinHandle<()>>,
    pub join_control: Option<std::thread::JoinHandle<()>>,
    pub client: ScrcpyClient,
}

fn codec_to_str(c: VideoCodec) -> &'static str {
    match c {
        VideoCodec::H264 => "h264",
        VideoCodec::H265 => "h265",
        VideoCodec::AV1 => "av1",
    }
}

/// Locate the scrcpy-server jar.
fn resolve_server_jar(app: &AppHandle) -> PathBuf {
    if let Ok(p) = std::env::var("SADB_SERVER_JAR") {
        logger::info("SADB", &format!("using SADB_SERVER_JAR env var: path={}", p));
        return PathBuf::from(p);
    }
    if let Ok(r) = app.path().resource_dir() {
        let p = r.join("resources/scrcpy-server-v3.3.4");
        if p.exists() {
            logger::info("SADB", &format!("using resource dir server jar: path={}", p.display()));
            return p;
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        for anc in exe.ancestors().take(8) {
            let p = anc.join("assets").join("scrcpy-server-v3.3.4");
            if p.exists() {
                logger::info("SADB", &format!("using exe-relative server jar: path={}", p.display()));
                return p;
            }
        }
    }
    logger::warn("SADB", "scrcpy-server jar not found in any standard location, using default path");
    PathBuf::from("assets/scrcpy-server-v3.3.4")
}

#[tauri::command]
pub(crate) async fn sadb_start_mirroring(
    app: AppHandle,
    state: State<'_, IslandState>,
    channel: Channel<PacketEvent>,
    max_size: Option<u32>,
    bitrate: Option<u32>,
    serial: Option<String>,
) -> Result<(), String> {
    logger::info("SADB", &format!("sadb initialization requested: serial={:?}, max_size={:?}, bitrate={:?}", serial, max_size, bitrate));
    // 先杀掉已有 session（保证干净状态）
    {
        logger::debug("SADB", "checking existing sadb session before initialization");
        let mut guard = state.sadb_session.lock().await;
        if let Some(mut session) = guard.take() {
            logger::info("SADB", "killing existing session before starting new one");
            session.stop.store(true, Ordering::Relaxed);
            if let Err(e) = session.client.cleanup().await {
                logger::warn("SADB", &format!("cleanup error for existing session: {}", e));
            }
            logger::info("SADB", "existing session killed");
        } else {
            logger::debug("SADB", "no existing sadb session found");
        }
    }
    state.sadb_mirroring.store(false, Ordering::Relaxed);
    logger::debug("SADB", "sadb mirroring state reset");

    let server_jar = resolve_server_jar(&app);
    logger::info("SADB", &format!("resolved scrcpy-server jar: path={}, exists={}", server_jar.display(), server_jar.exists()));
    if !server_jar.exists() {
        logger::error("SADB", &format!("scrcpy-server jar not found: path={}", server_jar.display()));
        return Err(format!("scrcpy-server jar not found at {}", server_jar.display()));
    }

    let bitrate_val = bitrate.or(Some(8_000_000));
    let adb_path = state.adb_path.lock().unwrap().trim().to_string();
    logger::debug("SADB", &format!("loaded adb path from settings: {}", adb_path));
    let adb_path = if adb_path.is_empty() {
        logger::info("SADB", "no custom adb path configured, falling back to PATH adb");
        None
    } else {
        logger::info("SADB", &format!("using custom adb path for sadb: adb_path={}", adb_path));
        Some(adb_path)
    };

    let cfg = Config {
        server_jar_path: server_jar.to_string_lossy().into_owned(),
        audio: true,
        control: true,
        max_size,
        video_bitrate: bitrate_val,
        serial: serial.clone(),
        adb_path: adb_path.clone(),
        ..Config::default()
    };

    logger::info("SADB", &format!("starting mirroring session: serial={:?}, max_size={:?}, bitrate={:?}, adb_path={:?}, audio=true, control=true", serial, max_size, bitrate_val, adb_path));

    logger::debug("SADB", "creating ScrcpyClient");
    let mut client = ScrcpyClient::new(cfg).await.map_err(|e| {
        logger::error("SADB", &format!("failed to create ScrcpyClient: {}", e));
        e.to_string()
    })?;

    logger::info("SADB", &format!("ScrcpyClient created: scid={}", client.scid()));
    logger::debug("SADB", &format!("starting scrcpy server: scid={}", client.scid()));
    client.start().await.map_err(|e| {
        logger::error("SADB", &format!("failed to start scrcpy: {}", e));
        e.to_string()
    })?;
    logger::info("SADB", &format!("scrcpy server started: scid={}", client.scid()));

    logger::debug("SADB", &format!("reading device metadata: scid={}", client.scid()));
    let device_meta = client.read_device_metadata().map_err(|e| {
        logger::error("SADB", &format!("failed to read device metadata: {}", e));
        e.to_string()
    })?;
    logger::info("SADB", &format!("device metadata received: device_name={}", device_meta.name));

    logger::debug("SADB", &format!("reading video codec metadata: scid={}", client.scid()));
    let codec_meta = client.read_video_codec_metadata().map_err(|e| {
        logger::error("SADB", &format!("failed to read video codec metadata: {}", e));
        e.to_string()
    })?;
    logger::info("SADB", &format!("video codec metadata received: codec={:?}, width={}, height={}", codec_meta.codec, codec_meta.width, codec_meta.height));

    // ── Audio metadata ──
    let audio_meta = client.read_audio_codec_metadata();
    match &audio_meta {
        Ok(m) => logger::info("SADB", &format!("audio codec metadata received: codec={:?}", m.codec)),
        Err(e) => logger::warn("SADB", &format!("audio metadata unavailable (device may not support audio capture): {}", e)),
    }

    channel
        .send(PacketEvent::Meta {
            device_name: device_meta.name,
            codec: codec_to_str(codec_meta.codec).to_string(),
            width: codec_meta.width,
            height: codec_meta.height,
        })
        .map_err(|e| {
            logger::error("SADB", &format!("failed to send meta event to frontend: {}", e));
            e.to_string()
        })?;

    let mut video_stream = client.video_stream().map_err(|e| {
        logger::error("SADB", &format!("failed to get video stream: {}", e));
        e.to_string()
    })?;
    logger::info("SADB", "video stream obtained");

    let audio_stream = client.audio_stream().ok();
    match &audio_stream {
        Some(_) => logger::info("SADB", "audio stream obtained"),
        None => logger::warn("SADB", "audio stream unavailable — no audio will be sent"),
    }

    let stop = Arc::new(AtomicBool::new(false));

    // ── Video reader thread ──
    let stop_video = stop.clone();
    let channel_video = channel.clone();
    let join_video = std::thread::Builder::new()
        .name("sadb-video-reader".into())
        .spawn(move || {
            logger::info("SADB", "video reader thread started");
            let mut packet_count: u64 = 0;
            loop {
                if stop_video.load(Ordering::Relaxed) {
                    logger::info("SADB", &format!("video reader stopping: packets={}", packet_count));
                    break;
                }
                match video_stream.read_packet() {
                    Ok(Some(pkt)) => {
                        packet_count += 1;
                        if packet_count % 300 == 0 {
                            logger::debug("SADB", &format!("video packet: packets={}, pts={}, key_frame={}", packet_count, pkt.header.pts, pkt.header.key_frame));
                        }
                        let evt = PacketEvent::Packet {
                            pts: pkt.header.pts,
                            key_frame: pkt.header.key_frame,
                            config: pkt.header.config_packet,
                            data: B64.encode(&pkt.data),
                        };
                        if let Err(e) = channel_video.send(evt) {
                            logger::error("SADB", &format!("video channel send failed: {}", e));
                            break;
                        }
                    }
                    Ok(None) => continue,
                    Err(e) => {
                        logger::error("SADB", &format!("video read_packet error: {}", e));
                        let _ = channel_video.send(PacketEvent::Error {
                            message: e.to_string(),
                        });
                        break;
                    }
                }
            }
            let _ = channel_video.send(PacketEvent::Closed);
            logger::info("SADB", "video reader thread finished");
        })
        .map_err(|e| format!("failed to spawn video reader thread: {}", e))?;

    // ── Audio reader thread ──
    let join_audio = if let Some(mut audio_stream) = audio_stream {
        let stop_audio = stop.clone();
        let channel_audio = channel.clone();
        Some(
            std::thread::Builder::new()
                .name("sadb-audio-reader".into())
                .spawn(move || {
                    logger::info("SADB", "audio reader thread started");
                    let mut packet_count: u64 = 0;
                    loop {
                        if stop_audio.load(Ordering::Relaxed) {
                            logger::info("SADB", &format!("audio reader stopping: packets={}", packet_count));
                            break;
                        }
                        match audio_stream.read_packet() {
                            Ok(Some(pkt)) => {
                                packet_count += 1;
                                if packet_count <= 5 || packet_count % 200 == 0 {
                                    logger::debug("SADB", &format!("audio packet: packets={}, pts={}, config={}, size={}", packet_count, pkt.header.pts, pkt.header.config_packet, pkt.data.len()));
                                }
                                let evt = PacketEvent::AudioPacket {
                                    pts: pkt.header.pts,
                                    config: pkt.header.config_packet,
                                    data: B64.encode(&pkt.data),
                                };
                                if let Err(e) = channel_audio.send(evt) {
                                    logger::error("SADB", &format!("audio channel send failed: {}", e));
                                    break;
                                }
                            }
                            Ok(None) => {
                                logger::debug("SADB", "audio read_packet returned None, continuing");
                                continue;
                            }
                            Err(e) => {
                                logger::error("SADB", &format!("audio read_packet error: {}", e));
                                break;
                            }
                        }
                    }
                    logger::info("SADB", "audio reader thread finished");
                })
                .expect("failed to spawn audio reader thread"),
        )
    } else {
        logger::warn("SADB", "no audio stream available, skipping audio reader thread");
        None
    };

    // ── Control socket reader thread ──
    let control_socket = client.control_socket_clone().ok();
    let join_control = if let Some(ctrl) = control_socket {
        let stop_ctrl = stop.clone();
        let channel_ctrl = channel.clone();
        Some(
            std::thread::Builder::new()
                .name("sadb-control-reader".into())
                .spawn(move || {
                    logger::info("SADB", "control reader thread started");
                    let mut reader = BufReader::with_capacity(64 * 1024, ctrl);
                    let mut buf = Vec::new();
                    loop {
                        if stop_ctrl.load(Ordering::Relaxed) {
                            logger::info("SADB", "control reader stopping");
                            break;
                        }
                        let mut temp = [0u8; 4096];
                        match reader.read(&mut temp) {
                            Ok(0) => {
                                logger::debug("SADB", "control socket closed (read 0)");
                                break;
                            }
                            Ok(n) => {
                                logger::debug("SADB", &format!("control data received: bytes={}", n));
                                buf.extend_from_slice(&temp[..n]);
                                loop {
                                    match DeviceMessage::deserialize(&buf) {
                                        Ok(Some((msg, consumed))) => {
                                            buf.drain(..consumed);
                                            if let DeviceMessage::Clipboard { text } = msg {
                                                logger::debug("SADB", &format!("clipboard message from device: text_len={}", text.len()));
                                                let _ = channel_ctrl.send(PacketEvent::Clipboard { text });
                                            }
                                        }
                                        Ok(None) => break,
                                        Err(e) => {
                                            logger::warn("SADB", &format!("device msg parse error, clearing buffer: {}", e));
                                            buf.clear();
                                            break;
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                logger::error("SADB", &format!("control read error: {}", e));
                                break;
                            }
                        }
                    }
                    logger::info("SADB", "control reader thread finished");
                })
                .expect("failed to spawn control reader thread"),
        )
    } else {
        logger::debug("SADB", "no control socket available");
        None
    };

    *state.sadb_session.lock().await = Some(SessionHandle {
        stop,
        join_video: Some(join_video),
        join_audio,
        join_control,
        client,
    });

    state.sadb_mirroring.store(true, Ordering::Relaxed);
    logger::info("SADB", "mirroring session fully established");
    Ok(())
}

#[tauri::command]
pub(crate) async fn sadb_send_touch_event(
    state: State<'_, IslandState>,
    x: i32,
    y: i32,
    screen_width: u16,
    screen_height: u16,
    action: u8,
    buttons: u32,
) -> Result<(), String> {
    let guard = state.sadb_session.lock().await;
    if let Some(ref session) = *guard {
        let act = match action {
            0 => MotionEventAction::Down,
            1 => MotionEventAction::Up,
            _ => MotionEventAction::Move,
        };
        let btn = MotionEventButtons(buttons);
        let (action_button, held_buttons, pressure) = match act {
            MotionEventAction::Down => (btn, btn, 1.0f32),
            MotionEventAction::Up => (btn, MotionEventButtons::NONE, 0.0f32),
            MotionEventAction::Move => (MotionEventButtons::NONE, btn, 1.0f32),
        };
        let msg = InjectTouchEvent {
            action: act,
            pointer_id: POINTER_ID_MOUSE,
            x,
            y,
            screen_width,
            screen_height,
            pressure,
            action_button,
            buttons: held_buttons,
        };
        session
            .client
            .send_control_msg(&msg.serialize())
            .map_err(|e| {
                logger::error("SADB", &format!("send touch event failed: {}", e));
                e.to_string()
            })?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn sadb_send_keycode(
    state: State<'_, IslandState>,
    action: u8,
    keycode: i32,
    metastate: u32,
) -> Result<(), String> {
    let guard = state.sadb_session.lock().await;
    if let Some(ref session) = *guard {
        let act = match action {
            1 => KeyEventAction::Up,
            _ => KeyEventAction::Down,
        };
        let msg = InjectKeycodeEvent {
            action: act,
            keycode,
            repeat: 0,
            metastate,
        };
        logger::debug("SADB", &format!("send keycode: action={}, keycode={}, metastate={}", action, keycode, metastate));
        session
            .client
            .send_control_msg(&msg.serialize())
            .map_err(|e| {
                logger::error("SADB", &format!("send keycode failed: {}", e));
                e.to_string()
            })?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn sadb_inject_text(
    state: State<'_, IslandState>,
    text: String,
) -> Result<(), String> {
    let guard = state.sadb_session.lock().await;
    if let Some(ref session) = *guard {
        logger::debug("SADB", &format!("inject text: text_len={}", text.len()));
        let msg = InjectTextEvent { text };
        session
            .client
            .send_control_msg(&msg.serialize())
            .map_err(|e| {
                logger::error("SADB", &format!("inject text failed: {}", e));
                e.to_string()
            })?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn sadb_connect_device(serial: String) -> Result<(), String> {
    logger::info("SADB", &format!("connecting to device via ADB: serial={}", serial));
    sadb_core::adb::AdbClient::connect(&serial)
        .await
        .map_err(|e| {
            logger::error("SADB", &format!("ADB connect failed: serial={}, error={}", serial, e));
            e.to_string()
        })?;
    logger::info("SADB", &format!("ADB connect succeeded: serial={}", serial));
    Ok(())
}

#[tauri::command]
pub(crate) async fn sadb_disconnect_device(serial: String) -> Result<(), String> {
    logger::info("SADB", &format!("disconnecting device: serial={}", serial));
    sadb_core::adb::AdbClient::disconnect(&serial)
        .await
        .map_err(|e| {
            logger::warn("SADB", &format!("ADB disconnect failed: serial={}, error={}", serial, e));
            e.to_string()
        })?;
    logger::info("SADB", &format!("ADB disconnect succeeded: serial={}", serial));
    Ok(())
}

#[tauri::command]
pub(crate) async fn sadb_set_clipboard(
    state: State<'_, IslandState>,
    text: String,
    paste: bool,
) -> Result<(), String> {
    let guard = state.sadb_session.lock().await;
    if let Some(ref session) = *guard {
        logger::debug("SADB", &format!("set clipboard: text_len={}, paste={}", text.len(), paste));
        let msg = SetClipboard { text, paste };
        session
            .client
            .send_control_msg(&msg.serialize())
            .map_err(|e| {
                logger::error("SADB", &format!("set clipboard failed: {}", e));
                e.to_string()
            })?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn sadb_send_scroll_event(
    state: State<'_, IslandState>,
    x: i32,
    y: i32,
    screen_width: u16,
    screen_height: u16,
    hscroll: f32,
    vscroll: f32,
) -> Result<(), String> {
    let guard = state.sadb_session.lock().await;
    if let Some(ref session) = *guard {
        let msg = InjectScrollEvent {
            x,
            y,
            screen_width,
            screen_height,
            hscroll,
            vscroll,
            buttons: MotionEventButtons::NONE,
        };
        session
            .client
            .send_control_msg(&msg.serialize())
            .map_err(|e| {
                logger::error("SADB", &format!("send scroll event failed: {}", e));
                e.to_string()
            })?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn sadb_stop_mirroring(state: State<'_, IslandState>) -> Result<(), String> {
    state.sadb_mirroring.store(false, Ordering::Relaxed);
    logger::info("SADB", "stopping mirroring session");
    let session = state.sadb_session.lock().await.take();
    if let Some(mut session) = session {
        session.stop.store(true, Ordering::Relaxed);

        if let Err(e) = session.client.cleanup().await {
            logger::warn("SADB", &format!("cleanup error: {}", e));
        }

        if let Some(join) = session.join_video.take() {
            let _ = tokio::task::spawn_blocking(move || {
                let _ = join.join();
            })
            .await;
            logger::info("SADB", "video thread joined");
        }
        if let Some(join) = session.join_audio.take() {
            let _ = tokio::task::spawn_blocking(move || {
                let _ = join.join();
            })
            .await;
            logger::info("SADB", "audio thread joined");
        }
        if let Some(join) = session.join_control.take() {
            let _ = tokio::task::spawn_blocking(move || {
                let _ = join.join();
            })
            .await;
            logger::info("SADB", "control thread joined");
        }

        logger::info("SADB", "mirroring session stopped");
    } else {
        logger::debug("SADB", "stop_mirroring called but no active session");
    }
    Ok(())
}
