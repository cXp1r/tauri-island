use serde::{Deserialize, Serialize};

/// 歌词同步类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum SyncTypes {
    /// 逐字同步
    SyllableSynced,
    /// 逐行同步
    LineSynced,
    /// 非同步
    #[default]
    Unsynced,
}



