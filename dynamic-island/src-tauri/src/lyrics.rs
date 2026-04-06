use crate::shared_http_client;
use regex::Regex;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
pub(crate) struct LyricLine {
    pub time_ms: i64,
    pub text: String,
}

enum LogLevel {
    Info,
    Warn,
}

fn lyric_log_file() -> Option<&'static Mutex<File>> {
    static LOG_FILE: OnceLock<Option<Mutex<File>>> = OnceLock::new();
    LOG_FILE
        .get_or_init(|| {
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            let file_name = format!("lyrics_{}.log", ts);
            let file_path: PathBuf = match std::env::current_dir() {
                Ok(dir) => dir.join(file_name),
                Err(_) => return None,
            };
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(file_path)
                .ok()?;
            Some(Mutex::new(file))
        })
        .as_ref()
}

fn lyric_log(level: LogLevel, message: &str) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let level = match level {
        LogLevel::Info => "INFO",
        LogLevel::Warn => "WARN",
    };
    let line = format!("[Lyrics][{}][{}] {}", now, level, message);
    println!("{}", line);
    if let Some(file) = lyric_log_file() {
        let mut file = file.lock().unwrap_or_else(|e| e.into_inner());
        let _ = writeln!(file, "{}", line);
    }
}

pub(crate) fn lyric_log_info(message: &str) {
    lyric_log(LogLevel::Info, message);
}

pub(crate) fn lyric_log_warn(message: &str) {
    lyric_log(LogLevel::Warn, message);
}

fn is_meta_lyric_text(text: &str) -> bool {
    let meta_prefixes = [
        "作词",
        "作曲",
        "编曲",
        "制作",
        "混音",
        "母带",
        "录音",
        "Lyrics by",
        "Composed by",
        "Produced by",
        "Arranged by",
    ];
    meta_prefixes.iter().any(|p| text.starts_with(p))
}

pub(crate) fn parse_synced_lyrics(lrc: &str) -> Vec<LyricLine> {
    let mut lines = Vec::new();
    for line in lrc.lines() {
        let line = line.trim();
        if !line.starts_with('[') { continue; }
        if let Some(end) = line.find(']') {
            let tag = &line[1..end];
            let text = line[end+1..].trim().to_string();
            if let Some(ms) = parse_lrc_time(tag) {
                if !text.is_empty() && !is_meta_lyric_text(&text) {
                    lines.push(LyricLine { time_ms: ms, text });
                }
            }
        }
    }
    lines.sort_by_key(|l| l.time_ms);
    lines
}

fn parse_lrc_time(tag: &str) -> Option<i64> {
    // [mm:ss.xx] or [mm:ss.xxx]
    let parts: Vec<&str> = tag.split(':').collect();
    if parts.len() != 2 { return None; }
    let min: i64 = parts[0].parse().ok()?;
    let sec_parts: Vec<&str> = parts[1].split('.').collect();
    if sec_parts.is_empty() { return None; }
    let sec: i64 = sec_parts[0].parse().ok()?;
    let ms = if sec_parts.len() > 1 {
        let frac = sec_parts[1];
        let val: i64 = frac.parse().ok()?;
        if frac.len() == 2 { val * 10 } else { val }
    } else { 0 };
    Some(min * 60000 + sec * 1000 + ms)
}

fn extract_ncm_id_from_genre(genre: &str) -> Option<i64> {
    static NCM_GENRE_RE: OnceLock<Option<Regex>> = OnceLock::new();
    let re = NCM_GENRE_RE
        .get_or_init(|| Regex::new(r"(?i)ncm[-_: ]*(\d{3,})").ok())
        .as_ref()?;
    re.captures(genre)
        .and_then(|caps| caps.get(1).map(|m| m.as_str().to_string()))
        .and_then(|id| id.parse::<i64>().ok())
}

