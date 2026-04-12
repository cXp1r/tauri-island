use crate::shared_http_client;
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use flate2::read::ZlibDecoder;
use regex::Regex;
use std::io::Read;
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

// ===== 酷狗音乐歌词获取 =====

fn kugou_search_song(title: &str, artist: &str) -> Option<(String, i64)> {
    let keyword = if artist.trim().is_empty() {
        title.trim().to_string()
    } else {
        format!("{} {}", title.trim(), artist.trim())
    };
    if keyword.is_empty() { return None; }
    let encoded = urlencoding::encode(&keyword);
    let url = format!(
        "http://mobilecdn.kugou.com/api/v3/search/song?format=json&keyword={}&page=1&pagesize=8&showtype=1",
        encoded
    );
    crate::logger::info("Lyrics", &format!("kugou-search: keyword='{}'", keyword));
    let client = shared_http_client();
    let json: serde_json::Value = client
        .get(&url).header("User-Agent", "Mozilla/5.0").send().ok()?
        .json().ok()?;
    let info = json
        .get("data").and_then(|d| d.get("info")).and_then(|i| i.as_array())
        .filter(|a| !a.is_empty())?;
    let title_lc = title.to_lowercase();
    let artist_lc = artist.to_lowercase();
    let mut fallback: Option<(String, i64)> = None;
    for item in info {
        let hash = item.get("hash").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let dur_s = item.get("duration").and_then(|v| v.as_i64()).unwrap_or(0);
        if hash.is_empty() { continue; }
        if fallback.is_none() { fallback = Some((hash.clone(), dur_s * 1000)); }
        let name = item.get("songname").and_then(|v| v.as_str()).unwrap_or_default().to_lowercase();
        let singer = item.get("singername").and_then(|v| v.as_str()).unwrap_or_default().to_lowercase();
        if name.contains(&title_lc) && (artist_lc.is_empty() || singer.contains(&artist_lc)) {
            crate::logger::info("Lyrics", &format!("kugou-search: matched name='{}' singer='{}'", name, singer));
            return Some((hash, dur_s * 1000));
        }
    }
    if fallback.is_some() {
        crate::logger::info("Lyrics", "kugou-search: using first result as fallback");
    }
    fallback
}

fn kugou_search_lyric(keyword: &str, duration_ms: i64, hash: &str) -> Option<(String, String)> {
    let encoded_kw = urlencoding::encode(keyword);
    let url = format!(
        "https://lyrics.kugou.com/search?ver=1&man=yes&client=pc&keyword={}&duration={}&hash={}",
        encoded_kw, duration_ms, hash
    );
    crate::logger::info("Lyrics", &format!("kugou-lyric-search: duration_ms={} hash='{}'", duration_ms, hash));
    let client = shared_http_client();
    let json: serde_json::Value = client
        .get(&url).header("User-Agent", "Mozilla/5.0").send().ok()?
        .json().ok()?;
    let candidates = json.get("candidates").and_then(|v| v.as_array())?;
    let first = candidates.first()?;
    let id = first.get("id").and_then(|v| v.as_str())?.to_string();
    let accesskey = first.get("accesskey").and_then(|v| v.as_str())?.to_string();
    crate::logger::info("Lyrics", &format!("kugou-lyric-search: id='{}'", id));
    Some((id, accesskey))
}

fn kugou_download_krc(id: &str, accesskey: &str) -> Option<String> {
    let url = format!(
        "https://lyrics.kugou.com/download?ver=1&client=pc&id={}&accesskey={}&fmt=krc&charset=utf8",
        id, accesskey
    );
    let client = shared_http_client();
    let json: serde_json::Value = client
        .get(&url).header("User-Agent", "Mozilla/5.0").send().ok()?
        .json().ok()?;
    let content = json.get("content").and_then(|v| v.as_str()).map(|s| s.to_string())?;
    if content.is_empty() { return None; }
    crate::logger::info("Lyrics", &format!("kugou-download: content_len={}", content.len()));
    Some(content)
}

