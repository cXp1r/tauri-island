use crate::models::{
    TrackMetadata,
    LyricsData
};
use crate::searchers::{
    ISearcher,
    netease::NeteaseSearchResult,
    qqmusic::QQMusicSearchResult,
    kugou::KugouSearchResult,
    soda_music::SodaMusicSearchResult
};
use crate::parsers::{
    IParsers,
    lrc::LrcParser,
    netease::{
        NeteaseParser,
        NeteaseLrcParser
    },
    qqmusic::{
        QQMusicParser,
        QQMusicLrcParser
    },
    kugou::KugouParser,
    soda_music::SodaParser,
};


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
            MusicPlayer::Kugou => "KuGou.exe",
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


// ===== 公开接口 =====

/// 获取歌词
/// # 参数
/// - `title` — 歌曲名 (必填)
/// - `artist` — 歌手名 (可选，推荐提供)
/// - `album` — 专辑名 (可选)
/// - `album_artist` — 专辑艺术家 (可选)
/// - `duration_ms` — 时长毫秒 (可选)
///
/// # 返回
/// - `Ok((MusicPlayer, LyricsData))` — 使用的播放器源 + 歌词数据
/// - `Err(...)` — 未检测到播放器，或获取歌词失败
pub async fn get_lyrics_with_player(
    player: &MusicPlayer,
    title: &str,
    artist: Option<&str>,
    album: Option<&str>,
    album_artist: Option<&str>,
    duration_ms: u32,
) -> Result<LyricsData, Box<dyn std::error::Error + Send + Sync>> {
    let metadata = TrackMetadata {
        title: Some(title.to_string()),
        artist: artist.map(|s| s.to_string()),
        album: album.map(|s| s.to_string()),
        album_artist: album_artist.map(|s| s.to_string()),
        duration_ms: Some(duration_ms),
        ..Default::default()
    };

    fetch_lyrics_from_player(player, &metadata).await
}

pub async fn get_lyrics_with_appid(
    app_id: &str,
    title: &str,
    artist: Option<&str>,
    album: Option<&str>,
    album_artist: Option<&str>,
    duration_ms: u32,
) -> Result<LyricsData, Box<dyn std::error::Error + Send + Sync>> {
    let player = match app_id {
        "cloudmusic.exe" => MusicPlayer::Netease,
        "qqmusic.exe" => MusicPlayer::QQMusic,
        "kugou" => MusicPlayer::Kugou,
        "\u{6c7d}\u{6c34}\u{97f3}\u{4e50}" => MusicPlayer::SodaMusic,
        _ => {
            return Err(format!("Unsupported appid: {}", app_id).into());
        }
    };
    let metadata = TrackMetadata {
        title: Some(title.to_string()),
        artist: artist.map(|s| s.to_string()),
        album: album.map(|s| s.to_string()),
        album_artist: album_artist.map(|s| s.to_string()),
        duration_ms: Some(duration_ms),
        ..Default::default()
    };

    fetch_lyrics_from_player(&player, &metadata).await
}


//分发
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
    let result = searcher.search_for_result(track).await?
        .ok_or("网易云: 未找到匹配的歌曲")?;
    let best = result.as_any()
        .downcast_ref::<NeteaseSearchResult>()
        .ok_or("网易云: 搜索结果类型不匹配")?;
    let id = best.id.clone();

    let api = NeteaseApi::new();
    let lyric_result = api.get_lyric(&id).await?;
    let mut data = LyricsData {
        file: None,
        lines: vec![],
        track_metadata: 
            Some(TrackMetadata {
                title: Some(best.title.clone()),
                artist: Some(best.artists.join(", ")),
                album: Some(best.album.clone()),
                duration_ms: best.duration_ms,
                ..Default::default()
            }),
    };
    if let Some(yrc) = lyric_result.yrc.and_then(|y| y.lyric) {
        if !yrc.is_empty() {
            println!("get yrc");
            let parser = NeteaseParser {};
            data.lines = parser.parse(yrc)?;
            return Ok(data);
        }
    }
    let lrc = lyric_result.lrc.ok_or("网易云: LRC也没有哟")?;
    println!("get lrc");
    let parser = NeteaseLrcParser { 
        version: lrc.version.unwrap_or(3) as u8,
    };
    data.lines = parser.parse(lrc.lyric.ok_or("网易云: LRC也没有哟")?)?;
    if !data.lines.is_empty() {
        return Ok(data);
    }
    Err("网易云: 未获取到歌词内容".into())
}