fn fetch_netease_lyrics_by_song_id(song_id: i64) -> Option<Vec<LyricLine>> {
    let client = shared_http_client();
    let lyric_urls = [
        format!(
            "https://music.163.com/api/song/lyric?id={}&lv=1&tv=-1&rv=1",
            song_id
        ),
        format!("https://music.163.com/api/song/lyric?id={}&lv=-1", song_id),
    ];

    for (idx, lyric_url) in lyric_urls.iter().enumerate() {
        lyric_log(
            LogLevel::Info,
            &format!(
                "lyric source=SMTC-GENRE-NCM step=fetch attempt={} url='{}'",
                idx + 1,
                lyric_url
            ),
        );

        let lyric_resp = match client
            .get(lyric_url)
            .header("Referer", "https://music.163.com")
            .header("User-Agent", "Mozilla/5.0")
            .send()
        {
            Ok(v) => v,
            Err(_) => continue,
        };

        let lyric_json: serde_json::Value = match lyric_resp.json() {
            Ok(v) => v,
            Err(_) => continue,
        };

        if let Some(lrc_str) = lyric_json
            .get("lrc")
            .and_then(|v| v.get("lyric"))
            .and_then(|v| v.as_str())
        {
            if lrc_str.is_empty() {
                continue;
            }
            let lines = parse_synced_lyrics(lrc_str);
            if !lines.is_empty() {
                lyric_log(
                    LogLevel::Info,
                    &format!(
                        "lyric source=SMTC-GENRE-NCM step=parse lines={} song_id={}",
                        lines.len(),
                        song_id
                    ),
                );
                return Some(lines);
            }
        }
    }

    lyric_log(
        LogLevel::Warn,
        &format!(
            "lyric source=SMTC-GENRE-NCM empty song_id={} both_urls_failed",
            song_id
        ),
    );
    None
}

/// 使用 SMTC genre 中的 NCM ID 直连网易云歌词接口获取歌词
pub(crate) fn fetch_lyrics_parallel(
    title: &str,
    artist: &str,
    genre: &str,
    ncm_genre_hit_enabled: bool,
    api_search_enabled: bool,
) -> Option<Vec<LyricLine>> {
    lyric_log(
        LogLevel::Info,
        &format!(
            "start lyric fetch song='{}' artist='{}' genre='{}' strategy=genre_ncmid",
            title, artist, genre
        ),
    );

    if !api_search_enabled {
        lyric_log(LogLevel::Info, "lyric source=API disabled");
        return None;
    }

    if !ncm_genre_hit_enabled {
        lyric_log(
            LogLevel::Info,
            "lyric source=SMTC-GENRE-NCM disabled by setting",
        );
        return None;
    }

    let song_id = match extract_ncm_id_from_genre(genre) {
        Some(id) => id,
        None => {
            lyric_log(
                LogLevel::Warn,
                &format!(
                    "lyric source=SMTC-GENRE-NCM invalid_genre genre='{}' no_ncmid",
                    genre
                ),
            );
            return None;
        }
    };

    lyric_log(
        LogLevel::Info,
        &format!(
            "lyric source=SMTC-GENRE-NCM extracted_song_id={} genre='{}'",
            song_id, genre
        ),
    );

    fetch_netease_lyrics_by_song_id(song_id)
}

pub(crate) fn get_current_lyric(lyrics: &[LyricLine], position_ms: i64) -> Option<&LyricLine> {
    if lyrics.is_empty() { return None; }
    let mut result = None;
    for line in lyrics {
        if line.time_ms <= position_ms {
            result = Some(line);
        } else {
            break;
        }
    }
    result
}

/// 获取当前播放位置周围的歌词行（前2行、当前行、后2行）
pub(crate) fn get_nearby_lyrics(lyrics: &[LyricLine], position_ms: i64) -> Vec<(String, bool)> {
    if lyrics.is_empty() { return Vec::new(); }
    const UPCOMING_PREVIEW_WINDOW_MS: i64 = 8_000;
    // 找到当前行索引
    let mut current_idx: Option<usize> = None;
    for (i, line) in lyrics.iter().enumerate() {
        if line.time_ms <= position_ms {
            current_idx = Some(i);
        } else {
            break;
        }
    }
    let current_idx = match current_idx {
        Some(i) => i,
        None => return Vec::new(),
    };
    let start = current_idx.saturating_sub(2);
    let end = (current_idx + 3).min(lyrics.len());
    let mut result = Vec::new();
    for i in start..end {
        if i > current_idx {
            let delta_ms = lyrics[i].time_ms - position_ms;
            if delta_ms > UPCOMING_PREVIEW_WINDOW_MS {
                continue;
            }
        }
        result.push((lyrics[i].text.clone(), i == current_idx));
    }
    result
}