fn kugou_decrypt_krc(encoded: &str) -> Option<String> {
    use flate2::read::DeflateDecoder;
    const KEY: &[u8] = &[0x40, 0x47, 0x61, 0x77, 0x5e, 0x32, 0x74, 0x47, 0x51, 0x36, 0x31, 0x2d, 0xce, 0xd2, 0x6e, 0x69];
    let clean: String = encoded.chars().filter(|c| !c.is_whitespace()).collect();
    crate::logger::info("Lyrics", &format!("kugou-decrypt: base64_len={}", clean.len()));
    let decoded = match B64.decode(&clean) {
        Ok(v) => v,
        Err(e) => {
            crate::logger::warn("Lyrics", &format!("kugou-decrypt: base64 decode failed: {}", e));
            return None;
        }
    };
    crate::logger::info("Lyrics", &format!("kugou-decrypt: decoded_len={}", decoded.len()));
    if decoded.len() <= 4 {
        crate::logger::warn("Lyrics", &format!("kugou-decrypt: decoded too short ({})", decoded.len()));
        return None;
    }
    let mut data = decoded[4..].to_vec();
    for (i, byte) in data.iter_mut().enumerate() {
        *byte ^= KEY[i % KEY.len()];
    }
    crate::logger::info("Lyrics", &format!("kugou-decrypt: xor done, data[0..4]={:02x?}", &data[..4.min(data.len())]));
    let inflated = {
        let mut out = Vec::new();
        let zlib_ok = ZlibDecoder::new(&data[..]).read_to_end(&mut out).is_ok() && !out.is_empty();
        if zlib_ok {
            crate::logger::info("Lyrics", &format!("kugou-decrypt: zlib ok, inflated_len={}", out.len()));
            out
        } else {
            crate::logger::warn("Lyrics", "kugou-decrypt: zlib failed, trying raw deflate");
            let mut out2 = Vec::new();
            match DeflateDecoder::new(&data[..]).read_to_end(&mut out2) {
                Ok(_) if !out2.is_empty() => {
                    crate::logger::info("Lyrics", &format!("kugou-decrypt: deflate ok, inflated_len={}", out2.len()));
                    out2
                }
                Ok(_) => {
                    crate::logger::warn("Lyrics", "kugou-decrypt: deflate produced empty output");
                    return None;
                }
                Err(e) => {
                    crate::logger::warn("Lyrics", &format!("kugou-decrypt: deflate failed: {}", e));
                    return None;
                }
            }
        }
    };
    let skip = if inflated.starts_with(&[0xEF, 0xBB, 0xBF]) { 3 } else { 1 };
    crate::logger::info("Lyrics", &format!("kugou-decrypt: bom_skip={} inflated_len={}", skip, inflated.len()));
    if inflated.len() <= skip {
        crate::logger::warn("Lyrics", &format!("kugou-decrypt: inflated too short after skip({}) ({})", skip, inflated.len()));
        return None;
    }
    match String::from_utf8(inflated[skip..].to_vec()) {
        Ok(s) => {
            crate::logger::info("Lyrics", &format!("kugou-decrypt: ok, text_len={}", s.len()));
            Some(s)
        }
        Err(e) => {
            crate::logger::warn("Lyrics", &format!("kugou-decrypt: utf8 failed: {}", e));
            None
        }
    }
}

fn parse_krc_to_lyric_lines(krc: &str) -> Vec<LyricLine> {
    static KRC_LINE_RE: OnceLock<Option<Regex>> = OnceLock::new();
    static KRC_TAG_RE: OnceLock<Option<Regex>> = OnceLock::new();
    let Some(line_re) = KRC_LINE_RE
        .get_or_init(|| Regex::new(r"^\[(\d+),\d+\]").ok())
        .as_ref() else { return Vec::new(); };
    let Some(tag_re) = KRC_TAG_RE
        .get_or_init(|| Regex::new(r"<\d+,\d+,\d+>").ok())
        .as_ref() else { return Vec::new(); };
    let mut lines = Vec::new();
    for raw in krc.lines() {
        let raw = raw.trim();
        if !raw.starts_with('[') { continue; }
        if !raw.chars().nth(1).map(|c| c.is_ascii_digit()).unwrap_or(false) { continue; }
        let start_ms = match line_re.captures(raw)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse::<i64>().ok()) {
            Some(t) => t,
            None => continue,
        };
        let content_start = match raw.find(']') { Some(i) => i + 1, None => continue };
        let text = tag_re.replace_all(&raw[content_start..], "").trim().to_string();
        if !text.is_empty() && !is_meta_lyric_text(&text) {
            lines.push(LyricLine { time_ms: start_ms, text });
        }
    }
    lines.sort_by_key(|l| l.time_ms);
    lines
}

fn fetch_kugou_lyrics(title: &str, artist: &str) -> Option<Vec<LyricLine>> {
    let keyword = if artist.trim().is_empty() {
        title.trim().to_string()
    } else {
        format!("{} {}", title.trim(), artist.trim())
    };
    if keyword.is_empty() { return None; }
    crate::logger::info("Lyrics", &format!("\nkugou: title='{}' artist='{}'", title, artist));
    let (hash, duration_ms) = kugou_search_song(title, artist).unwrap_or_default();
    let (id, accesskey) = match kugou_search_lyric(&keyword, duration_ms, &hash) {
        Some(p) => p,
        None => { crate::logger::warn("Lyrics", "kugou: lyric search failed"); return None; }
    };
    let encrypted = match kugou_download_krc(&id, &accesskey) {
        Some(c) => c,
        None => { crate::logger::warn("Lyrics", "kugou: download failed"); return None; }
    };
    let krc_text = match kugou_decrypt_krc(&encrypted) {
        Some(t) => t,
        None => { crate::logger::warn("Lyrics", "kugou: decrypt failed"); return None; }
    };
    let lines = parse_krc_to_lyric_lines(&krc_text);
    if lines.is_empty() {
        crate::logger::warn("Lyrics", "kugou: parse returned empty");
        return None;
    }
    crate::logger::info("Lyrics", &format!("kugou: done lines={}", lines.len()));
    Some(lines)
}

