use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Clone, Debug, Default)]
pub(crate) struct MediaInfo {
    pub title: String,
    pub artist: String,
    pub album_title: String,
    pub album_artist: String,
    pub duration_ms: i64,
    pub genre: String,
    pub seekable: bool,
}

#[derive(Clone, Default)]
struct SmtcWhitelist {
    enabled: bool,
    app_ids: Vec<String>,
}

static SMTC_WHITELIST: OnceLock<Mutex<SmtcWhitelist>> = OnceLock::new();

pub(crate) fn update_smtc_whitelist(enabled: bool, app_ids: Vec<String>) {
    let normalized: Vec<String> = app_ids
        .into_iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    let mut guard = SMTC_WHITELIST
        .get_or_init(|| Mutex::new(SmtcWhitelist::default()))
        .lock()
        .unwrap();
    *guard = SmtcWhitelist {
        enabled,
        app_ids: normalized,
    };
}

fn get_smtc_whitelist() -> SmtcWhitelist {
    SMTC_WHITELIST
        .get_or_init(|| Mutex::new(SmtcWhitelist::default()))
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_default()
}

pub(crate) fn is_preferred_music_app(app_id: &str) -> bool {
    let id = app_id.to_ascii_lowercase();
    [
        // —— 国内音乐平台 ——
        "cloudmusic",   // 网易云音乐
        "netease",      // 网易云音乐（备用匹配）
        "music.163",    // 网易云音乐（UWP / 网页版 PWA）
        "qqmusic",      // QQ 音乐
        "kugou",        // 酷狗音乐
        "kuwo",         // 酷我音乐
        "qishui",       // 汽水音乐
        "\u{6c7d}\u{6c34}\u{97f3}\u{4e50}", // 汽水音乐（中文名）
        "migu",         // 咪咕音乐
        // —— 国际音乐平台 ——
        "spotify",      // Spotify
        "itunes",       // Apple Music / iTunes
        "applemusic",   // Apple Music
        "apple.music",  // Apple Music（UWP）
        "tidal",        // TIDAL
        "deezer",       // Deezer
        "amazonmusic",  // Amazon Music
        "amazon music", // Amazon Music（备用）
        // —— 本地播放器 ——
        "foobar",       // foobar2000 / foobox
        "vlc",          // VLC media player
        "aimp",         // AIMP
        "musicbee",     // MusicBee
        "winamp",       // Winamp
        "wacup",        // WACUP（Winamp 社区版）
        "mediamonkey",  // MediaMonkey
        "dopamine",     // Dopamine
        // —— Windows 内置 ——
        "zunemusic",    // Groove 音乐 / Windows Media Player（新版）
        "microsoft.windows.media", // Windows Media Player
        // —— 第三方开源 / 小众 ——
        "lx-music",     // 洛雪音乐
        "lx_music",     // 洛雪音乐（备用）
        "listen1",      // Listen 1
        "yesplaymusic", // YesPlayMusic
        "harmonoid",    // Harmonoid
        "cider",        // Cider（Apple Music 第三方客户端）
        "plexamp",      // Plexamp
        "tauon",        // Tauon Music Box
    ]
    .iter()
    .any(|k| id.contains(k))
}

/// 判断是否为浏览器或视频播放器（非音乐应用）
fn is_browser_or_video_app(app_id: &str) -> bool {
    let id = app_id.to_ascii_lowercase();
    [
        "chrome",       // Google Chrome
        "msedge",       // Microsoft Edge
        "firefox",      // Firefox
        "opera",        // Opera
        "brave",        // Brave
        "vivaldi",      // Vivaldi
        "potplayer",    // PotPlayer
        "mpc-hc",       // MPC-HC
        "mpc-be",       // MPC-BE
        "kmplayer",     // KMPlayer
    ]
    .iter()
    .any(|k| id.contains(k))
}

#[derive(Default)]
struct CloudMusicWindowContext {
    titles: Vec<String>,
    pid_cache: HashMap<u32, bool>,
}

fn is_generic_cloudmusic_title(title: &str) -> bool {
    let t = title.trim();
    t.is_empty()
        || t == "网易云音乐"
        || t == "Netease Cloud Music"
        || t == "CloudMusic"
        || t == "cloudmusic"
}

