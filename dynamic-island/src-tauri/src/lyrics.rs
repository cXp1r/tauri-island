use lyricify_lyrics_provider::models::LyricsData;
use regex::Regex;
use std::sync::OnceLock;

#[derive(Clone, Debug, serde::Serialize)]
pub(crate) struct LyricToken {
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
}

#[derive(Clone, Debug)]
pub(crate) struct LyricLine {
    pub time_ms: i64,
    pub end_time_ms: i64,
    pub text: String,
    pub tokens: Vec<LyricToken>,
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


/// 通过 lyricify-lyrics-provider 统一接口获取歌词（自动检测播放器，多源 fallback）
fn fetch_lyrics_by_rust_api(
    title: &str,
    artist: &str,
    album_title: &str,
    album_artist: &str,
    duration_ms: i64,
    gen_ref: &std::sync::Arc<std::sync::atomic::AtomicU64>,
    gen: u64,
) -> Option<LyricsData> {
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
    let duration_ms_u32: u32 = duration_ms.clamp(0, i64::from(u32::MAX)) as u32;
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

        match smtc_lyrics::get_lyrics(title, artist_opt, album_opt, album_artist_opt, duration_ms_u32).await {
            Ok((player, data)) => {
                let meta = data.track_metadata.as_ref();
                crate::logger::info("Lyrics", &format!(
                    "rust-api: raw from='{}' lines={} file={:?} meta={:?}",
                    player.display_name(),
                    data.lines.len(),
                    data.file.as_ref().map(|f| format!("{:?}/{:?}", f.lyrics_type, f.sync_type)),
                    meta
                ));
                return Some(data);
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
            match smtc_lyrics::get_lyrics_with_player(player, title, artist_opt, album_opt, album_artist_opt, duration_ms_u32).await {
                Ok(data) => {
                    let meta = data.track_metadata.as_ref();
                    crate::logger::info("Lyrics", &format!(
                        "rust-api: raw from='{}' lines={} file={:?} meta={:?}",
                        player.display_name(),
                        data.lines.len(),
                        data.file.as_ref().map(|f| format!("{:?}/{:?}", f.lyrics_type, f.sync_type)),
                        meta
                    ));
                    return Some(data);
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

fn line_end_ms(data: &LyricsData, sorted_indices: &[usize], sorted_pos: usize, start_ms: i64) -> i64 {
    let line = &data.lines[sorted_indices[sorted_pos]];
    if line.duration > 0 {
        return start_ms + i64::from(line.duration);
    }
    if sorted_pos + 1 < sorted_indices.len() {
        return i64::from(data.lines[sorted_indices[sorted_pos + 1]].start_time);
    }
    start_ms + 4000
}

fn tokens_from_line(line: &lyricify_lyrics_provider::models::LineInfo, line_start_ms: i64, line_end_ms: i64) -> Vec<LyricToken> {
    if !line.syllables.is_empty() {
        let mut tokens = Vec::with_capacity(line.syllables.len());
        for (i, s) in line.syllables.iter().enumerate() {
            if s.text.is_empty() {
                continue;
            }
            let start = line_start_ms + i64::from(s.start_time);
            let mut end = if s.duration > 0 {
                start + i64::from(s.duration)
            } else if i + 1 < line.syllables.len() {
                line_start_ms + i64::from(line.syllables[i + 1].start_time)
            } else {
                line_end_ms
            };
            if end < start {
                end = start;
            }
            if end > line_end_ms {
                end = line_end_ms;
            }
            tokens.push(LyricToken {
                text: s.text.clone(),
                start_ms: start,
                end_ms: end,
            });
        }
        if !tokens.is_empty() {
            return tokens;
        }
    }

    let parts = tokenize(&line.text);
    if parts.is_empty() {
        return Vec::new();
    }
    let duration = (line_end_ms - line_start_ms).max(1);
    let n = parts.len() as i64;
    parts
        .into_iter()
        .enumerate()
        .map(|(i, text)| {
            let i = i as i64;
            LyricToken {
                text,
                start_ms: line_start_ms + duration * i / n,
                end_ms: line_start_ms + duration * (i + 1) / n,
            }
        })
        .collect()
}

fn lyrics_data_to_lyric_lines(data: &LyricsData) -> Vec<LyricLine> {
    let mut indices: Vec<usize> = data
        .lines
        .iter()
        .enumerate()
        .filter_map(|(i, l)| if l.text.trim().is_empty() { None } else { Some(i) })
        .collect();
    indices.sort_by_key(|&i| data.lines[i].start_time);

    let mut out = Vec::with_capacity(indices.len());
    for (pos, idx) in indices.iter().enumerate() {
        let line = &data.lines[*idx];
        let start_ms = i64::from(line.start_time);
        let end_ms = line_end_ms(data, &indices, pos, start_ms);
        out.push(LyricLine {
            time_ms: start_ms,
            end_time_ms: end_ms,
            text: line.text.clone(),
            tokens: tokens_from_line(line, start_ms, end_ms),
        });
    }
    out
}

pub(crate) fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut latin_buf = String::new();
    for ch in text.chars() {
        if is_cjk_char(ch) {
            if !latin_buf.is_empty() {
                tokens.push(std::mem::take(&mut latin_buf));
            }
            tokens.push(ch.to_string());
        } else if ch.is_whitespace() {
            if !latin_buf.is_empty() {
                tokens.push(std::mem::take(&mut latin_buf));
            }
        } else {
            latin_buf.push(ch);
        }
    }
    if !latin_buf.is_empty() {
        tokens.push(latin_buf);
    }
    tokens
}

fn is_cjk_char(ch: char) -> bool {
    let c = ch as u32;
    matches!(
        c,
        0x4E00..=0x9FFF
            | 0x3400..=0x4DBF
            | 0x20000..=0x2A6DF
            | 0x2A700..=0x2B73F
            | 0x2B740..=0x2B81F
            | 0x2B820..=0x2CEAF
            | 0x3040..=0x309F
            | 0x30A0..=0x30FF
            | 0xAC00..=0xD7AF
            | 0x3000..=0x303F
    )
}


pub(crate) fn fetch_lyrics_parallel(
    title: &str,
    artist: &str,
    album_title: &str,
    album_artist: &str,
    duration_ms: i64,
    genre: &str,
    _ncm_genre_hit_enabled: bool,
    rust_api_enabled: bool,
    _api_search_enabled: bool,
    gen_ref: std::sync::Arc<std::sync::atomic::AtomicU64>,
    gen: u64,
) -> Option<Vec<LyricLine>> {
    crate::logger::info("Lyrics", &format!(
        "\nlyric-fetch: song='{}' artist='{}' album='{}' album_artist='{}' duration_ms={} genre='{}'",
        title, artist, album_title, album_artist, duration_ms, genre
    ));

    /*if ncm_genre_hit_enabled {
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
    }*/

    // --- 第二优先：Rust API（lyricify-lyrics-helper 库） ---
    if rust_api_enabled {
        crate::logger::info("Lyrics", &format!("rust-api: enabled, title='{}' artist='{}'", title, artist));
        if let Some(data) = fetch_lyrics_by_rust_api(title, artist, album_title, album_artist, duration_ms, &gen_ref, gen) {
            return Some(lyrics_data_to_lyric_lines(&data));
        }
        crate::logger::warn("Lyrics", "rust-api: failed, fallback to api-search");
    } else {
        crate::logger::info("Lyrics", "rust-api: disabled by setting");
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

