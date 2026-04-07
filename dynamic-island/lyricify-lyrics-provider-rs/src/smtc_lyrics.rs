//! SMTC 歌词获取管线
//!
//! 调用方提供歌曲名、歌手名、专辑名等信息，
//! 库内部自动检测运行中的音乐播放器进程，按首字母排序取第一个，
//! 用该播放器自家的源搜索并获取歌词。

use crate::models::{TrackMetadata, LyricsData};
use crate::searchers::ISearcher;
use crate::searchers::netease::NeteaseSearchResult;
use crate::searchers::qqmusic::QQMusicSearchResult;
use crate::searchers::kugou::KugouSearchResult;
use crate::searchers::soda_music::SodaMusicSearchResult;

// ===== 播放器定义 =====

/// 音乐播放器类型 (枚举值已按首字母排序: K, N, Q, S)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MusicPlayer {
    /// 酷狗音乐
    Kugou,
    /// 网易云音乐
    Netease,
    /// QQ音乐
    QQMusic,
    /// 汽水音乐
    SodaMusic,
}

impl MusicPlayer {
    /// 播放器进程名
    pub fn process_name(&self) -> &str {
        match self {
            MusicPlayer::Kugou => "KGMusic.exe",
            MusicPlayer::Netease => "cloudmusic.exe",
            MusicPlayer::QQMusic => "QQMusic.exe",
            MusicPlayer::SodaMusic => "SodaMusic.exe",
        }
    }

    /// 播放器显示名称
    pub fn display_name(&self) -> &str {
        match self {
            MusicPlayer::Kugou => "酷狗音乐",
            MusicPlayer::Netease => "网易云音乐",
            MusicPlayer::QQMusic => "QQ音乐",
            MusicPlayer::SodaMusic => "汽水音乐",
        }
    }

    /// 所有支持的播放器 (已按首字母排序)
    pub fn all_sorted() -> &'static [MusicPlayer] {
        &[
            MusicPlayer::Kugou,
            MusicPlayer::Netease,
            MusicPlayer::QQMusic,
            MusicPlayer::SodaMusic,
        ]
    }
}

// ===== 进程检测 (Windows) =====

/// 检测指定进程名是否正在运行
#[cfg(target_os = "windows")]
fn is_process_running(process_name: &str) -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let output = std::process::Command::new("tasklist")
        .args(["/FI", &format!("IMAGENAME eq {}", process_name), "/NH"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    match output {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            stdout.contains(process_name)
        }
        Err(_) => false,
    }
}

#[cfg(not(target_os = "windows"))]
fn is_process_running(_process_name: &str) -> bool {
    false
}

/// 获取当前正在运行的音乐播放器 (按首字母排序)
pub fn get_running_players() -> Vec<MusicPlayer> {
    MusicPlayer::all_sorted()
        .iter()
        .filter(|p| is_process_running(p.process_name()))
        .copied()
        .collect()
}

/// 获取排序后的第一个正在运行的音乐播放器
pub fn get_first_running_player() -> Option<MusicPlayer> {
    MusicPlayer::all_sorted()
        .iter()
        .find(|p| is_process_running(p.process_name()))
        .copied()
}

// ===== 公开接口 =====

/// 获取歌词
///
/// 调用方提供歌曲信息，库自动检测正在运行的播放器进程，
/// 按首字母排序取第一个，用该播放器自家的源搜索并返回歌词。
///
/// # 参数
/// - `title` — 歌曲名 (必填)
/// - `artist` — 歌手名 (可选，推荐提供)
/// - `album` — 专辑名 (可选)
/// - `duration_ms` — 时长毫秒 (可选)
///
/// # 返回
/// - `Ok((MusicPlayer, LyricsData))` — 使用的播放器源 + 歌词数据
/// - `Err(...)` — 未检测到播放器，或获取歌词失败
pub async fn get_lyrics(
    title: &str,
    artist: Option<&str>,
    album: Option<&str>,
    duration_ms: Option<i32>,
) -> Result<(MusicPlayer, LyricsData), Box<dyn std::error::Error + Send + Sync>> {
    let player = get_first_running_player()
        .ok_or("未检测到正在运行的音乐播放器")?;

    let metadata = TrackMetadata {
        title: Some(title.to_string()),
        artist: artist.map(|s| s.to_string()),
        album: album.map(|s| s.to_string()),
        duration_ms,
        ..Default::default()
    };

    let lyrics = fetch_lyrics_from_player(&player, &metadata).await?;
    Ok((player, lyrics))
}

