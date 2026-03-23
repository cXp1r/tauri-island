use std::collections::HashMap;
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Clone, Debug, Default)]
pub(crate) struct MediaInfo {
    pub title: String,
    pub artist: String,
    pub duration_ms: i64,
}

pub(crate) fn is_preferred_music_app(app_id: &str) -> bool {
    let id = app_id.to_ascii_lowercase();
    [
        "cloudmusic", // 网易云音乐
        "netease",
        "music.163",
        "spotify",
        "qqmusic",
        "kugou",
        "kuwo",
        "foobar",
        "vlc",
        "aimp",
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
                duration_ms: 0,
            });
        }
    }

    Some(MediaInfo {
        title,
        artist: String::new(),
        duration_ms: 0,
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
    // 尝试检测图片格式
    let mime = if buf.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "image/png"
    } else {
        "image/jpeg" // 大多数情况是 JPEG
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

    let is_playing = session
        .GetPlaybackInfo()
        .ok()
        .and_then(|p| p.PlaybackStatus().ok())
        .map(|status| status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing)
        .unwrap_or(false);

    let timeline = session.GetTimelineProperties().ok();

    let position_ms = timeline.as_ref()
        .and_then(|t| t.Position().ok())
        .map(|p| p.Duration / 10_000) // 100ns ticks -> ms
        .unwrap_or(0);

    let duration_ms = timeline.as_ref()
        .and_then(|t| t.EndTime().ok())
        .map(|p| p.Duration / 10_000)
        .unwrap_or(0);

    let (title, artist) = match session.TryGetMediaPropertiesAsync().ok().and_then(|op| op.get().ok()) {
        Some(props) => {
            let title = props.Title().ok().map(|v| v.to_string_lossy()).unwrap_or_default();
            let artist = props.Artist().ok().map(|v| v.to_string_lossy()).unwrap_or_default();
            (title, artist)
        }
        None => (String::new(), String::new()),
    };

    Some((MediaInfo { title, artist, duration_ms }, position_ms, is_playing))
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
    let cloud_fallback = get_cloudmusic_fallback_info();

    if let Some((_, media, position_ms, is_playing, app_id)) = select_best_smtc_session() {
        let has_meta = !media.title.trim().is_empty() || !media.artist.trim().is_empty();
        let is_preferred = is_preferred_music_app(&app_id);

        // SMTC 有元数据且来自首选音乐应用：信任 SMTC 的播放状态
        if has_meta && is_preferred {
            return Some((media, position_ms, is_playing));
        }

        // SMTC 缺少元数据，但来自首选应用：用网易云窗口标题补充元数据，但保留 SMTC 的播放状态
        if !has_meta && is_preferred {
            if let Some((fallback_media, _, _)) = cloud_fallback {
                return Some((fallback_media, position_ms, is_playing));
            }
        }

        // SMTC 有元数据但非首选应用：使用 SMTC 数据
        if has_meta {
            return Some((media, position_ms, is_playing));
        }

        // SMTC 完全没有有用数据：回退到网易云窗口标题
        if let Some(fb) = cloud_fallback {
            return Some(fb);
        }

        return Some((media, position_ms, is_playing));
    }

    cloud_fallback
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

