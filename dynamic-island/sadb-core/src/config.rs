use serde::{Deserialize, Serialize};

/// Configuration for scrcpy connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Path to the scrcpy-server jar file
    pub server_jar_path: String,
    
    /// Maximum video size (width or height)
    pub max_size: Option<u32>,
    
    /// Video bitrate in bps
    pub video_bitrate: Option<u32>,
    
    /// Enable audio stream
    pub audio: bool,
    
    /// Enable control stream
    pub control: bool,
    
    /// Server log level
    pub log_level: String,
    
    /// Force ADB forward instead of reverse
    pub force_forward: bool,
    
    /// Device serial (if multiple devices)
    pub serial: Option<String>,

    /// Path to adb executable
    pub adb_path: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_jar_path: "assets/scrcpy-server-v3.3.4".to_string(),
            max_size: Some(1920),
            video_bitrate: Some(8_000_000), // 8 Mbps
            audio: false,
            control: true,
            log_level: "info".to_string(),
            force_forward: false,
            serial: None,
            adb_path: None,
        }
    }
}
