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
use tracing::{debug, error, info, warn};

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
        info!(path=%p, "using SADB_SERVER_JAR env var");
        return PathBuf::from(p);
    }
    if let Ok(r) = app.path().resource_dir() {
        let p = r.join("resources/scrcpy-server-v3.3.4");
        if p.exists() {
            info!(path=%p.display(), "using resource dir server jar");
            return p;
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        for anc in exe.ancestors().take(8) {
            let p = anc.join("assets").join("scrcpy-server-v3.3.4");
            if p.exists() {
                info!(path=%p.display(), "using exe-relative server jar");
                return p;
            }
        }
    }
    warn!("scrcpy-server jar not found in any standard location, using default path");
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
    // 先杀掉已有 session（保证干净状态）
    {
        let mut guard = state.sadb_session.lock().await;
        if let Some(mut session) = guard.take() {
            info!("killing existing session before starting new one");
            session.stop.store(true, Ordering::Relaxed);
            if let Err(e) = session.client.cleanup().await {
                warn!(%e, "cleanup error for existing session");
            }
            info!("existing session killed");
        }
    }
    state.sadb_mirroring.store(false, Ordering::Relaxed);

    let server_jar = resolve_server_jar(&app);
    if !server_jar.exists() {
        error!(path=%server_jar.display(), "scrcpy-server jar not found");
        return Err(format!("scrcpy-server jar not found at {}", server_jar.display()));
    }

    let bitrate_val = bitrate.or(Some(8_000_000));

    let cfg = Config {
        server_jar_path: server_jar.to_string_lossy().into_owned(),
        audio: true,
        control: true,
        max_size,
        video_bitrate: bitrate_val,
        serial: serial.clone(),
        ..Config::default()
    };

    info!(
        serial = ?serial,
        max_size = ?max_size,
        bitrate = bitrate_val,
        audio = true,
        control = true,
        "starting mirroring session"
    );

    let mut client = ScrcpyClient::new(cfg).await.map_err(|e| {
        error!(%e, "failed to create ScrcpyClient");
        e.to_string()
    })?;

    info!("ScrcpyClient created, starting...");
    client.start().await.map_err(|e| {
        error!(%e, "failed to start scrcpy");
        e.to_string()
    })?;
    info!("scrcpy server started");

    let device_meta = client.read_device_metadata().map_err(|e| {
        error!(%e, "failed to read device metadata");
        e.to_string()
    })?;
    info!(
        device_name = %device_meta.name,
        "device metadata received"
    );

    let codec_meta = client.read_video_codec_metadata().map_err(|e| {
        error!(%e, "failed to read video codec metadata");
        e.to_string()
    })?;
    info!(
        codec = ?codec_meta.codec,
        width = codec_meta.width,
        height = codec_meta.height,
        "video codec metadata received"
    );

    // ── Audio metadata ──
    let audio_meta = client.read_audio_codec_metadata();
    match &audio_meta {
        Ok(m) => info!(
            codec = ?m.codec,
            "audio codec metadata received"
        ),
        Err(e) => warn!(%e, "audio metadata unavailable (device may not support audio capture)"),
    }

    channel
        .send(PacketEvent::Meta {
            device_name: device_meta.name,
            codec: codec_to_str(codec_meta.codec).to_string(),
            width: codec_meta.width,
            height: codec_meta.height,
        })
        .map_err(|e| {
            error!(%e, "failed to send meta event to frontend");
            e.to_string()
        })?;

    let mut video_stream = client.video_stream().map_err(|e| {
        error!(%e, "failed to get video stream");
        e.to_string()
    })?;
    info!("video stream obtained");

    let audio_stream = client.audio_stream().ok();
    match &audio_stream {
        Some(_) => info!("audio stream obtained"),
        None => warn!("audio stream unavailable — no audio will be sent"),
    }

    let stop = Arc::new(AtomicBool::new(false));

    // ── Video reader thread ──
    let stop_video = stop.clone();
    let channel_video = channel.clone();
    let join_video = std::thread::Builder::new()
        .name("sadb-video-reader".into())
        .spawn(move || {
            info!("video reader thread started");
            let mut packet_count: u64 = 0;
            loop {
                if stop_video.load(Ordering::Relaxed) {
                    info!(packets = packet_count, "video reader stopping");
                    break;
                }
                match video_stream.read_packet() {
                    Ok(Some(pkt)) => {
                        packet_count += 1;
                        if packet_count % 300 == 0 {
                            debug!(packets = packet_count, pts = pkt.header.pts, key_frame = pkt.header.key_frame, "video packet");
                        }
                        let evt = PacketEvent::Packet {
                            pts: pkt.header.pts,
                            key_frame: pkt.header.key_frame,
                            config: pkt.header.config_packet,
                            data: B64.encode(&pkt.data),
                        };
                        if let Err(e) = channel_video.send(evt) {
                            error!(%e, "video channel send failed");
                            break;
                        }
                    }
                    Ok(None) => continue,
                    Err(e) => {
                        error!(%e, "video read_packet error");
                        let _ = channel_video.send(PacketEvent::Error {
                            message: e.to_string(),
                        });
                        break;
                    }
                }
            }
            let _ = channel_video.send(PacketEvent::Closed);
            info!("video reader thread finished");
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
                    info!("audio reader thread started");
                    let mut packet_count: u64 = 0;
                    loop {
                        if stop_audio.load(Ordering::Relaxed) {
                            info!(packets = packet_count, "audio reader stopping");
                            break;
                        }
                        match audio_stream.read_packet() {
                            Ok(Some(pkt)) => {
                                packet_count += 1;
                                if packet_count <= 5 || packet_count % 200 == 0 {
                                    debug!(
                                        packets = packet_count,
                                        pts = pkt.header.pts,
                                        config = pkt.header.config_packet,
                                        size = pkt.data.len(),
                                        "audio packet"
                                    );
                                }
                                let evt = PacketEvent::AudioPacket {
                                    pts: pkt.header.pts,
                                    config: pkt.header.config_packet,
                                    data: B64.encode(&pkt.data),
                                };
                                if let Err(e) = channel_audio.send(evt) {
                                    error!(%e, "audio channel send failed");
                                    break;
                                }
                            }
                            Ok(None) => {
                                debug!("audio read_packet returned None, continuing");
                                continue;
                            }
                            Err(e) => {
                                error!(%e, "audio read_packet error");
                                break;
                            }
                        }
                    }
                    info!("audio reader thread finished");
                })
                .expect("failed to spawn audio reader thread"),
        )
    } else {
        warn!("no audio stream available, skipping audio reader thread");
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
                    info!("control reader thread started");
                    let mut reader = BufReader::with_capacity(64 * 1024, ctrl);
                    let mut buf = Vec::new();
                    loop {
                        if stop_ctrl.load(Ordering::Relaxed) {
                            info!("control reader stopping");
                            break;
                        }
                        let mut temp = [0u8; 4096];
                        match reader.read(&mut temp) {
                            Ok(0) => {
                                debug!("control socket closed (read 0)");
                                break;
                            }
                            Ok(n) => {
                                debug!(bytes = n, "control data received");
                                buf.extend_from_slice(&temp[..n]);
                                loop {
                                    match DeviceMessage::deserialize(&buf) {
                                        Ok(Some((msg, consumed))) => {
                                            buf.drain(..consumed);
                                            if let DeviceMessage::Clipboard { text } = msg {
                                                debug!(text_len = text.len(), "clipboard message from device");
                                                let _ = channel_ctrl.send(PacketEvent::Clipboard { text });
                                            }
                                        }
                                        Ok(None) => break,
                                        Err(e) => {
                                            warn!(%e, "device msg parse error, clearing buffer");
                                            buf.clear();
                                            break;
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                error!(%e, "control read error");
                                break;
                            }
                        }
                    }
                    info!("control reader thread finished");
                })
                .expect("failed to spawn control reader thread"),
        )
    } else {
        debug!("no control socket available");
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
    info!("mirroring session fully established");
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
                error!(%e, "send touch event failed");
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
        debug!(action = action, keycode = keycode, metastate = metastate, "send keycode");
        session
            .client
            .send_control_msg(&msg.serialize())
            .map_err(|e| {
                error!(%e, "send keycode failed");
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
        debug!(text_len = text.len(), "inject text");
        let msg = InjectTextEvent { text };
        session
            .client
            .send_control_msg(&msg.serialize())
            .map_err(|e| {
                error!(%e, "inject text failed");
                e.to_string()
            })?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn sadb_connect_device(serial: String) -> Result<(), String> {
    info!(%serial, "connecting to device via ADB");
    sadb_core::adb::AdbClient::connect(&serial)
        .await
        .map_err(|e| {
            error!(%serial, %e, "ADB connect failed");
            e.to_string()
        })?;
    info!(%serial, "ADB connect succeeded");
    Ok(())
}

#[tauri::command]
pub(crate) async fn sadb_disconnect_device(serial: String) -> Result<(), String> {
    info!(%serial, "disconnecting device");
    sadb_core::adb::AdbClient::disconnect(&serial)
        .await
        .map_err(|e| {
            warn!(%serial, %e, "ADB disconnect failed");
            e.to_string()
        })?;
    info!(%serial, "ADB disconnect succeeded");
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
        debug!(text_len = text.len(), paste, "set clipboard");
        let msg = SetClipboard { text, paste };
        session
            .client
            .send_control_msg(&msg.serialize())
            .map_err(|e| {
                error!(%e, "set clipboard failed");
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
                error!(%e, "send scroll event failed");
                e.to_string()
            })?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn sadb_stop_mirroring(state: State<'_, IslandState>) -> Result<(), String> {
    state.sadb_mirroring.store(false, Ordering::Relaxed);
    info!("stopping mirroring session");
    let session = state.sadb_session.lock().await.take();
    if let Some(mut session) = session {
        session.stop.store(true, Ordering::Relaxed);

        if let Err(e) = session.client.cleanup().await {
            warn!(%e, "cleanup error");
        }

        if let Some(join) = session.join_video.take() {
            let _ = tokio::task::spawn_blocking(move || {
                let _ = join.join();
            })
            .await;
            info!("video thread joined");
        }
        if let Some(join) = session.join_audio.take() {
            let _ = tokio::task::spawn_blocking(move || {
                let _ = join.join();
            })
            .await;
            info!("audio thread joined");
        }
        if let Some(join) = session.join_control.take() {
            let _ = tokio::task::spawn_blocking(move || {
                let _ = join.join();
            })
            .await;
            info!("control thread joined");
        }

        info!("mirroring session stopped");
    } else {
        debug!("stop_mirroring called but no active session");
    }
    Ok(())
}
