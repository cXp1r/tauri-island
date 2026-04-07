use serde::{Deserialize, Serialize};

/// 歌词类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LyricsTypes {
    Unknown,
    LyricifySyllable,
    LyricifyLines,
    Lrc,
    Qrc,
    Krc,
    Yrc,
    Ttml,
    Spotify,
    Musixmatch,
}

impl Default for LyricsTypes {
    fn default() -> Self {
        LyricsTypes::Unknown
    }
}

/// 歌词原始字符串类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LyricsRawTypes {
    Unknown,
    LyricifySyllable,
    LyricifyLines,
    Lrc,
    Qrc,
    QrcFull,
    Krc,
    Yrc,
    YrcFull,
    Ttml,
    AppleJson,
    Spotify,
    Musixmatch,
}

impl Default for LyricsRawTypes {
    fn default() -> Self {
        LyricsRawTypes::Unknown
    }
}
