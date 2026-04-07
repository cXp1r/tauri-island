use serde::{Deserialize, Serialize};

/// 附加文件信息枚举
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AdditionalFileInfo {
    General(GeneralAdditionalInfo),
    Krc(KrcAdditionalInfo),
    Spotify(SpotifyAdditionalInfo),
}

/// 通用附加信息
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GeneralAdditionalInfo {
    pub attributes: Vec<(String, String)>,
}

/// KRC 附加信息
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KrcAdditionalInfo {
    pub hash: Option<String>,
    pub attributes: Vec<(String, String)>,
}

/// Spotify 附加信息
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SpotifyAdditionalInfo {
    pub provider: Option<String>,
    pub provider_lyrics_id: Option<String>,
    pub provider_display_name: Option<String>,
}