fn split_track_artist(title: &str) -> Option<(String, String)> {
    for sep in [" - ", " — ", " – ", " / "] {
        if let Some((left, right)) = title.split_once(sep) {
            let track = left.trim().to_string();
            let artist = right.trim().to_string();
            if !track.is_empty() && !artist.is_empty() {
                return Some((track, artist));
            }
        }
    }
    None
}

fn pick_best_cloudmusic_title(titles: &[String]) -> Option<String> {
    let mut best: Option<(i32, String)> = None;
    for raw in titles {
        let t = raw.trim();
        if t.is_empty() {
            continue;
        }
        let mut score = 0;
        if !is_generic_cloudmusic_title(t) {
            score += 30;
        }
        if split_track_artist(t).is_some() {
            score += 40;
        }
        if !t.contains("MediaPlayer") {
            score += 20;
        }
        if (2..=80).contains(&t.chars().count()) {
            score += 10;
        }

        let should_replace = best.as_ref().map(|(s, _)| score > *s).unwrap_or(true);
        if should_replace {
            best = Some((score, t.to_string()));
        }
    }
    best.and_then(|(_, t)| if is_generic_cloudmusic_title(&t) { None } else { Some(t) })
}

fn is_cloudmusic_process(pid: u32) -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::core::PWSTR;

    unsafe {
        let handle = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(h) => h,
            Err(_) => return false,
        };

        let mut buf = vec![0u16; 260];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buf.as_mut_ptr()),
            &mut size,
        )
        .is_ok();
        let _ = CloseHandle(handle);

        if !ok || size == 0 {
            return false;
        }

        let full_path = String::from_utf16_lossy(&buf[..size as usize]).to_ascii_lowercase();
        full_path.ends_with("\\cloudmusic.exe")
            || full_path.ends_with("/cloudmusic.exe")
            || full_path.contains("cloudmusic")
            || full_path.contains("netease")
    }
}

unsafe extern "system" fn enum_cloudmusic_windows(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
    let ctx = &mut *(lparam.0 as *mut CloudMusicWindowContext);

    let len = GetWindowTextLengthW(hwnd);
    if len <= 0 {
        return windows::core::BOOL(1);
    }

    let mut buf = vec![0u16; len as usize + 1];
    let copied = GetWindowTextW(hwnd, &mut buf);
    if copied <= 0 {
        return windows::core::BOOL(1);
    }

    let title = String::from_utf16_lossy(&buf[..copied as usize]).trim().to_string();
    if title.is_empty() {
        return windows::core::BOOL(1);
    }

    let mut pid = 0u32;
    let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == 0 {
        return windows::core::BOOL(1);
    }

    let is_cloud = if let Some(v) = ctx.pid_cache.get(&pid) {
        *v
    } else {
        let v = is_cloudmusic_process(pid);
        ctx.pid_cache.insert(pid, v);
        v
    };

    if is_cloud {
        ctx.titles.push(title);
    }

    windows::core::BOOL(1)
}

fn parse_cloudmusic_window_title(raw: &str) -> Option<MediaInfo> {
    let mut title = raw.trim().to_string();
    if title.is_empty() {
        return None;
    }

    for suffix in [" - 网易云音乐", " - Netease Cloud Music"] {
        if title.ends_with(suffix) {
            title = title.trim_end_matches(suffix).trim().to_string();
        }
    }

    if let Some((track_title, mut artist)) = split_track_artist(&title) {
        if artist == "网易云音乐" || artist == "Netease Cloud Music" {
            artist.clear();
        }
        if !track_title.trim().is_empty() {
            return Some(MediaInfo {
                title: track_title.trim().to_string(),
                artist,
                album_title: String::new(),
                album_artist: String::new(),
                duration_ms: 0,
                genre: String::new(),
                seekable: false,
            });
        }
    }

    Some(MediaInfo {
        title,
        artist: String::new(),
        album_title: String::new(),
        album_artist: String::new(),
        duration_ms: 0,
        genre: String::new(),
        seekable: false,
    })
}

fn get_cloudmusic_fallback_info() -> Option<(MediaInfo, i64, bool)> {
    let mut ctx = CloudMusicWindowContext::default();

    unsafe {
        let _ = EnumWindows(
            Some(enum_cloudmusic_windows),
            LPARAM((&mut ctx as *mut CloudMusicWindowContext) as isize),
        );
    }

    let title = pick_best_cloudmusic_title(&ctx.titles)?;

    let media = parse_cloudmusic_window_title(&title)?;
    Some((media, 0, true))
}