/// 指定播放器源获取歌词
///
/// 当调用方已知要使用哪个播放器源时，可直接指定，跳过进程检测。
pub async fn get_lyrics_with_player(
    player: &MusicPlayer,
    title: &str,
    artist: Option<&str>,
    album: Option<&str>,
    duration_ms: Option<i32>,
) -> Result<LyricsData, Box<dyn std::error::Error + Send + Sync>> {
    let metadata = TrackMetadata {
        title: Some(title.to_string()),
        artist: artist.map(|s| s.to_string()),
        album: album.map(|s| s.to_string()),
        duration_ms,
        ..Default::default()
    };

    fetch_lyrics_from_player(player, &metadata).await
}

// ===== 内部: 按播放器分发 =====

async fn fetch_lyrics_from_player(
    player: &MusicPlayer,
    track: &TrackMetadata,
) -> Result<LyricsData, Box<dyn std::error::Error + Send + Sync>> {
    match player {
        MusicPlayer::Netease => fetch_netease_lyrics(track).await,
        MusicPlayer::QQMusic => fetch_qqmusic_lyrics(track).await,
        MusicPlayer::Kugou => fetch_kugou_lyrics(track).await,
        MusicPlayer::SodaMusic => fetch_soda_music_lyrics(track).await,
    }
}

// ===== 各播放器歌词获取实现 =====

async fn fetch_netease_lyrics(
    track: &TrackMetadata,
) -> Result<LyricsData, Box<dyn std::error::Error + Send + Sync>> {
    use crate::searchers::netease::NeteaseSearcher;
    use crate::providers::netease::NeteaseApi;

    let searcher = NeteaseSearcher::new();
    let best = searcher.search_for_result(track).await?
        .ok_or("网易云: 未找到匹配的歌曲")?;

    let id = best.as_any()
        .downcast_ref::<NeteaseSearchResult>()
        .ok_or("网易云: 搜索结果类型不匹配")?
        .id.clone();

    let api = NeteaseApi::new();
    let lyric_result = api.get_lyric(&id).await?;

    // 优先 YRC (逐字), 其次 LRC (逐行)
    if let Some(yrc_text) = lyric_result.yrc.and_then(|y| y.lyric) {
        if !yrc_text.is_empty() {
            return Ok(crate::parsers::yrc::parse(&yrc_text));
        }
    }
    if let Some(lrc_text) = lyric_result.lrc.and_then(|l| l.lyric) {
        if !lrc_text.is_empty() {
            return Ok(crate::parsers::lrc::parse(&lrc_text));
        }
    }

    Err("网易云: 未获取到歌词内容".into())
}

async fn fetch_qqmusic_lyrics(
    track: &TrackMetadata,
) -> Result<LyricsData, Box<dyn std::error::Error + Send + Sync>> {
    use crate::searchers::qqmusic::QQMusicSearcher;
    use crate::providers::qqmusic::QQMusicApi;

    let searcher = QQMusicSearcher::new();
    let best = searcher.search_for_result(track).await?
        .ok_or("QQ音乐: 未找到匹配的歌曲")?;

    let mid = best.as_any()
        .downcast_ref::<QQMusicSearchResult>()
        .ok_or("QQ音乐: 搜索结果类型不匹配")?
        .mid.clone();

    let api = QQMusicApi::new();
    let lyric_result = api.get_lyric(&mid).await?
        .ok_or("QQ音乐: 获取歌词失败")?;

    if let Some(lrc_text) = lyric_result.lyric {
        if !lrc_text.is_empty() {
            return Ok(crate::parsers::lrc::parse(&lrc_text));
        }
    }

    Err("QQ音乐: 未获取到歌词内容".into())
}

async fn fetch_kugou_lyrics(
    track: &TrackMetadata,
) -> Result<LyricsData, Box<dyn std::error::Error + Send + Sync>> {
    use crate::searchers::kugou::KugouSearcher;
    use crate::providers::kugou::KugouApi;

    let searcher = KugouSearcher::new();
    let best = searcher.search_for_result(track).await?
        .ok_or("酷狗: 未找到匹配的歌曲")?;

    let result = best.as_any()
        .downcast_ref::<KugouSearchResult>()
        .ok_or("酷狗: 搜索结果类型不匹配")?;

    let api = KugouApi::new();
    let keyword = format!("{} {}", result.title, result.artists.join(", "));
    let lyrics_resp = api.get_search_lyrics(
        Some(&keyword),
        result.duration_ms,
        Some(&result.hash),
    ).await?
        .ok_or("酷狗: 获取歌词候选失败")?;

    // TODO: 酷狗歌词需要通过 candidate 的 id + access_key 下载实际歌词内容
    // 当前 KugouApi 尚未实现歌词下载接口
    let _candidates = lyrics_resp.candidates.unwrap_or_default();

    Err("酷狗: 歌词下载接口尚未实现".into())
}

