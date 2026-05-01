//! sadb-core: Scrcpy-like Android screen mirroring protocol implementation in Rust
//!
//! This library provides:
//! - ADB client wrapper (push/reverse/shell)
//! - Scrcpy protocol parsing (device meta + codec meta + 12-byte frame headers)
//! - Server lifecycle management
//! - Video stream reader returning H.264 packets
//!
//! Typical usage:
//! ```no_run
//! use sadb_core::{ScrcpyClient, Config};
//! use tokio::fs::File;
//! use tokio::io::AsyncWriteExt;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let client = ScrcpyClient::new(Config::default()).await?;
//!     let mut stream = client.video_stream().await?;
//!     let mut out = File::create("out.h264").await?;
//!
//!     while let Some(packet) = stream.next().await? {
//!         out.write_all(&packet.data).await?;
//!     }
//!     Ok(())
//! }
//! ```

pub mod adb;
pub mod client;
pub mod config;
pub mod control;
pub mod error;
pub mod protocol;
pub mod stream;

pub use client::ScrcpyClient;
pub use config::Config;
pub use control::{CopyKey, DeviceMessage, GetClipboard};
pub use error::{Error, Result};