/// 从 SMTC 媒体属性中提取封面图片并编码为 base64
fn extract_thumbnail(
    props: &windows::Media::Control::GlobalSystemMediaTransportControlsSessionMediaProperties,
) -> Option<String> {
    use windows::Storage::Streams::DataReader;

    let thumbnail_ref = props.Thumbnail().ok()?;
    let stream = thumbnail_ref.OpenReadAsync().ok()?.get().ok()?;
    let size = stream.Size().ok()? as u32;
    if size == 0 || size > 10_000_000 {
        // 无效或过大，跳过
        return None;
    }
    let input_stream = stream.GetInputStreamAt(0).ok()?;
    let reader = DataReader::CreateDataReader(&input_stream).ok()?;
    reader.LoadAsync(size).ok()?.get().ok()?;
    let mut buf = vec![0u8; size as usize];
    reader.ReadBytes(&mut buf).ok()?;
    let _ = reader.Close();
    let _ = stream.Close();

    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&buf);
    let mime = if buf.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "image/png"
    } else if buf.starts_with(b"RIFF") && buf.get(8..12) == Some(b"WEBP") {
        "image/webp"
    } else if buf.starts_with(b"GIF8") {
        "image/gif"
    } else if buf.starts_with(&[0x42, 0x4D]) {
        "image/bmp"
    } else {
        "image/jpeg"
    };
    Some(format!("data:{};base64,{}", mime, b64))
}

pub(crate) fn send_media_virtual_key(vk: u8) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{keybd_event, KEYEVENTF_KEYUP};
    unsafe {
        keybd_event(vk, 0, Default::default(), 0);
        keybd_event(vk, 0, KEYEVENTF_KEYUP, 0);
    }
}

fn read_smtc_session_info(
    session: &windows::Media::Control::GlobalSystemMediaTransportControlsSession,
) -> Option<(MediaInfo, i64, bool)> {
    use windows::Media::Control::GlobalSystemMediaTransportControlsSessionPlaybackStatus;

    let playback_info = session.GetPlaybackInfo();
    if let Err(ref e) = playback_info {
        crate::logger::warn("SMTC", &format!("GetPlaybackInfo failed: {:?}", e));
    }
    let seekable = playback_info.as_ref().ok()
        .and_then(|p| p.Controls().ok())
        .and_then(|c| c.IsPlaybackPositionEnabled().ok())
        .unwrap_or(false);
    let playback_status = playback_info.ok()
        .and_then(|p| {
            p.PlaybackStatus().map_err(|e| {
                crate::logger::warn("SMTC", &format!("PlaybackStatus failed: {:?}", e));
            }).ok()
        });
    if matches!(playback_status, Some(GlobalSystemMediaTransportControlsSessionPlaybackStatus::Closed)) {
        return None;
    }
    let is_playing = matches!(
        playback_status,
        Some(GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing)
    );

    let timeline = match session.GetTimelineProperties() {
        Ok(t) => Some(t),
        Err(e) => {
            crate::logger::warn("SMTC", &format!("GetTimelineProperties failed: {:?}", e));
            None
        }
    };

    let position_ms = if let Some(ref t) = timeline {
        let reported_ms = match t.Position() {
            Ok(p) => p.Duration / 10_000,
            Err(e) => {
                crate::logger::warn("SMTC", &format!("Position() failed: {:?}", e));
                0
            }
        };
        // SMTC Position() is a snapshot at LastUpdatedTime; interpolate to now
        if is_playing {
            if let Ok(last_updated) = t.LastUpdatedTime() {
                let now_ticks = {
                    use std::time::{SystemTime, UNIX_EPOCH};
                    let unix_ms = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as i64;
                    // Windows FILETIME epoch offset (100ns ticks from 1601 to 1970)
                    unix_ms * 10_000 + 116_444_736_000_000_000i64
                };
                let elapsed_ms = ((now_ticks - last_updated.UniversalTime).max(0)) / 10_000;
                reported_ms + elapsed_ms
            } else {
                reported_ms
            }
        } else {
            reported_ms
        }
    } else {
        0
    };

    const MAX_DURATION_MS: i64 = 24 * 3600 * 1000; // 24小时，用于 position 校验上限

    let raw_duration_ms = match timeline.as_ref() {
        Some(t) => match t.EndTime() {
            Ok(p) => p.Duration / 10_000,
            Err(e) => {
                crate::logger::warn("SMTC", &format!("EndTime() failed: {:?}", e));
                0
            }
        },
        None => 0,
    };
    // duration_ms 不做截断，保留原始值，便于观察各播放器上报情况（dump_smtc_session 负责详细日志）
    let duration_ms = raw_duration_ms;

    let position_ms = {
        let max_pos = if duration_ms > 0 && duration_ms <= MAX_DURATION_MS {
            duration_ms
        } else {
            MAX_DURATION_MS
        };
        if position_ms < 0 || position_ms > max_pos { 0 } else { position_ms }
    };

    let media_props_result = session.TryGetMediaPropertiesAsync()
        .map_err(|e| { crate::logger::warn("SMTC", &format!("TryGetMediaPropertiesAsync failed: {:?}", e)); })
        .ok()
        .and_then(|op| op.get().map_err(|e| { crate::logger::warn("SMTC", &format!("MediaProperties.get() failed: {:?}", e)); }).ok());
    let (title, artist, album_title, album_artist, genre) = match media_props_result {
        Some(props) => {
            let title = props.Title().ok().map(|v| v.to_string_lossy()).unwrap_or_default();
            let artist = props.Artist().ok().map(|v| v.to_string_lossy()).unwrap_or_default();
            let album_title = props.AlbumTitle().ok().map(|v| v.to_string_lossy()).unwrap_or_default();
            let album_artist = props.AlbumArtist().ok().map(|v| v.to_string_lossy()).unwrap_or_default();
            let genre = props
                .Genres()
                .ok()
                .and_then(|genres| {
                    let size = genres.Size().ok()?;
                    let mut values = Vec::new();
                    for i in 0..size {
                        if let Ok(v) = genres.GetAt(i) {
                            let g = v.to_string_lossy();
                            let g = g.trim();
                            if !g.is_empty() {
                                values.push(g.to_string());
                            }
                        }
                    }
                    if values.is_empty() {
                        None
                    } else {
                        Some(values.join(" / "))
                    }
                })
                .unwrap_or_default();
            (title, artist, album_title, album_artist, genre)
        }
        None => (String::new(), String::new(), String::new(), String::new(), String::new()),
    };

    Some((MediaInfo { title, artist, album_title, album_artist, duration_ms, genre, seekable }, position_ms, is_playing))
}

