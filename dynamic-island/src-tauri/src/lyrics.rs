use crate::shared_http_client;
use regex::Regex;
use std::sync::OnceLock;

#[derive(Clone, Debug)]
pub(crate) struct LyricLine {
    pub time_ms: i64,
    pub text: String,
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

fn fetch_netease_lyrics_by_song_id(song_id: i64, source: &str) -> Option<Vec<LyricLine>> {
    let client = shared_http_client();
    let lyric_urls = [
        format!(
            "https://music.163.com/api/song/lyric?id={}&lv=1&tv=-1&rv=1",
            song_id
        ),
        format!("https://music.163.com/api/song/lyric?id={}&lv=-1", song_id),
    ];

    for (idx, lyric_url) in lyric_urls.iter().enumerate() {
        crate::logger::info("Lyrics", &format!(
            "{}: fetch attempt={} url='{}'",
            source, idx + 1, lyric_url
        ));

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
                crate::logger::info("Lyrics", &format!(
                    "{}: parse ok lines={} song_id={}",
                    source, lines.len(), song_id
                ));
                return Some(lines);
            }
        }
    }

    crate::logger::warn("Lyrics", &format!("{}: both urls failed song_id={}", source, song_id));
    None
}

fn fetch_netease_song_id_by_search(title: &str, artist: &str) -> Option<i64> {
    let keyword = if artist.trim().is_empty() {
        title.trim().to_string()
    } else {
        format!("{} {}", title.trim(), artist.trim())
    };
    if keyword.is_empty() {
        crate::logger::warn("Lyrics", "\napi-search: empty keyword");
        return None;
    }

    let encoded = urlencoding::encode(&keyword);
    let search_url = format!(
        "https://music.163.com/api/search/get/web?csrf_token=&s={}&type=1&offset=0&total=true&limit=8",
        encoded
    );
    crate::logger::info("Lyrics", &format!("\napi-search: keyword='{}'", keyword));

    let client = shared_http_client();
    let resp = match client
        .get(&search_url)
        .header("Referer", "https://music.163.com")
        .header("User-Agent", "Mozilla/5.0")
        .send()
    {
        Ok(v) => v,
        Err(_) => {
            crate::logger::warn("Lyrics", "api-search: search request failed");
            return None;
        }
    };

    let search_json: serde_json::Value = match resp.json() {
        Ok(v) => v,
        Err(_) => {
            crate::logger::warn("Lyrics", "api-search: search invalid json");
            return None;
        }
    };

    let songs = match search_json
        .get("result")
        .and_then(|v| v.get("songs"))
        .and_then(|v| v.as_array())
    {
        Some(v) if !v.is_empty() => v,
        _ => {
            crate::logger::warn("Lyrics", "api-search: no song results");
            return None;
        }
    };

    let title_lc = title.to_lowercase();
    let artist_lc = artist.to_lowercase();

    let mut fallback_id: Option<i64> = None;
    for song in songs {
        let id = match song.get("id").and_then(|v| v.as_i64()) {
            Some(v) => v,
            None => continue,
        };
        if fallback_id.is_none() {
            fallback_id = Some(id);
        }

        let name = song
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_lowercase();
        let artists_joined = song
            .get("artists")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|a| a.get("name").and_then(|n| n.as_str()))
                    .collect::<Vec<_>>()
                    .join("/")
                    .to_lowercase()
            })
            .unwrap_or_default();

        let title_ok = !title_lc.is_empty() && name.contains(&title_lc);
        let artist_ok = artist_lc.is_empty() || artists_joined.contains(&artist_lc);
        if title_ok && artist_ok {
            crate::logger::info("Lyrics", &format!(
                "api-search: matched song id={} name='{}' artists='{}'",
                id, name, artists_joined
            ));
            return Some(id);
        }
    }

    if let Some(id) = fallback_id {
        crate::logger::info("Lyrics", &format!(
            "api-search: using first song id={} total_candidates={}",
            id, songs.len()
        ));
    }
    fallback_id
}

