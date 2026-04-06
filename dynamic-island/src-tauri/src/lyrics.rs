use crate::shared_http_client;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use tungstenite::{connect, Message};

#[derive(Clone, Debug)]
pub(crate) struct LyricLine {
    pub time_ms: i64,
    pub text: String,
}

#[derive(Debug, Deserialize)]
struct LocalWsMessage {
    #[serde(rename = "type")]
    msg_type: String,
    song: Option<LocalWsSong>,
    lyrics: Option<LocalWsLyrics>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocalWsSong {
    #[serde(default)]
    song_name: String,
    #[serde(default)]
    author_name: String,
    #[serde(default)]
    ncm_id: i64,
}

#[derive(Debug, Deserialize)]
struct LocalWsLyrics {
    #[serde(default)]
    lines: Vec<LocalWsLyricLine>,
}

#[derive(Debug, Deserialize)]
struct LocalWsLyricLine {
    #[serde(default)]
    time: f64,
    #[serde(default)]
    text: String,
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

fn normalize_for_match(input: &str) -> String {
    input
        .trim()
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(|c| c.to_lowercase())
        .collect::<String>()
}

fn is_local_ws_song_match(song: &LocalWsSong, title: &str, artist: &str) -> bool {
    let ws_title = normalize_for_match(&clean_title(&song.song_name));
    let expected_title = normalize_for_match(&clean_title(title));
    if ws_title.is_empty() || expected_title.is_empty() {
        return false;
    }

    let title_match = ws_title == expected_title
        || ws_title.contains(&expected_title)
        || expected_title.contains(&ws_title);

    let ws_artist = normalize_for_match(song.author_name.split(['/', ',', '&']).next().unwrap_or("").trim());
    let expected_artist = normalize_for_match(artist.split(['/', ',', '&']).next().unwrap_or(artist).trim());
    let artist_match = ws_artist.is_empty()
        || expected_artist.is_empty()
        || ws_artist == expected_artist
        || ws_artist.contains(&expected_artist)
        || expected_artist.contains(&ws_artist);

    title_match && artist_match
}

fn convert_local_ws_lines(lines: Vec<LocalWsLyricLine>) -> Option<Vec<LyricLine>> {
    let mut parsed = Vec::new();
    for line in lines {
        let text = line.text.trim();
        if text.is_empty() || is_meta_lyric_text(text) {
            continue;
        }
        parsed.push(LyricLine {
            time_ms: line.time.max(0.0).round() as i64,
            text: text.to_string(),
        });
    }
    parsed.sort_by_key(|l| l.time_ms);
    if parsed.is_empty() { None } else { Some(parsed) }
}

fn ws_payload_preview(payload: &str, max_chars: usize) -> String {
    let normalized = payload.replace('\r', "\\r").replace('\n', "\\n");
    let total_chars = normalized.chars().count();
    let preview = normalized.chars().take(max_chars).collect::<String>();
    if total_chars > max_chars {
        format!("{}... (truncated, total_chars={})", preview, total_chars)
    } else {
        format!("{} (total_chars={})", preview, total_chars)
    }
}

fn parse_local_ws_payload(payload: &str, title: &str, artist: &str) -> Option<Vec<LyricLine>> {
    let msg: LocalWsMessage = match serde_json::from_str(payload) {
        Ok(v) => v,
        Err(primary_err) => {
            if let Ok(inner_json) = serde_json::from_str::<String>(payload) {
                if let Ok(v) = serde_json::from_str::<LocalWsMessage>(&inner_json) {
                    v
                } else {
                    let preview = ws_payload_preview(payload, 200);
                    lyric_log(
                        LogLevel::Info,
                        &format!(
                            "local ws payload parse failed err='{}' preview='{}'",
                            primary_err, preview
                        ),
                    );
                    return None;
                }
            } else {
                let preview = ws_payload_preview(payload, 200);
                lyric_log(
                    LogLevel::Info,
                    &format!(
                        "local ws payload parse failed err='{}' preview='{}'",
                        primary_err, preview
                    ),
                );
                return None;
            }
        }
    };
    if msg.msg_type != "lyricUpdate" {
        return None;
    }

    let song = msg.song?;
    if !is_local_ws_song_match(&song, title, artist) {
        return None;
    }

    lyric_log(
        LogLevel::Info,
        &format!(
            "matched local lyricUpdate ncmId={} song='{}' artist='{}'",
            song.ncm_id, song.song_name, song.author_name
        ),
    );

    let lyrics = msg.lyrics?;
    convert_local_ws_lines(lyrics.lines)
}

fn fetch_lyrics_from_local_ws(title: &str, artist: &str) -> Option<Vec<LyricLine>> {
    const MAX_WS_RETRIES: u32 = 5;
    let mut attempt = 0u32;

    while attempt < MAX_WS_RETRIES {
        attempt += 1;

        let (mut socket, _) = match connect("ws://127.0.0.1:11452") {
            Ok(v) => v,
            Err(_) => {
                lyric_log(
                    LogLevel::Info,
                    &format!("local ws connect failed attempt={}", attempt),
                );
                if attempt < MAX_WS_RETRIES {
                    std::thread::sleep(Duration::from_millis(120));
                }
                continue;
            }
        };

        if let tungstenite::stream::MaybeTlsStream::Plain(stream) = socket.get_mut() {
            let _ = stream.set_read_timeout(Some(Duration::from_millis(350)));
            let _ = stream.set_write_timeout(Some(Duration::from_millis(350)));
        }

        lyric_log(
            LogLevel::Info,
            &format!("local ws connected attempt={}", attempt),
        );

        let mut need_reconnect = false;
        let read_deadline = Instant::now() + Duration::from_millis(1800);
        while Instant::now() < read_deadline {
            match socket.read() {
                Ok(Message::Text(text)) => {
                    if let Some(lines) = parse_local_ws_payload(&text, title, artist) {
                        let _ = socket.close(None);
                        return Some(lines);
                    }
                }
                Ok(Message::Binary(bin)) => {
                    if let Ok(text) = String::from_utf8(bin) {
                        if let Some(lines) = parse_local_ws_payload(&text, title, artist) {
                            let _ = socket.close(None);
                            return Some(lines);
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    lyric_log(
                        LogLevel::Info,
                        &format!("local ws closed attempt={} reconnecting", attempt),
                    );
                    need_reconnect = true;
                    break;
                }
                Ok(_) => {}
                Err(tungstenite::Error::ConnectionClosed) => {
                    lyric_log(
                        LogLevel::Info,
                        &format!("local ws connection closed attempt={} reconnecting", attempt),
                    );
                    need_reconnect = true;
                    break;
                }
                Err(tungstenite::Error::Io(err))
                    if err.kind() == std::io::ErrorKind::WouldBlock
                        || err.kind() == std::io::ErrorKind::TimedOut => {}
                Err(tungstenite::Error::Io(err))
                    if err.kind() == std::io::ErrorKind::ConnectionReset
                        || err.kind() == std::io::ErrorKind::ConnectionAborted
                        || err.kind() == std::io::ErrorKind::BrokenPipe
                        || err.kind() == std::io::ErrorKind::NotConnected => {
                    lyric_log(
                        LogLevel::Info,
                        &format!(
                            "local ws disconnected attempt={} kind={:?} reconnecting",
                            attempt,
                            err.kind()
                        ),
                    );
                    need_reconnect = true;
                    break;
                }
                Err(_) => {
                    lyric_log(
                        LogLevel::Info,
                        &format!("local ws read failed attempt={} stop current ws", attempt),
                    );
                    break;
                }
            }
        }

        let _ = socket.close(None);
        if need_reconnect && attempt < MAX_WS_RETRIES {
            std::thread::sleep(Duration::from_millis(80));
            continue;
        }
        break;
    }

    lyric_log(
        LogLevel::Info,
        &format!(
            "local ws no matched lyricUpdate attempts={} song='{}' artist='{}'",
            attempt, title, artist
        ),
    );
    None
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

/// 清理歌曲标题，去除括号内容、feat信息等干扰搜索的部分
fn clean_title(title: &str) -> String {
    let mut s = title.to_string();
    // 去除各种括号内容: (feat. X), [Remix], （翻唱）等
    for (open, close) in [('(', ')'), ('[', ']')] {
        while let Some(start) = s.find(open) {
            if let Some(end) = s[start..].find(close) {
                s = format!("{}{}", &s[..start], &s[start + end + close.len_utf8()..]);
            } else {
                s = s[..start].to_string();
                break;
            }
        }
    }
    // 去除 " - " 后面的副标题
    if let Some(idx) = s.find(" - ") {
        s = s[..idx].to_string();
    }
    s.trim().to_string()
}

/// 从搜索结果数组中提取第一个有 syncedLyrics 的结果
fn extract_synced_from_array(json: &serde_json::Value) -> Option<Vec<LyricLine>> {
    let arr = json.as_array()?;
    for item in arr {
        if let Some(synced) = item.get("syncedLyrics").and_then(|v| v.as_str()) {
            if !synced.is_empty() {
                let lines = parse_synced_lyrics(synced);
                if !lines.is_empty() {
                    return Some(lines);
                }
            }
        }
    }
    None
}


/// 从网易云音乐获取歌词（作为 LRCLIB 的备用源）
pub(crate) fn fetch_lyrics_from_netease(title: &str, artist: &str) -> Option<Vec<LyricLine>> {
    let client = shared_http_client();

    let cleaned_title = clean_title(title);
    let cleaned_artist = artist.split(['/', ',']).next().unwrap_or(artist).trim();
    let query = format!("{} {}", cleaned_title, cleaned_artist);

    lyric_log(
        LogLevel::Info,
        &format!("lyric source=API-NETEASE step=search query='{}'", query),
    );

    // 搜索歌曲
    let search_resp = client.post("https://music.163.com/api/search/get")
        .header("Referer", "https://music.163.com")
        .header("User-Agent", "Mozilla/5.0")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!("s={}&type=1&limit=5&offset=0", urlencoding::encode(&query)))
        .send().ok()?;
    let search_json: serde_json::Value = search_resp.json().ok()?;
    let songs = search_json.get("result")?.get("songs")?.as_array()?;
    if songs.is_empty() {
        lyric_log(LogLevel::Warn, "lyric source=API-NETEASE step=search empty");
        return None;
    }

    // 取第一个结果的 ID
    let song_id = songs[0].get("id")?.as_i64()?;
    lyric_log(
        LogLevel::Info,
        &format!("lyric source=API-NETEASE step=search matched_song_id={}", song_id),
    );

    // 获取歌词
    let lyric_url = format!("https://music.163.com/api/song/lyric?id={}&lv=1", song_id);
    lyric_log(
        LogLevel::Info,
        &format!("lyric source=API-NETEASE step=fetch url='{}'", lyric_url),
    );
    let lyric_resp = client.get(&lyric_url)
        .header("Referer", "https://music.163.com")
        .header("User-Agent", "Mozilla/5.0")
        .send().ok()?;
    let lyric_json: serde_json::Value = lyric_resp.json().ok()?;
    let lrc_str = lyric_json.get("lrc")?.get("lyric")?.as_str()?;
    if lrc_str.is_empty() {
        lyric_log(LogLevel::Warn, "lyric source=API-NETEASE step=fetch empty_lyric");
        return None;
    }

    let lines = parse_synced_lyrics(lrc_str);
    if lines.is_empty() {
        lyric_log(LogLevel::Warn, "lyric source=API-NETEASE step=parse empty_lines");
        None
    } else {
        lyric_log(
            LogLevel::Info,
            &format!("lyric source=API-NETEASE step=parse lines={}", lines.len()),
        );
        Some(lines)
    }
}

pub(crate) fn fetch_lyrics_from_lrclib(title: &str, artist: &str) -> Option<Vec<LyricLine>> {
    let client = shared_http_client();
    let ua = "DynamicIsland/1.0 (https://github.com/user/dynamic-island)";

    let cleaned_title = clean_title(title);
    let cleaned_artist = artist.split(['/', ',']).next().unwrap_or(artist).trim();

    lyric_log(
        LogLevel::Info,
        &format!(
            "lyric source=API-LRCLIB step=search strategy=track+artist title='{}' artist='{}'",
            title, artist
        ),
    );

    // 策略1: /api/search?track_name=X&artist_name=Y (原始标题)
    let url1 = format!(
        "https://lrclib.net/api/search?track_name={}&artist_name={}",
        urlencoding::encode(title), urlencoding::encode(artist)
    );
    if let Ok(resp) = client.get(&url1).header("User-Agent", ua).send() {
        if let Ok(json) = resp.json::<serde_json::Value>() {
            if let Some(lines) = extract_synced_from_array(&json) {
                lyric_log(
                    LogLevel::Info,
                    &format!("lyric source=API-LRCLIB step=parse strategy=track+artist lines={}", lines.len()),
                );
                return Some(lines);
            }
        }
    }

    // 策略2: /api/search?q= 自由搜索（清理后的标题+艺术家）
    let query = format!("{} {}", cleaned_title, cleaned_artist);
    lyric_log(
        LogLevel::Info,
        &format!("lyric source=API-LRCLIB step=search strategy=q query='{}'", query),
    );
    let url2 = format!(
        "https://lrclib.net/api/search?q={}",
        urlencoding::encode(&query)
    );
    if let Ok(resp) = client.get(&url2).header("User-Agent", ua).send() {
        if let Ok(json) = resp.json::<serde_json::Value>() {
            if let Some(lines) = extract_synced_from_array(&json) {
                lyric_log(
                    LogLevel::Info,
                    &format!("lyric source=API-LRCLIB step=parse strategy=q lines={}", lines.len()),
                );
                return Some(lines);
            }
        }
    }

    lyric_log(LogLevel::Warn, "lyric source=API-LRCLIB no_lyrics_found");
    None
}

/// 优先尝试本地 ws(11452) lyricUpdate，失败后并行从 LRCLIB 和网易云获取
pub(crate) fn fetch_lyrics_parallel(
    title: &str,
    artist: &str,
    ws_enabled: bool,
    api_search_enabled: bool,
) -> Option<Vec<LyricLine>> {
    use std::sync::mpsc;
    use std::thread;

    lyric_log(
        LogLevel::Info,
        &format!("start lyric fetch song='{}' artist='{}'", title, artist),
    );

    if ws_enabled {
        if let Some(lines) = fetch_lyrics_from_local_ws(title, artist) {
            lyric_log(
                LogLevel::Info,
                &format!("lyric source=WS lines={}", lines.len()),
            );
            return Some(lines);
        }
        lyric_log(LogLevel::Info, "lyric source=WS unavailable, fallback to API");
    } else {
        lyric_log(LogLevel::Info, "lyric source=WS disabled");
    }

    if !api_search_enabled {
        lyric_log(LogLevel::Info, "lyric source=API disabled");
        return None;
    }

    let (tx, rx) = mpsc::channel::<(&'static str, Option<Vec<LyricLine>>)>() ;

    let tx1 = tx.clone();
    let t1 = title.to_string();
    let a1 = artist.to_string();
    thread::Builder::new()
        .name("lrclib-fetch".into())
        .stack_size(512 * 1024)
        .spawn(move || {
            let result = std::panic::catch_unwind(|| {
                fetch_lyrics_from_lrclib(&t1, &a1)
            }).unwrap_or(None);
            let _ = tx1.send(("API-LRCLIB", result));
        }).ok();

    let tx2 = tx.clone();
    let t2 = title.to_string();
    let a2 = artist.to_string();
    thread::Builder::new()
        .name("netease-fetch".into())
        .stack_size(512 * 1024)
        .spawn(move || {
            let result = std::panic::catch_unwind(|| {
                fetch_lyrics_from_netease(&t2, &a2)
            }).unwrap_or(None);
            let _ = tx2.send(("API-NETEASE", result));
        }).ok();

    drop(tx);

    // 收集两个结果，优先返回有数据的
    let mut results_received = 0;
    while let Ok((source, result)) = rx.recv() {
        results_received += 1;
        if let Some(lines) = result {
            if !lines.is_empty() {
                lyric_log(
                    LogLevel::Info,
                    &format!("lyric source={} lines={}", source, lines.len()),
                );
                return Some(lines);
            }
        }
        lyric_log(LogLevel::Warn, &format!("lyric source={} empty", source));
        if results_received >= 2 {
            break;
        }
    }
    lyric_log(LogLevel::Warn, "lyric source=NONE, no lyrics found from WS/API");
    None
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