/// 将当前 SMTC 会话的所有可读字段打印到日志，用于排查不同播放器行为差异
pub(crate) fn dump_smtc_session(app_id: &str) {
    use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager as MediaSessionManager;
    use windows::Media::Control::GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus;
    use windows::Media::MediaPlaybackType as PlaybackType;
    use windows::Media::MediaPlaybackAutoRepeatMode;

    let Ok(manager) = MediaSessionManager::RequestAsync().ok().and_then(|a| a.get().ok()).ok_or(()) else {
        crate::logger::warn("SMTC-Dump", "RequestAsync failed");
        return;
    };

    // 找到对应 app_id 的 session
    let sessions = match manager.GetSessions().ok() {
        Some(s) => s,
        None => { crate::logger::warn("SMTC-Dump", "GetSessions failed"); return; }
    };
    let size = sessions.Size().ok().unwrap_or(0);
    for i in 0..size {
        let session = match sessions.GetAt(i) { Ok(s) => s, Err(_) => continue };
        let sid = session.SourceAppUserModelId().ok()
            .map(|s| s.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if !app_id.is_empty() && !sid.to_ascii_lowercase().contains(&app_id.to_ascii_lowercase()) {
            continue;
        }

        let mut lines = vec![format!("=== SMTC dump app_id='{}' ===", sid)];

        // --- MediaProperties ---
        if let Some(props) = session.TryGetMediaPropertiesAsync().ok().and_then(|op| op.get().ok()) {
            lines.push(format!("  Title:          {:?}", props.Title().ok().map(|v| v.to_string_lossy())));
            lines.push(format!("  Artist:         {:?}", props.Artist().ok().map(|v| v.to_string_lossy())));
            lines.push(format!("  AlbumTitle:     {:?}", props.AlbumTitle().ok().map(|v| v.to_string_lossy())));
            lines.push(format!("  AlbumArtist:    {:?}", props.AlbumArtist().ok().map(|v| v.to_string_lossy())));
            lines.push(format!("  Subtitle:       {:?}", props.Subtitle().ok().map(|v| v.to_string_lossy())));
            lines.push(format!("  TrackNumber:    {:?}", props.TrackNumber().ok()));
            lines.push(format!("  AlbumTrackCnt:  {:?}", props.AlbumTrackCount().ok()));
            let genres: Vec<String> = props.Genres().ok()
                .map(|g| {
                    let n = g.Size().ok().unwrap_or(0);
                    (0..n).filter_map(|i| g.GetAt(i).ok().map(|v| v.to_string_lossy()))
                        .collect()
                })
                .unwrap_or_default();
            lines.push(format!("  Genres:         {:?}", genres));
            let has_thumb = props.Thumbnail().is_ok();
            lines.push(format!("  Thumbnail:      present={}", has_thumb));
        } else {
            lines.push("  MediaProperties: unavailable".to_string());
        }

        // --- PlaybackInfo ---
        if let Ok(pb) = session.GetPlaybackInfo() {
            let status = pb.PlaybackStatus().ok().map(|s| match s {
                PlaybackStatus::Playing  => "Playing",
                PlaybackStatus::Paused   => "Paused",
                PlaybackStatus::Stopped  => "Stopped",
                PlaybackStatus::Changing => "Changing",
                PlaybackStatus::Closed   => "Closed",
                PlaybackStatus::Opened   => "Opened",
                _ => "Unknown",
            });
            lines.push(format!("  PlaybackStatus: {:?}", status));
            let ptype = pb.PlaybackType().ok().map(|t| match t.Value() {
                Ok(v) => match v {
                    PlaybackType::Unknown => "Unknown",
                    PlaybackType::Music   => "Music",
                    PlaybackType::Video   => "Video",
                    PlaybackType::Image   => "Image",
                    _ => "Other",
                },
                Err(_) => "null",
            });
            lines.push(format!("  PlaybackType:   {:?}", ptype));
            let rate = pb.PlaybackRate().ok().map(|r| r.Value().ok());
            lines.push(format!("  PlaybackRate:   {:?}", rate));
            let shuffle = pb.IsShuffleActive().ok().map(|r| r.Value().ok());
            lines.push(format!("  IsShuffleActive:{:?}", shuffle));
            let repeat = pb.AutoRepeatMode().ok().map(|r| r.Value().ok().map(|v| match v {
                MediaPlaybackAutoRepeatMode::None  => "None",
                MediaPlaybackAutoRepeatMode::Track => "Track",
                MediaPlaybackAutoRepeatMode::List  => "List",
                _ => "Other",
            }));
            lines.push(format!("  AutoRepeatMode: {:?}", repeat));
        }

        // --- TimelineProperties ---
        if let Ok(tl) = session.GetTimelineProperties() {
            let ticks_to_ms = |t: i64| t / 10_000;
            lines.push(format!("  StartTime:      {}ms", tl.StartTime().ok().map(|t| ticks_to_ms(t.Duration)).unwrap_or(-1)));
            lines.push(format!("  EndTime:        {}ms", tl.EndTime().ok().map(|t| ticks_to_ms(t.Duration)).unwrap_or(-1)));
            lines.push(format!("  MinSeekTime:    {}ms", tl.MinSeekTime().ok().map(|t| ticks_to_ms(t.Duration)).unwrap_or(-1)));
            lines.push(format!("  MaxSeekTime:    {}ms", tl.MaxSeekTime().ok().map(|t| ticks_to_ms(t.Duration)).unwrap_or(-1)));
            lines.push(format!("  Position:       {}ms", tl.Position().ok().map(|t| ticks_to_ms(t.Duration)).unwrap_or(-1)));
            lines.push(format!("  LastUpdatedTime:{}", tl.LastUpdatedTime().ok().map(|t| t.UniversalTime).unwrap_or(-1)));
        }

        crate::logger::info("SMTC-Dump", &lines.join("\n"));
        break; // 只打第一个匹配的 session
    }
}

pub(crate) fn select_best_smtc_session(
) -> Option<(
    windows::Media::Control::GlobalSystemMediaTransportControlsSession,
    MediaInfo,
    i64,
    bool,
    String,
)> {
    use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager as MediaSessionManager;

    let session_manager = MediaSessionManager::RequestAsync().ok()?.get().ok()?;
    let current_app_id = session_manager
        .GetCurrentSession()
        .ok()
        .and_then(|s| s.SourceAppUserModelId().ok())
        .map(|s| s.to_string_lossy().to_ascii_lowercase());

    let sessions = session_manager.GetSessions().ok()?;
    let size = sessions.Size().ok()?;

    let mut best: Option<(
        i32,
        windows::Media::Control::GlobalSystemMediaTransportControlsSession,
        MediaInfo,
        i64,
        bool,
        String,
    )> = None;

    let whitelist = get_smtc_whitelist();

    for i in 0..size {
        let session = match sessions.GetAt(i) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let (media, position_ms, is_playing) = match read_smtc_session_info(&session) {
            Some(v) => v,
            None => continue,
        };

        let app_id = session
            .SourceAppUserModelId()
            .ok()
            .map(|s| s.to_string_lossy())
            .unwrap_or_default();
        let app_id_lc = app_id.to_ascii_lowercase();

        if whitelist.enabled {
            let allowed = !whitelist.app_ids.is_empty()
                && whitelist.app_ids.iter().any(|id| app_id_lc.contains(id));
            if !allowed {
                continue;
            }
        }

        let has_meta = !media.title.trim().is_empty() || !media.artist.trim().is_empty();
        if !has_meta && !is_playing {
            continue;
        }

        let mut score = 0;
        if is_playing {
            score += 100;
        }
        if has_meta {
            score += 20;
        }
        if is_preferred_music_app(&app_id_lc) {
            score += 40;
        }
        if is_browser_or_video_app(&app_id_lc) {
            score -= 80; // 大幅降低浏览器/视频应用的优先级
        }
        if current_app_id
            .as_deref()
            .map(|current| current == app_id_lc)
            .unwrap_or(false)
        {
            score += 10;
        }

        let should_replace = best.as_ref().map(|(s, ..)| score > *s).unwrap_or(true);
        if should_replace {
            best = Some((score, session, media, position_ms, is_playing, app_id_lc));
        }
    }

    best.map(|(_, session, media, position_ms, is_playing, app_id)| {
        (session, media, position_ms, is_playing, app_id)
    })
}

pub(crate) fn get_smtc_session() -> Option<windows::Media::Control::GlobalSystemMediaTransportControlsSession> {
    select_best_smtc_session().map(|(session, _, _, _, _)| session)
}

#[tauri::command]
pub fn media_play_pause() {
    if let Some(session) = get_smtc_session() {
        let _ = session.TryTogglePlayPauseAsync();
    } else {
        use windows::Win32::UI::Input::KeyboardAndMouse::VK_MEDIA_PLAY_PAUSE;
        send_media_virtual_key(VK_MEDIA_PLAY_PAUSE.0 as u8);
    }
}

#[tauri::command]
pub fn media_next() {
    if let Some(session) = get_smtc_session() {
        let _ = session.TrySkipNextAsync();
    } else {
        use windows::Win32::UI::Input::KeyboardAndMouse::VK_MEDIA_NEXT_TRACK;
        send_media_virtual_key(VK_MEDIA_NEXT_TRACK.0 as u8);
    }
}

#[tauri::command]
pub fn media_prev() {
    if let Some(session) = get_smtc_session() {
        let _ = session.TrySkipPreviousAsync();
    } else {
        use windows::Win32::UI::Input::KeyboardAndMouse::VK_MEDIA_PREV_TRACK;
        send_media_virtual_key(VK_MEDIA_PREV_TRACK.0 as u8);
    }
}

#[tauri::command]
pub fn media_volume_up() {
    use windows::Win32::UI::Input::KeyboardAndMouse::VK_VOLUME_UP;
    send_media_virtual_key(VK_VOLUME_UP.0 as u8);
}

#[tauri::command]
pub fn media_volume_down() {
    use windows::Win32::UI::Input::KeyboardAndMouse::VK_VOLUME_DOWN;
    send_media_virtual_key(VK_VOLUME_DOWN.0 as u8);
}

#[tauri::command]
pub fn media_seek(position_ms: i64) -> Result<(), String> {
    let session = get_smtc_session().ok_or("没有活跃的媒体会话".to_string())?;
    // 将 ms 转换为 100ns ticks (Windows TimeSpan)
    let ticks = position_ms * 10_000;
    let timespan = windows::Foundation::TimeSpan { Duration: ticks };
    session
        .TryChangePlaybackPositionAsync(timespan.Duration)
        .map_err(|e| format!("Seek 失败: {}", e))?
        .get()
        .map_err(|e| format!("Seek 失败: {}", e))?;
    Ok(())
}

pub(crate) fn get_smtc_media_info() -> Option<(MediaInfo, i64, bool)> {
    // 惰性调用 cloud_fallback，避免每次轮询都枚举窗口
    let cloud_fallback = || get_cloudmusic_fallback_info();

    if let Some((_, media, position_ms, is_playing, app_id)) = select_best_smtc_session() {
        let has_meta = !media.title.trim().is_empty() || !media.artist.trim().is_empty();
        let is_preferred = is_preferred_music_app(&app_id);

        // SMTC 有元数据且来自首选音乐应用：信任 SMTC 的播放状态
        if has_meta && is_preferred {
            return Some((media, position_ms, is_playing));
        }

        // SMTC 缺少元数据，但来自首选应用：用网易云窗口标题补充元数据，但保留 SMTC 的播放状态
        if !has_meta && is_preferred {
            if let Some((fallback_media, _, _)) = cloud_fallback() {
                return Some((fallback_media, position_ms, is_playing));
            }
        }

        // 非首选应用：仅当白名单启用且该应用通过了白名单筛选时才接受
        if has_meta {
            let whitelist = get_smtc_whitelist();
            if whitelist.enabled {
                // 白名单启用，能到达此处说明通过了 select_best_smtc_session 的筛选
                return Some((media, position_ms, is_playing));
            }
            // 白名单未启用，非首选应用不当作音乐
            if let Some(fb) = cloud_fallback() {
                return Some(fb);
            }
            return None;
        }

        // 非首选应用且无元数据：回退到网易云窗口标题
        if let Some(fb) = cloud_fallback() {
            return Some(fb);
        }
        return None;
    }

    cloud_fallback()
}

/// 仅获取封面（歌曲切换时调用，避免每次轮询都读流）
pub(crate) fn get_smtc_thumbnail() -> Option<String> {
    let (session, _, _, _, _) = select_best_smtc_session()?;
    let props = session.TryGetMediaPropertiesAsync().ok()?.get().ok()?;
    extract_thumbnail(&props)
}

/// 获取系统默认音频输出设备的音量 (0.0 ~ 1.0)
fn get_system_volume_internal() -> Result<f32, String> {
    use windows::Win32::Media::Audio::{
        eRender, eConsole,
        IMMDeviceEnumerator, MMDeviceEnumerator,
    };
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::System::Com::{
        CoInitializeEx, CoCreateInstance,
        CLSCTX_ALL, COINIT_MULTITHREADED,
    };


    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|e| format!("CoCreateInstance failed: {}", e))?;
        let device = enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .map_err(|e| format!("GetDefaultAudioEndpoint failed: {}", e))?;
        let volume: IAudioEndpointVolume = device
            .Activate(CLSCTX_ALL, None)
            .map_err(|e| format!("Activate IAudioEndpointVolume failed: {}", e))?;
        let level = volume
            .GetMasterVolumeLevelScalar()
            .map_err(|e| format!("GetMasterVolumeLevelScalar failed: {}", e))?;
        Ok(level)
    }
}

