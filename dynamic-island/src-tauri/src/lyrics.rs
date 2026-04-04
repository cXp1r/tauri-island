use crate::shared_http_client;

#[derive(Clone, Debug)]
pub(crate) struct LyricLine {
    pub time_ms: i64,
    pub text: String,
}

pub(crate) fn parse_synced_lyrics(lrc: &str) -> Vec<LyricLine> {
    let mut lines = Vec::new();
    let meta_prefixes = ["作词", "作曲", "编曲", "制作", "混音", "母带", "录音", "Lyrics by", "Composed by", "Produced by", "Arranged by"];
    for line in lrc.lines() {
        let line = line.trim();
        if !line.starts_with('[') { continue; }
        if let Some(end) = line.find(']') {
            let tag = &line[1..end];
            let text = line[end+1..].trim().to_string();
            if let Some(ms) = parse_lrc_time(tag) {
                if !text.is_empty() && !meta_prefixes.iter().any(|p| text.starts_with(p)) {
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

    // 搜索歌曲
    let search_resp = client.post("https://music.163.com/api/search/get")
        .header("Referer", "https://music.163.com")
        .header("User-Agent", "Mozilla/5.0")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!("s={}&type=1&limit=5&offset=0", urlencoding::encode(&query)))
        .send().ok()?;
    let search_json: serde_json::Value = search_resp.json().ok()?;
    let songs = search_json.get("result")?.get("songs")?.as_array()?;
    if songs.is_empty() { return None; }

    // 取第一个结果的 ID
    let song_id = songs[0].get("id")?.as_i64()?;

    // 获取歌词
    let lyric_url = format!("https://music.163.com/api/song/lyric?id={}&lv=1", song_id);
    let lyric_resp = client.get(&lyric_url)
        .header("Referer", "https://music.163.com")
        .header("User-Agent", "Mozilla/5.0")
        .send().ok()?;
    let lyric_json: serde_json::Value = lyric_resp.json().ok()?;
    let lrc_str = lyric_json.get("lrc")?.get("lyric")?.as_str()?;
    if lrc_str.is_empty() { return None; }

    let lines = parse_synced_lyrics(lrc_str);
    if lines.is_empty() { None } else { Some(lines) }
}

pub(crate) fn fetch_lyrics_from_lrclib(title: &str, artist: &str) -> Option<Vec<LyricLine>> {
    let client = shared_http_client();
    let ua = "DynamicIsland/1.0 (https://github.com/user/dynamic-island)";

    let cleaned_title = clean_title(title);
    let cleaned_artist = artist.split(['/', ',']).next().unwrap_or(artist).trim();

    // 策略1: /api/search?track_name=X&artist_name=Y (原始标题)
    let url1 = format!(
        "https://lrclib.net/api/search?track_name={}&artist_name={}",
        urlencoding::encode(title), urlencoding::encode(artist)
    );
    if let Ok(resp) = client.get(&url1).header("User-Agent", ua).send() {
        if let Ok(json) = resp.json::<serde_json::Value>() {
            if let Some(lines) = extract_synced_from_array(&json) {
                return Some(lines);
            }
        }
    }

    // 策略2: /api/search?q= 自由搜索（清理后的标题+艺术家）
    let query = format!("{} {}", cleaned_title, cleaned_artist);
    let url2 = format!(
        "https://lrclib.net/api/search?q={}",
        urlencoding::encode(&query)
    );
    if let Ok(resp) = client.get(&url2).header("User-Agent", ua).send() {
        if let Ok(json) = resp.json::<serde_json::Value>() {
            if let Some(lines) = extract_synced_from_array(&json) {
                return Some(lines);
            }
        }
    }

    None
}

/// 并行从 LRCLIB 和网易云获取歌词，取先完成且有结果的一方
pub(crate) fn fetch_lyrics_parallel(title: &str, artist: &str) -> Option<Vec<LyricLine>> {
    use std::sync::mpsc;
    use std::thread;

    let (tx, rx) = mpsc::channel::<Option<Vec<LyricLine>>>();

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
            let _ = tx1.send(result);
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
            let _ = tx2.send(result);
        }).ok();

    drop(tx);

    // 收集两个结果，优先返回有数据的
    let mut results_received = 0;
    while let Ok(result) = rx.recv() {
        results_received += 1;
        if let Some(lines) = result {
            if !lines.is_empty() {
                return Some(lines);
            }
        }
        if results_received >= 2 {
            break;
        }
    }
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
        result.push((lyrics[i].text.clone(), i == current_idx));
    }
    result
}

