use serde::{Deserialize, Serialize};
use super::{LyricsTypes, SyncTypes, AdditionalFileInfo};

/// 歌词文件信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricsFileInfo {
    pub lyrics_type: LyricsTypes,
    pub sync_type: SyncTypes,
    pub additional_info: Option<AdditionalFileInfo>,
}

impl Default for LyricsFileInfo {
    fn default() -> Self {
        Self {
            lyrics_type: LyricsTypes::Unknown,
            sync_type: SyncTypes::Unknown,
            additional_info: None,
        }
    }
}