async fn fetch_qqmusic_lyrics(
    track: &TrackMetadata,
) -> Result<LyricsData, Box<dyn std::error::Error + Send + Sync>> {
    use crate::searchers::qqmusic::QQMusicSearcher;
    use crate::providers::qqmusic::QQMusicApi;
    let searcher = QQMusicSearcher::new();
    let result = searcher.search_for_result(track).await?
        .ok_or("QQ音乐: 未找到匹配的歌曲")?;
    let best = result.as_any()
        .downcast_ref::<QQMusicSearchResult>()
        .ok_or("QQ音乐: 搜索结果类型不匹配")?;
    let api = QQMusicApi::new();
    let id = best.id;
    let mut data = LyricsData {
        file: None,
        lines: vec![],
        track_metadata: 
            Some(TrackMetadata {
                title: Some(best.title.clone()),
                artist: Some(best.artists.join(", ")),
                album: Some(best.album.clone()),
                duration_ms: best.duration_ms,
                ..Default::default()
            }),
    };
    if let Ok(qrc) = api.get_lyrics_qrc(&id.to_string()).await {
        let parser = QQMusicParser {};
        data.lines = parser.decrypt_and_parse(qrc)?;
        return Ok(data);
    }
    let mid = best.mid.clone();
    let lyric_result = api.get_lyric(&mid).await?
        .ok_or("QQ音乐: 获取歌词失败")?;
    if let Some(lrc) = lyric_result.lyric {
        if !lrc.is_empty() {
            let parser = QQMusicLrcParser {};
            data.lines = parser.parse(lrc)?;
            return Ok(data);
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
    let result = searcher.search_for_result(track).await?
        .ok_or("酷狗: 未找到匹配的歌曲")?;

    let best = result.as_any()
        .downcast_ref::<KugouSearchResult>()
        .ok_or("酷狗: 搜索结果类型不匹配")?;

    let api = KugouApi::new();
    let keyword = format!("{} {}", best.title, best.artists.join(", "));

    let lyrics_resp = api.get_search_lyrics(
        Some(&keyword),
        Some(&best.hash),
    ).await?
        .ok_or("酷狗: 获取歌词候选失败")?;

    let candidates = lyrics_resp.candidates.unwrap_or_default();
    let candidate = candidates.first().ok_or("酷狗: 无歌词候选")?;

    let id = candidate.id.as_deref().ok_or("酷狗: 候选缺少 id")?;
    let access_key = candidate.access_key.as_deref().ok_or("酷狗: 候选缺少 accesskey")?;

    let dl_resp = api.get_download_krc(id, access_key).await?
        .ok_or("酷狗: 下载 KRC 失败")?;

    let krc = dl_resp.content.ok_or("酷狗: KRC 内容为空")?;
    if krc.is_empty() {
        return Err("酷狗: KRC 内容为空".into());
    }
    let parser = KugouParser {};
    let data = LyricsData {
        file: None,
        lines: parser.decrypt_and_parse(krc)?,
        track_metadata: 
            Some(TrackMetadata {
                title: Some(best.title.clone()),
                artist: Some(best.artists.join(", ")),
                album: Some(best.album.clone()),
                duration_ms: best.duration_ms,
                ..Default::default()
            }),
    };

    if data.lines.is_empty() {
        return Err("酷狗: 解析歌词为空".into());
    }
    Ok(data)
}


async fn fetch_soda_music_lyrics(
    track: &TrackMetadata,
) -> Result<LyricsData, Box<dyn std::error::Error + Send + Sync>> {
    use crate::searchers::soda_music::SodaMusicSearcher;
    use crate::providers::soda_music::SodaMusicApi;

    let searcher = SodaMusicSearcher::new();
    let result = match searcher.search_for_result(track).await {
        Ok(Some(r)) => r,
        Ok(None) => return Err("汽水音乐: 未找到匹配的歌曲".into()),
        Err(e) => return Err(e),
    };

    let best = result
        .as_any()
        .downcast_ref::<SodaMusicSearchResult>()
        .ok_or("汽水音乐: 搜索结果类型不匹配")?;

    let id = best.id.clone();

    let api = SodaMusicApi::new();
    let detail = api.get_detail(&id).await?
        .ok_or("汽水音乐: 获取歌曲详情失败")?;

    if let Some(lyric_info) = detail.lyric {
        if let Some(content) = lyric_info.content {
            if !content.is_empty() {
                let parser = SodaParser {};

                let data = LyricsData {
                    file: None,
                    lines: parser.parse(content)?,
                    track_metadata: 
                        Some(TrackMetadata {
                            title: Some(best.title.clone()),
                            artist: Some(best.artists.join(", ")),
                            album: Some(best.album.clone()),
                            duration_ms: best.duration_ms,
                            ..Default::default()
                        }),
                };

                return Ok(data);
            }
            return Err("汽水音乐: 歌词内容为空".into());
        }
        return Err("汽水音乐: 无歌曲详细信息".into());
    }
    Err("汽水音乐: 歌曲没有歌词".into())
}


//bro 懂我的歌品
#[cfg(test)]
mod tests {

    use super::*;
    #[allow(unused_variables)]
    fn jtrack(s: &str) -> TrackMetadata {
        TrackMetadata {
            title: Some("メルト (Melt) (CPK! Remix|かぐや ver.)".to_string()),
            artist: Some(format!("ryo {} 夏吉ゆうこ", s)),
            album: Some("超かぐや姫！".to_string()),
            album_artist: Some("超かぐや姫！".to_string()),
            duration_ms: Some(271627),
            ..Default::default()
        }
    }

    #[allow(unused_variables)]
    fn etrack(s: &str) -> TrackMetadata {
        TrackMetadata {
            title: Some("Is There Someone Else?".to_string()),
            artist: Some(format!("The Weeknd")),
            album: Some("".to_string()),
            album_artist: Some("".to_string()),
            duration_ms: None,
            ..Default::default()
        }
    }

    #[allow(unused)]
    fn ttrack(s: &str) -> TrackMetadata {
        TrackMetadata {
            title: Some("Extraordinary".to_string()),
            artist: Some(format!("Connor Price")),
            album: Some("".to_string()),
            album_artist: Some("".to_string()),
            duration_ms: None,
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_netease(){
        let track = etrack("/");
        #[allow(unused_variables)]
        let result = fetch_netease_lyrics(&track).await;
        println!("{:?}",result)
    }

    #[tokio::test]
    async fn test_qqmusic(){
        let track = jtrack("/");
        #[allow(unused_variables)]
        let result = fetch_qqmusic_lyrics(&track).await;
        println!("{:?}",result)        
    }

    #[tokio::test]
    async fn test_kugou_music(){
        let track = jtrack("、");
        #[allow(unused_variables)]
        let result = fetch_kugou_lyrics(&track).await;
        println!("{:?}",result)
    }

    #[tokio::test]
    async fn test_soda_music(){
        let track = jtrack(",");
        #[allow(unused_variables)]
        let result = fetch_soda_music_lyrics(&track).await;
        println!("{:?}",result)
    }
}