/// 设置系统默认音频输出设备的音量 (0.0 ~ 1.0)
fn set_system_volume_internal(vol: f32) -> Result<(), String> {
    use windows::Win32::Media::Audio::{
        eRender, eConsole,
        IMMDeviceEnumerator, MMDeviceEnumerator,
    };
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::System::Com::{
        CoInitializeEx, CoCreateInstance,
        CLSCTX_ALL, COINIT_MULTITHREADED,
    };
    use windows::core::GUID;

    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|e| format!("CoCreateInstance failed: {}", e))?;
        let device = enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .map_err(|e| format!("GetDefaultAudioEndpoint failed: {}", e))?;
        let volume: IAudioEndpointVolume = device
            .Activate(CLSCTX_ALL, None)
            .map_err(|e| format!("Activate IAudioEndpointVolume failed: {}", e))?;
        let clamped = vol.clamp(0.0, 1.0);
        volume
            .SetMasterVolumeLevelScalar(clamped, &GUID::zeroed())
            .map_err(|e| format!("SetMasterVolumeLevelScalar failed: {}", e))?;
        Ok(())
    }
}

#[tauri::command]
pub fn media_get_volume() -> Result<f32, String> {
    get_system_volume_internal()
}

#[tauri::command]
pub fn media_set_volume(volume: f32) -> Result<(), String> {
    set_system_volume_internal(volume)
}