/// 通过 lyricify-lyrics-provider 统一接口获取歌词（自动检测播放器，多源 fallback）
fn fetch_lyrics_by_rust_api(title: &str, artist: &str) -> Option<Vec<LyricLine>> {
    use lyricify_lyrics_provider::smtc_lyrics;

    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            crate::logger::warn("Lyrics", &format!("\nrust-api: tokio runtime init failed: {}", e));
            return None;
        }
    };

    let artist_opt = if artist.trim().is_empty() { None } else { Some(artist) };

    rt.block_on(async {
        crate::logger::info("Lyrics", &format!(
            "\nrust-api: title='{}' artist='{}'", title, artist
        ));

        // 1) 尝试自动检测播放器
        let running = smtc_lyrics::get_running_players();
        if !running.is_empty() {
            let names: Vec<&str> = running.iter().map(|p| p.display_name()).collect();
            crate::logger::info("Lyrics", &format!(
                "rust-api: running players=[{}]", names.join(", ")
            ));
        } else {
            crate::logger::info("Lyrics", "rust-api: no running players detected");
        }

        match smtc_lyrics::get_lyrics(title, artist_opt, None, None).await {
            Ok((player, data)) => {
                let lines = lyrics_data_to_lyric_lines(&data);
                if !lines.is_empty() {
                    crate::logger::info("Lyrics", &format!(
                        "rust-api: done player='{}' lines={}",
                        player.display_name(), lines.len()
                    ));
                    return Some(lines);
                }
                crate::logger::warn("Lyrics", &format!(
                    "rust-api: player='{}' empty after convert", player.display_name()
                ));
            }
            Err(e) => {
                crate::logger::warn("Lyrics", &format!(
                    "rust-api: auto detect failed: {}", e
                ));
            }
        }

        // 2) 自动检测失败，按优先级逐个尝试：网易云 → QQ音乐 → 汽水音乐
        let fallback_players = [
            smtc_lyrics::MusicPlayer::Netease,
            smtc_lyrics::MusicPlayer::QQMusic,
            smtc_lyrics::MusicPlayer::SodaMusic,
        ];
        for player in &fallback_players {
            crate::logger::info("Lyrics", &format!(
                "rust-api: fallback trying '{}'", player.display_name()
            ));
            match smtc_lyrics::get_lyrics_with_player(player, title, artist_opt, None, None).await {
                Ok(data) => {
                    let lines = lyrics_data_to_lyric_lines(&data);
                    if !lines.is_empty() {
                        crate::logger::info("Lyrics", &format!(
                            "rust-api: fallback done player='{}' lines={}",
                            player.display_name(), lines.len()
                        ));
                        return Some(lines);
                    }
                }
                Err(e) => {
                    crate::logger::warn("Lyrics", &format!(
                        "rust-api: fallback player='{}' failed: {}",
                        player.display_name(), e
                    ));
                }
            }
        }

        crate::logger::warn("Lyrics", "rust-api: all sources exhausted");
        None
    })
}

/// 将 LyricsData 转换为 Vec<LyricLine>
fn lyrics_data_to_lyric_lines(
    data: &lyricify_lyrics_provider::models::LyricsData,
) -> Vec<LyricLine> {
    data.lines
        .iter()
        .filter_map(|line| {
            let text = line.text();
            if text.trim().is_empty() {
                return None;
            }
            let time_ms = line.start_time().unwrap_or(0) as i64;
            Some(LyricLine { time_ms, text })
        })
        .collect()
}

/// 使用 SMTC genre 中的 NCM ID 直连网易云歌词接口获取歌词
pub(crate) fn fetch_lyrics_parallel(
    title: &str,
    artist: &str,
    genre: &str,
    ncm_genre_hit_enabled: bool,
    rust_api_enabled: bool,
    api_search_enabled: bool,
) -> Option<Vec<LyricLine>> {
    crate::logger::info("Lyrics", &format!(
        "\nlyric-fetch: song='{}' artist='{}' genre='{}'",
        title, artist, genre
    ));

    if ncm_genre_hit_enabled {
        if let Some(song_id) = extract_ncm_id_from_genre(genre) {
            crate::logger::info("Lyrics", &format!(
                "\nsmtc-genre-ncm: extracted song_id={} genre='{}'",
                song_id, genre
            ));
            if let Some(lines) = fetch_netease_lyrics_by_song_id(song_id, "smtc-genre-ncm") {
                return Some(lines);
            }
            crate::logger::warn("Lyrics", &format!(
                "smtc-genre-ncm: fallback to search song_id={}",
                song_id
            ));
        } else {
            crate::logger::warn("Lyrics", &format!(
                "smtc-genre-ncm: no ncmid in genre='{}' fallback to search",
                genre
            ));
        }
    } else {
        crate::logger::info("Lyrics", "smtc-genre-ncm: disabled by setting");
    }

    // --- 第二优先：Rust API（lyricify-lyrics-helper 库） ---
    if rust_api_enabled {
        crate::logger::info("Lyrics", &format!("rust-api: enabled, title='{}' artist='{}'", title, artist));
        if let Some(lines) = fetch_lyrics_by_rust_api(title, artist) {
            return Some(lines);
        }
        crate::logger::warn("Lyrics", "rust-api: failed, fallback to api-search");
    } else {
        crate::logger::info("Lyrics", "rust-api: disabled by setting");
    }

    // --- 第三优先：API 搜索保底 ---
    if !api_search_enabled {
        crate::logger::info("Lyrics", "\napi-search: disabled by setting");
        return None;
    }

    let search_song_id = match fetch_netease_song_id_by_search(title, artist) {
        Some(id) => id,
        None => return None,
    };
    crate::logger::info("Lyrics", &format!(
        "\napi-search: fetch lyrics song_id={} title='{}' artist='{}'",
        search_song_id, title, artist
    ));
    fetch_netease_lyrics_by_song_id(search_song_id, "api-search")
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