async fn fetch_soda_music_lyrics(
    track: &TrackMetadata,
) -> Result<LyricsData, Box<dyn std::error::Error + Send + Sync>> {
    use crate::searchers::soda_music::SodaMusicSearcher;
    use crate::providers::soda_music::SodaMusicApi;

    let searcher = SodaMusicSearcher::new();
    let best = searcher.search_for_result(track).await?
        .ok_or("汽水音乐: 未找到匹配的歌曲")?;

    let id = best.as_any()
        .downcast_ref::<SodaMusicSearchResult>()
        .ok_or("汽水音乐: 搜索结果类型不匹配")?
        .id.clone();

    let api = SodaMusicApi::new();
    let detail = api.get_detail(&id).await?
        .ok_or("汽水音乐: 获取歌曲详情失败")?;

    if let Some(lyric_info) = detail.lyric {
        if let Some(content) = lyric_info.content {
            if !content.is_empty() {
                return Ok(parse_soda_lyric(&content));
            }
        }
    }

    Err("汽水音乐: 未获取到歌词内容".into())
}

/// 解析汽水音乐歌词：支持 KRC 毫秒格式 [start_ms,duration_ms]<offset,dur,0>text
/// 和标准 LRC [mm:ss.xx] 格式，自动检测
fn parse_soda_lyric(content: &str) -> crate::models::LyricsData {
    use crate::models::{LyricsData, LineInfo, BasicLineInfo};

    // 检测是否为 KRC 格式：行以 [数字,数字] 开头
    let is_krc = content.lines().any(|l| {
        let l = l.trim();
        l.starts_with('[') && {
            let inner = &l[1..];
            let comma = inner.find(',');
            let close = inner.find(']');
            match (comma, close) {
                (Some(c), Some(b)) if c < b => {
                    inner[..c].chars().all(|ch| ch.is_ascii_digit()) &&
                    inner[c+1..b].chars().all(|ch| ch.is_ascii_digit())
                }
                _ => false,
            }
        }
    });

    if !is_krc {
        return crate::parsers::lrc::parse(content);
    }

    // KRC 解析：每行 [start_ms,dur_ms]<offset,dur,0>字...
    let word_tag_re = regex::Regex::new(r"<\d+,\d+,\d+>").unwrap();
    let mut lines: Vec<LineInfo> = Vec::new();

    for raw in content.lines() {
        let raw = raw.trim();
        if raw.is_empty() { continue; }

        // 提取所有 [start_ms,dur_ms] 前缀（一行可能有多个时间戳）
        let mut pos = 0;
        let bytes = raw.as_bytes();
        while pos < bytes.len() && bytes[pos] == b'[' {
            // 找到闭括号
            let close = match raw[pos..].find(']') {
                Some(i) => pos + i,
                None => break,
            };
            let inner = &raw[pos+1..close];
            // 解析 start_ms,duration_ms
            if let Some(comma) = inner.find(',') {
                let start_part = &inner[..comma];
                let dur_part = &inner[comma+1..];
                if start_part.chars().all(|c| c.is_ascii_digit())
                    && dur_part.chars().all(|c| c.is_ascii_digit())
                {
                    if let Ok(start_ms) = start_part.parse::<i32>() {
                        // 歌词文本从闭括号后开始，去掉字级标签
                        let text_raw = &raw[close+1..];
                        let text = word_tag_re.replace_all(text_raw, "").to_string();
                        let text = text.trim().to_string();
                        if !text.is_empty() {
                            lines.push(LineInfo::Basic(BasicLineInfo {
                                text,
                                start_time: Some(start_ms),
                                end_time: None,
                                ..Default::default()
                            }));
                        }
                        break; // 一行只取第一个时间戳
                    }
                }
            }
            pos = close + 1;
        }
    }

    let mut data = LyricsData::default();
    data.lines = lines;
    data
}
