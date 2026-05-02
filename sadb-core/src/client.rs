//! Main scrcpy client implementation
//!
//! High-level interface for connecting to an Android device and streaming
//! video/audio using the scrcpy protocol.

use crate::adb::AdbClient;
use crate::config::Config;
use crate::control::{CopyKey, GetClipboard};
use crate::error::{Error, Result};
use crate::protocol::{AudioCodecMetadata, DeviceMetadata, VideoCodecMetadata};
use crate::stream::SyncPacketStream;
use std::io::{BufReader, Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::TcpListener;
use tokio::process::{Child, Command as TokioCommand};
use tracing::{debug, info, warn};

/// Scrcpy-server version that must match the jar we push to the device.
/// When upgrading the jar, bump this too.
pub const SCRCPY_SERVER_VERSION: &str = "3.3.4";

/// Default local TCP port used for the reverse/forward tunnel to the video socket.
const DEFAULT_VIDEO_PORT: u16 = 27183;

/// Main scrcpy client. Each instance manages a single device session.
pub struct ScrcpyClient {
    config: Config,
    adb: Arc<AdbClient>,
    scid: u32,
    server_child: Option<Child>,
    video_socket: Option<TcpStream>,
    audio_socket: Option<TcpStream>,
    /// Control socket shared with `send_control_msg`; `None` when control is disabled.
    control_socket: Option<Arc<Mutex<TcpStream>>>,
    tunnel_name: Option<String>,
}

impl ScrcpyClient {
    /// Create a new client. Does not connect yet; call [`start`] to launch the
    /// server and open sockets.
    pub async fn new(config: Config) -> Result<Self> {
        let adb = Arc::new(AdbClient::new(config.serial.clone()));

        if !adb.is_connected().await? {
            return Err(Error::Adb("No device connected".to_string()));
        }

        let scid = generate_scid();

        Ok(Self {
            config,
            adb,
            scid,
            server_child: None,
            video_socket: None,
            audio_socket: None,
            control_socket: None,
            tunnel_name: None,
        })
    }

    /// Session client ID (random 31-bit number, useful for logs).
    pub fn scid(&self) -> u32 {
        self.scid
    }

    /// Push the scrcpy-server jar, set up a reverse tunnel, spawn the server
    /// process, and accept the video socket.
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting scrcpy server (scid={:08x})...", self.scid);

        // 1. Push the jar to the device
        self.adb
            .push(
                &self.config.server_jar_path,
                "/data/local/tmp/scrcpy-server.jar",
            )
            .await?;

        // 2. Bind local listener FIRST (reverse mode: device -> us)
        let tunnel = format!("localabstract:scrcpy_{:08x}", self.scid);
        let listener = if self.config.force_forward {
            // In forward mode we connect later; no listener needed.
            None
        } else {
            let l = TcpListener::bind(("127.0.0.1", DEFAULT_VIDEO_PORT)).await?;
            debug!("Listening on 127.0.0.1:{}", DEFAULT_VIDEO_PORT);
            Some(l)
        };

        // 3. Setup ADB tunnel
        if self.config.force_forward {
            self.adb
                .forward(&format!("tcp:{}", DEFAULT_VIDEO_PORT), &tunnel)
                .await?;
        } else {
            self.adb
                .reverse(&tunnel, &format!("tcp:{}", DEFAULT_VIDEO_PORT))
                .await?;
        }
        self.tunnel_name = Some(tunnel.clone());

        // 4. Spawn server process
        let server_args = self.build_server_args();
        debug!("Server args: {}", server_args.join(" "));

        let mut cmd = TokioCommand::new("adb");
        if let Some(ref serial) = self.config.serial {
            cmd.arg("-s").arg(serial);
        }
        cmd.arg("shell").args(&server_args);
        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .stdin(std::process::Stdio::null())
            .kill_on_drop(true);

        let child = cmd.spawn().map_err(|e| {
            Error::Adb(format!("Failed to spawn adb shell for server: {}", e))
        })?;
        self.server_child = Some(child);

        // 5. Accept or connect video socket (and control socket if enabled)
        if let Some(ref listener) = listener {
            // Reverse tunnel: device connects to us
            let (video_tok, _) = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                listener.accept(),
            )
            .await
            .map_err(|_| Error::Adb("Timed out waiting for video socket".to_string()))??;
            let video = video_tok.into_std()?;
            video.set_nonblocking(false)?;
            debug!("Accepted video socket");
            self.video_socket = Some(video);

            if self.config.audio {
                let (audio_tok, _) = tokio::time::timeout(
                    std::time::Duration::from_secs(10),
                    listener.accept(),
                )
                .await
                .map_err(|_| Error::Adb("Timed out waiting for audio socket".to_string()))??;
                let audio = audio_tok.into_std()?;
                audio.set_nonblocking(false)?;
                debug!("Accepted audio socket");
                self.audio_socket = Some(audio);
            }

            if self.config.control {
                let (ctrl_tok, _) = tokio::time::timeout(
                    std::time::Duration::from_secs(10),
                    listener.accept(),
                )
                .await
                .map_err(|_| Error::Adb("Timed out waiting for control socket".to_string()))??;
                let ctrl = ctrl_tok.into_std()?;
                ctrl.set_nonblocking(false)?;
                debug!("Accepted control socket");
                self.control_socket = Some(Arc::new(Mutex::new(ctrl)));
            }
        } else {
            // Forward tunnel: we connect to the local forwarded port
            std::thread::sleep(std::time::Duration::from_millis(500));
            let video = TcpStream::connect(("127.0.0.1", DEFAULT_VIDEO_PORT))?;
            video.set_nonblocking(false)?;
            self.video_socket = Some(video);
            // control socket not supported in forward mode yet
        }

        info!("Scrcpy server connected");
        Ok(())
    }

    /// Read the 64-byte device metadata header from the video socket.
    /// Must be called right after [`start`] and before reading codec metadata.
    pub fn read_device_metadata(&mut self) -> Result<DeviceMetadata> {
        let socket = self
            .video_socket
            .as_mut()
            .ok_or(Error::ServerNotStarted)?;

        let mut buf = [0u8; 64];
        socket.read_exact(&mut buf)?;

        // Name is null-padded UTF-8
        let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        let name = String::from_utf8_lossy(&buf[..end]).to_string();
        Ok(DeviceMetadata { name })
    }

    /// Read the 12-byte video codec metadata (codec id + width + height).
    pub fn read_video_codec_metadata(&mut self) -> Result<VideoCodecMetadata> {
        let socket = self
            .video_socket
            .as_mut()
            .ok_or(Error::ServerNotStarted)?;

        let mut buf = [0u8; 12];
        socket.read_exact(&mut buf)?;
        VideoCodecMetadata::parse(&buf)
    }

    /// Read the 4-byte audio codec metadata (codec id only).
    pub fn read_audio_codec_metadata(&mut self) -> Result<AudioCodecMetadata> {
        let socket = self
            .audio_socket
            .as_mut()
            .ok_or(Error::ServerNotStarted)?;

        let mut buf = [0u8; 4];
        socket.read_exact(&mut buf)?;
        AudioCodecMetadata::parse(&buf)
    }

    /// Take ownership of the audio socket as a packet stream.
    pub fn audio_stream(&mut self) -> Result<SyncPacketStream<BufReader<TcpStream>>> {
        let socket = self.audio_socket.take().ok_or(Error::ServerNotStarted)?;
        Ok(SyncPacketStream::new(BufReader::with_capacity(
            64 * 1024,
            socket,
        )))
    }

    /// Write raw bytes to the control socket.
    /// Returns `Err(ServerNotStarted)` if control was not enabled.
    pub fn send_control_msg(&self, data: &[u8]) -> Result<()> {
        let sock = self
            .control_socket
            .as_ref()
            .ok_or(Error::ServerNotStarted)?;
        let mut guard = sock
            .lock()
            .map_err(|_| Error::Adb("control socket mutex poisoned".to_string()))?;
        guard.write_all(data).map_err(Error::from)
    }

    /// Clone the control socket for use in a reader thread.
    /// Returns `Err(ServerNotStarted)` if control was not enabled.
    pub fn control_socket_clone(&self) -> Result<std::net::TcpStream> {
        let sock = self
            .control_socket
            .as_ref()
            .ok_or(Error::ServerNotStarted)?;
        let guard = sock
            .lock()
            .map_err(|_| Error::Adb("control socket mutex poisoned".to_string()))?;
        guard.try_clone().map_err(Error::from)
    }

    /// Send a GET_CLIPBOARD request to the device.
    pub fn send_get_clipboard(&self, copy_key: CopyKey) -> Result<()> {
        let msg = GetClipboard { copy_key };
        self.send_control_msg(&msg.serialize())
    }

    /// Take ownership of the video socket as a packet stream.
    pub fn video_stream(&mut self) -> Result<SyncPacketStream<BufReader<TcpStream>>> {
        let socket = self.video_socket.take().ok_or(Error::ServerNotStarted)?;
        Ok(SyncPacketStream::new(BufReader::with_capacity(
            1024 * 1024,
            socket,
        )))
    }

    /// Build the list of arguments passed to `adb shell` to start the server.
    fn build_server_args(&self) -> Vec<String> {
        let mut args: Vec<String> = Vec::new();
        args.push("CLASSPATH=/data/local/tmp/scrcpy-server.jar".into());
        args.push("app_process".into());
        args.push("/".into());
        args.push("com.genymobile.scrcpy.Server".into());
        args.push(SCRCPY_SERVER_VERSION.into());

        args.push(format!("scid={:08x}", self.scid));
        args.push(format!("log_level={}", self.config.log_level));
        args.push(format!("audio={}", self.config.audio));
        args.push(format!("control={}", self.config.control));
        args.push(format!("cleanup=true"));

        if self.config.force_forward {
            args.push("tunnel_forward=true".into());
        }
        if let Some(max_size) = self.config.max_size {
            args.push(format!("max_size={}", max_size));
        }
        if let Some(bitrate) = self.config.video_bitrate {
            args.push(format!("video_bit_rate={}", bitrate));
        }
        args
    }

    /// Kill the server process and remove ADB tunnels.
    pub async fn cleanup(&mut self) -> Result<()> {
        debug!("Cleaning up scrcpy client");

        // Drop sockets first so the server sees EOF and exits naturally
        self.video_socket = None;
        self.audio_socket = None;
        self.control_socket = None;

        // Kill server process
        if let Some(mut child) = self.server_child.take() {
            let _ = child.start_kill();
            let _ = child.wait().await;
        }

        // Remove tunnels
        if let Some(tunnel) = self.tunnel_name.take() {
            if self.config.force_forward {
                let _ = self
                    .adb
                    .forward_remove(&format!("tcp:{}", DEFAULT_VIDEO_PORT))
                    .await;
            } else {
                let _ = self.adb.reverse_remove(&tunnel).await;
            }
        }
        Ok(())
    }
}

impl Drop for ScrcpyClient {
    fn drop(&mut self) {
        if self.server_child.is_some() || self.video_socket.is_some() || self.audio_socket.is_some() || self.control_socket.is_some() {
            warn!("ScrcpyClient dropped without cleanup(); leaking tunnels may occur");
        }
    }
}

/// Generate a 31-bit random session ID based on time.
fn generate_scid() -> u32 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .hash(&mut hasher);
    std::process::id().hash(&mut hasher);
    (hasher.finish() & 0x7fff_ffff) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scid_is_31_bit() {
        for _ in 0..100 {
            assert!(generate_scid() <= 0x7fff_ffff);
        }
    }
}
