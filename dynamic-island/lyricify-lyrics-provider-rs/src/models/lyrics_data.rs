use serde::{Deserialize, Serialize};
use super::{LineInfo, LyricsFileInfo, TrackMetadata};

/// 歌词数据
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LyricsData {
    pub file: Option<LyricsFileInfo>,
    pub lines: Vec<LineInfo>,
    pub track_metadata: Option<TrackMetadata>,
}