/// 通过 lyricify-lyrics-provider 统一接口获取歌词（自动检测播放器，多源 fallback）
fn fetch_lyrics_by_rust_api(
    title: &str,
    artist: &str,
    album_title: &str,
    album_artist: &str,
    duration_ms: i64,
    gen_ref: &std::sync::Arc<std::sync::atomic::AtomicU64>,
    gen: u64,
) -> Option<Vec<LyricLine>> {
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
    let album_opt: Option<&str> = if album_title.trim().is_empty() { None } else { Some(album_title) };
    let album_artist_opt: Option<&str> = if album_artist.trim().is_empty() { None } else { Some(album_artist) };
    let duration_ms_i32: i32 = duration_ms.clamp(0, i64::from(i32::MAX)) as i32;
    rt.block_on(async {
        crate::logger::info("Lyrics", &format!(
            "\nrust-api: title='{}' artist='{}' album artist='{}' duration_ms={}", title, artist, album_artist, duration_ms
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

        match smtc_lyrics::get_lyrics(title, artist_opt, album_opt, album_artist_opt, duration_ms_i32).await {
            Ok((player, data)) => {
                let meta = data.track_metadata.as_ref();
                crate::logger::info("Lyrics", &format!(
                    "rust-api: raw from='{}' lines={} file={:?} meta={:?}",
                    player.display_name(),
                    data.lines.len(),
                    data.file.as_ref().map(|f| format!("{:?}/{:?}", f.lyrics_type, f.sync_type)),
                    meta
                ));
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
            smtc_lyrics::MusicPlayer::Kugou,
        ];
        for player in &fallback_players {
            if gen_ref.load(std::sync::atomic::Ordering::Relaxed) != gen {
                crate::logger::warn("Lyrics", &format!("rust-api: fallback abort gen={} (stale)", gen));
                return None;
            }
            crate::logger::info("Lyrics", &format!(
                "rust-api: fallback trying '{}'", player.display_name()
            ));
            match smtc_lyrics::get_lyrics_with_player(player, title, artist_opt, album_opt, album_artist_opt, duration_ms_i32).await {
                Ok(data) => {
                    let meta = data.track_metadata.as_ref();
                    crate::logger::info("Lyrics", &format!(
                        "rust-api: raw from='{}' lines={} file={:?} meta={:?}",
                        player.display_name(),
                        data.lines.len(),
                        data.file.as_ref().map(|f| format!("{:?}/{:?}", f.lyrics_type, f.sync_type)),
                        meta
                    ));
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
    album_title: &str,
    album_artist: &str,
    duration_ms: i64,
    genre: &str,
    ncm_genre_hit_enabled: bool,
    rust_api_enabled: bool,
    api_search_enabled: bool,
    gen_ref: std::sync::Arc<std::sync::atomic::AtomicU64>,
    gen: u64,
) -> Option<Vec<LyricLine>> {
    crate::logger::info("Lyrics", &format!(
        "\nlyric-fetch: song='{}' artist='{}' album='{}' album_artist='{}' duration_ms={} genre='{}'",
        title, artist, album_title, album_artist, duration_ms, genre
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
        if let Some(lines) = fetch_lyrics_by_rust_api(title, artist, album_title, album_artist, duration_ms, &gen_ref, gen) {
            return Some(lines);
        }
        crate::logger::warn("Lyrics", "rust-api: failed, fallback to api-search");
    } else {
        crate::logger::info("Lyrics", "rust-api: disabled by setting");
    }

    // --- 第三优先：Netease API 搜索 ---
    if gen_ref.load(std::sync::atomic::Ordering::Relaxed) != gen {
        crate::logger::warn("Lyrics", &format!("fetch-parallel: abort before api-search gen={} (stale)", gen));
        return None;
    }
    if !api_search_enabled {
        crate::logger::info("Lyrics", "\napi-search: disabled by setting");
        return None;
    }

    if let Some(search_song_id) = fetch_netease_song_id_by_search(title, artist) {
        crate::logger::info("Lyrics", &format!(
            "\napi-search: fetch lyrics song_id={} title='{}' artist='{}'",
            search_song_id, title, artist
        ));
        if let Some(lines) = fetch_netease_lyrics_by_song_id(search_song_id, "api-search") {
            return Some(lines);
        }
        crate::logger::warn("Lyrics", "api-search: lyrics not found, trying kugou");
    } else {
        crate::logger::warn("Lyrics", "api-search: song not found, trying kugou");
    }

    // --- 第四优先：酷狗 API 搜索 ---
    if gen_ref.load(std::sync::atomic::Ordering::Relaxed) != gen {
        crate::logger::warn("Lyrics", &format!("fetch-parallel: abort before kugou gen={} (stale)", gen));
        return None;
    }
    if let Some(lines) = fetch_kugou_lyrics(title, artist) {
        return Some(lines);
    }
    crate::logger::warn("Lyrics", "kugou: all sources exhausted");
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

