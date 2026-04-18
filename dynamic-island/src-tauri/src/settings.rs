use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};
use crate::IslandState;
use crate::link_handler::LinkHandler;

/// 歌词补偿：按播放器保存时 clamp 的边界与步长（毫秒）
pub(crate) const LYRIC_OFFSET_MIN_MS: i64 = -3000;
pub(crate) const LYRIC_OFFSET_MAX_MS: i64 = 3000;
pub(crate) const LYRIC_OFFSET_STEP_MS: i64 = 500;

/// 将任意 i64 归一化到 [LYRIC_OFFSET_MIN_MS, LYRIC_OFFSET_MAX_MS] 并按 LYRIC_OFFSET_STEP_MS 取整。
pub(crate) fn clamp_lyric_offset_ms(ms: i64) -> i64 {
    let clamped = ms.clamp(LYRIC_OFFSET_MIN_MS, LYRIC_OFFSET_MAX_MS);
    // 就近取整到 step
    let step = LYRIC_OFFSET_STEP_MS;
    let rounded = ((clamped as f64) / step as f64).round() as i64 * step;
    rounded.clamp(LYRIC_OFFSET_MIN_MS, LYRIC_OFFSET_MAX_MS)
}

/// 规范化 SMTC app_id（trim + 小写），与 smtc_app_whitelist 保持一致
pub(crate) fn normalize_app_id(app_id: &str) -> String {
    app_id.trim().to_ascii_lowercase()
}

/// 对一份 map 做整体规范化（键小写 + 值 clamp），返回新 map
pub(crate) fn normalize_lyric_offsets(
    map: &HashMap<String, i64>,
) -> HashMap<String, i64> {
    let mut out = HashMap::with_capacity(map.len());
    for (k, v) in map.iter() {
        let key = normalize_app_id(k);
        if key.is_empty() {
            continue;
        }
        out.insert(key, clamp_lyric_offset_ms(*v));
    }
    out
}

#[cfg(windows)]
use winreg::enums::*;
#[cfg(windows)]
use winreg::RegKey;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SettingsData {
    #[serde(default)]
    pub clipboard_enabled: bool,
    #[serde(default = "default_shortcut")]
    pub shortcut_key: String,
    #[serde(default = "default_lyric_mode")]
    pub lyric_mode: String,
    #[serde(default = "default_lyric_offset_enabled")]
    pub lyric_offset_enabled: bool,
    /// 按 SMTC app_id 存储的歌词补偿（ms）。默认空表，未配置的播放器视为 0。
    #[serde(default)]
    pub lyric_offsets_by_player: HashMap<String, i64>,
    #[serde(default)]
    pub ai_api_url: String,
    #[serde(default)]
    pub ai_api_key: String,
    #[serde(default)]
    pub ai_model: String,
    #[serde(default)]
    pub is_reasoning_model: bool,
    #[serde(default = "default_indicator_color")]
    pub indicator_color: String,
    #[serde(default = "default_agent_window_size")]
    pub agent_window_size: String,
    #[serde(default = "crate::link_handler::get_default_link_handlers")]
    pub link_handlers: Vec<LinkHandler>,
    #[serde(default)]
    pub weather_city: String,
    #[serde(default)]
    pub weather_lat: f64,
    #[serde(default)]
    pub weather_lon: f64,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default = "default_blacklist_processes")]
    pub blacklist_processes: Vec<String>,
    #[serde(default = "default_blacklist_enabled")]
    pub blacklist_enabled: bool,
    #[serde(default = "default_smtc_whitelist_enabled")]
    pub smtc_whitelist_enabled: bool,
    #[serde(default = "default_smtc_app_whitelist")]
    pub smtc_app_whitelist: Vec<String>,
    #[serde(default)]
    pub preview_updates: bool,
    #[serde(default = "default_show_preview_toggle")]
    pub show_preview_toggle: bool,
}

fn default_show_preview_toggle() -> bool {
    false
}

fn default_shortcut() -> String {
    "Alt+O".to_string()
}

fn default_lyric_mode() -> String {
    "lyric".to_string()
}


fn default_lyric_offset_enabled() -> bool {
    true
}

pub(crate) fn default_indicator_color() -> String {
    "#2edb67".to_string()
}

pub(crate) fn default_agent_window_size() -> String {
    "medium".to_string()
}

fn default_blacklist_enabled() -> bool { true }

fn default_smtc_whitelist_enabled() -> bool { false }

fn default_smtc_app_whitelist() -> Vec<String> {
    vec![
        "汽水音乐".to_string(),
        "cloudmusic.exe".to_string(),
        "qqmusic.exe".to_string(),
        "kugou".to_string(),
    ]
}


fn default_blacklist_processes() -> Vec<String> {
    vec![
        "msedge.exe".to_string(),
        "chrome.exe".to_string(),
        "brave.exe".to_string(),
        "vivaldi.exe".to_string(),
        "opera.exe".to_string(),
        "firefox.exe".to_string(),
    ]
}

pub(crate) fn get_settings_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("dynamic-island");
    fs::create_dir_all(&path).ok();
    path.push("settings.json");
    path
}

pub(crate) fn load_settings_from_file() -> SettingsData {
    let path = get_settings_path();
    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(data) = serde_json::from_str::<SettingsData>(&content) {
            return data;
        }
    }
    // 文件不存在或解析失败，使用默认值并立即写入磁盘
    let defaults = SettingsData {
        clipboard_enabled: false,
        shortcut_key: default_shortcut(),
        lyric_mode: default_lyric_mode(),
        lyric_offset_enabled: default_lyric_offset_enabled(),
        lyric_offsets_by_player: HashMap::new(),
        ai_api_url: String::new(),
        ai_api_key: String::new(),
        ai_model: String::new(),
        is_reasoning_model: false,
        indicator_color: default_indicator_color(),
        agent_window_size: default_agent_window_size(),
        link_handlers: crate::link_handler::get_default_link_handlers(),
        weather_city: String::new(),
        weather_lat: 0.0,
        weather_lon: 0.0,
        auto_start: false,
        blacklist_processes: default_blacklist_processes(),
        blacklist_enabled: true,
        smtc_whitelist_enabled: default_smtc_whitelist_enabled(),
        smtc_app_whitelist: default_smtc_app_whitelist(),
        preview_updates: false,
        show_preview_toggle: false,
    };
    let _ = save_settings_to_file(&defaults);
    defaults
}

pub(crate) fn save_settings_to_file(data: &SettingsData) -> Result<(), String> {
    let path = get_settings_path();
    let json = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(())
}

/// 从 IslandState 构建完整 SettingsData 用于持久化
pub(crate) fn build_settings_data(state: &IslandState) -> SettingsData {
    SettingsData {
        clipboard_enabled: state.clipboard_enabled.load(Ordering::Relaxed),
        shortcut_key: state.shortcut_key.lock().unwrap().clone(),
        lyric_mode: state.lyric_mode.lock().unwrap().clone(),
        lyric_offset_enabled: state.lyric_offset_enabled.load(Ordering::Relaxed),
        lyric_offsets_by_player: state.lyric_offsets_by_player.lock().unwrap().clone(),
        ai_api_url: state.ai_api_url.lock().unwrap().clone(),
        ai_api_key: state.ai_api_key.lock().unwrap().clone(),
        ai_model: state.ai_model.lock().unwrap().clone(),
        is_reasoning_model: state.is_reasoning_model.load(Ordering::Relaxed),
        indicator_color: state.indicator_color.lock().unwrap().clone(),
        agent_window_size: state.agent_window_size.lock().unwrap().clone(),
        link_handlers: state.link_handlers.lock().unwrap().clone(),
        weather_city: state.weather_city.lock().unwrap().clone(),
        weather_lat: *state.weather_lat.lock().unwrap(),
        weather_lon: *state.weather_lon.lock().unwrap(),
        auto_start: state.auto_start.load(Ordering::Relaxed),
        blacklist_processes: state.blacklist_processes.lock().unwrap().clone(),
        blacklist_enabled: state.blacklist_enabled.load(Ordering::Relaxed),
        smtc_whitelist_enabled: state.smtc_whitelist_enabled.load(Ordering::Relaxed),
        smtc_app_whitelist: state.smtc_app_whitelist.lock().unwrap().clone(),
        preview_updates: state.preview_updates.load(Ordering::Relaxed),
        show_preview_toggle: state.show_preview_toggle.load(Ordering::Relaxed),
    }
}

#[tauri::command]
pub fn open_settings(app: tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.show();
        let _ = win.set_focus();
    } else {
        let _ = tauri::WebviewWindowBuilder::new(&app, "settings", tauri::WebviewUrl::App("settings.html".into()))
            .title("灵动岛 - 设置")
            .inner_size(1000.0, 600.0)
            .min_inner_size(800.0, 500.0)
            .resizable(true)
            .center()
            .build();
    }
}

#[tauri::command]
pub fn get_settings(state: tauri::State<'_, IslandState>) -> serde_json::Value {
    let shortcut = state.shortcut_key.lock().unwrap().clone();
    let clipboard_enabled = state.clipboard_enabled.load(Ordering::Relaxed);
    let lyric_mode = state.lyric_mode.lock().unwrap().clone();
    let lyric_offset_enabled = state.lyric_offset_enabled.load(Ordering::Relaxed);
    let indicator_color = state.indicator_color.lock().unwrap().clone();
    let agent_window_size = state.agent_window_size.lock().unwrap().clone();
    let weather_city = state.weather_city.lock().unwrap().clone();
    let weather_lat = *state.weather_lat.lock().unwrap();
    let weather_lon = *state.weather_lon.lock().unwrap();
    let auto_start = state.auto_start.load(Ordering::Relaxed);
    let smtc_whitelist_enabled = state.smtc_whitelist_enabled.load(Ordering::Relaxed);
    let smtc_app_whitelist = state.smtc_app_whitelist.lock().unwrap().clone();
    serde_json::json!({
        "clipboard_enabled": clipboard_enabled,
        "shortcut_key": shortcut,
        "lyric_mode": lyric_mode,
        "lyric_offset_enabled": lyric_offset_enabled,
        "indicator_color": indicator_color,
        "agent_window_size": agent_window_size,
        "weather_city": weather_city,
        "weather_lat": weather_lat,
        "weather_lon": weather_lon,
        "auto_start": auto_start,
        "smtc_whitelist_enabled": smtc_whitelist_enabled,
        "smtc_app_whitelist": smtc_app_whitelist
    })
}

#[tauri::command]
pub fn save_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, IslandState>,
    clipboard_enabled: bool,
    shortcut_key: String,
    lyric_mode: String,
    lyric_offset_enabled: Option<bool>,
    indicator_color: String,
    agent_window_size: String,
    weather_city: Option<String>,
    weather_lat: Option<f64>,
    weather_lon: Option<f64>,
    auto_start: Option<bool>,
    smtc_whitelist_enabled: Option<bool>,
    smtc_app_whitelist: Option<Vec<String>>,
) {
    state.clipboard_enabled.store(clipboard_enabled, Ordering::Relaxed);
    *state.shortcut_key.lock().unwrap() = shortcut_key.clone();
    *state.lyric_mode.lock().unwrap() = lyric_mode.clone();
    if let Some(enabled) = lyric_offset_enabled {
        state.lyric_offset_enabled.store(enabled, Ordering::Relaxed);
    }
    *state.indicator_color.lock().unwrap() = indicator_color.clone();
    *state.agent_window_size.lock().unwrap() = agent_window_size.clone();
    if let Some(ref city) = weather_city {
        *state.weather_city.lock().unwrap() = city.clone();
    }
    if let Some(lat) = weather_lat {
        *state.weather_lat.lock().unwrap() = lat;
    }
    if let Some(lon) = weather_lon {
        *state.weather_lon.lock().unwrap() = lon;
    }
    let mut smtc_whitelist_changed = false;
    if let Some(enabled) = smtc_whitelist_enabled {
        state.smtc_whitelist_enabled.store(enabled, Ordering::Relaxed);
        smtc_whitelist_changed = true;
    }
    if let Some(ref app_ids) = smtc_app_whitelist {
        let normalized: Vec<String> = app_ids
            .iter()
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        *state.smtc_app_whitelist.lock().unwrap() = normalized;
        smtc_whitelist_changed = true;
    }
    if smtc_whitelist_changed {
        let enabled = state.smtc_whitelist_enabled.load(Ordering::Relaxed);
        let app_ids = state.smtc_app_whitelist.lock().unwrap().clone();
        crate::media::update_smtc_whitelist(enabled, app_ids);
    }

    // 通知前端指示器颜色变更
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.emit("indicator-color-changed", &indicator_color);
        let _ = win.emit("agent-window-size-changed", &agent_window_size);
    }

    // 通知前端歌词模式变更
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.emit("lyric-mode-changed", &lyric_mode);
    }

    // 重新注册快捷键
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
    let _ = app.global_shortcut().unregister_all();
    let pending_url = state.pending_url.clone();
    let shortcut_str = shortcut_key.clone();
    let _ = app.global_shortcut().on_shortcut(shortcut_str.as_str(), move |_app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            let urls = pending_url.lock().unwrap();
            if let Some(url) = urls.first() {
                let _ = open::that(url);
            }
        }
    });

    // 持久化到文件
    let mut settings_data = build_settings_data(&state);
    settings_data.clipboard_enabled = clipboard_enabled;
    settings_data.shortcut_key = shortcut_key;
    settings_data.lyric_mode = lyric_mode;
    if let Some(enabled) = lyric_offset_enabled {
        settings_data.lyric_offset_enabled = enabled;
    }
    settings_data.indicator_color = indicator_color;
    settings_data.agent_window_size = agent_window_size;
    if let Some(city) = weather_city {
        settings_data.weather_city = city;
    }
    if let Some(lat) = weather_lat {
        settings_data.weather_lat = lat;
    }
    if let Some(lon) = weather_lon {
        settings_data.weather_lon = lon;
    }
    if let Some(auto) = auto_start {
        settings_data.auto_start = auto;
        state.auto_start.store(auto, Ordering::Relaxed);
        let _ = apply_auto_start(auto);
    }
    if let Some(enabled) = smtc_whitelist_enabled {
        settings_data.smtc_whitelist_enabled = enabled;
    }
    if let Some(ref app_ids) = smtc_app_whitelist {
        let normalized: Vec<String> = app_ids
            .iter()
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        settings_data.smtc_app_whitelist = normalized;
    }
    let _ = save_settings_to_file(&settings_data);
}

// ===== 歌词补偿（按播放器） =====

/// 返回 settings 页子页需要的状态：开关、步进、范围、各播放器 offset、当前命中 app_id
#[tauri::command]
pub fn get_lyric_offset_players(state: tauri::State<'_, IslandState>) -> serde_json::Value {
    let enabled = state.lyric_offset_enabled.load(Ordering::Relaxed);
    let active = state.active_player_app_id.lock().unwrap().clone();
    let players_map = state.lyric_offsets_by_player.lock().unwrap().clone();
    let mut players: Vec<serde_json::Value> = players_map
        .into_iter()
        .map(|(app_id, ms)| {
            serde_json::json!({
                "app_id": app_id,
                "ms": ms,
            })
        })
        .collect();
    // 稳定排序：按 app_id 升序，保证 UI 每次渲染顺序一致
    players.sort_by(|a, b| {
        a.get("app_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .cmp(b.get("app_id").and_then(|v| v.as_str()).unwrap_or(""))
    });
    serde_json::json!({
        "enabled": enabled,
        "active_app_id": active,
        "min_ms": LYRIC_OFFSET_MIN_MS,
        "max_ms": LYRIC_OFFSET_MAX_MS,
        "step_ms": LYRIC_OFFSET_STEP_MS,
        "players": players,
    })
}

/// 设置某个播放器的补偿（ms），clamp 后存盘，返回 clamp 后的值
#[tauri::command]
pub fn set_lyric_offset_for_player(
    state: tauri::State<'_, IslandState>,
    app_id: String,
    ms: i64,
) -> Result<i64, String> {
    let key = normalize_app_id(&app_id);
    if key.is_empty() {
        return Err("app_id 不能为空".to_string());
    }
    let clamped = clamp_lyric_offset_ms(ms);
    {
        let mut map = state.lyric_offsets_by_player.lock().unwrap();
        map.insert(key.clone(), clamped);
    }
    let data = build_settings_data(&state);
    save_settings_to_file(&data)?;
    Ok(clamped)
}

/// 即时开关歌词补偿总开关（替代 save_settings 的整单保存）
#[tauri::command]
pub fn set_lyric_offset_enabled(
    state: tauri::State<'_, IslandState>,
    enabled: bool,
) -> Result<(), String> {
    state.lyric_offset_enabled.store(enabled, Ordering::Relaxed);
    let data = build_settings_data(&state);
    save_settings_to_file(&data)?;
    Ok(())
}

/// 删除某播放器的补偿条目
#[tauri::command]
pub fn delete_lyric_offset_player(
    state: tauri::State<'_, IslandState>,
    app_id: String,
) -> Result<(), String> {
    let key = normalize_app_id(&app_id);
    if key.is_empty() {
        return Err("app_id 不能为空".to_string());
    }
    {
        let mut map = state.lyric_offsets_by_player.lock().unwrap();
        map.remove(&key);
    }
    let data = build_settings_data(&state);
    save_settings_to_file(&data)?;
    Ok(())
}

#[tauri::command]
pub fn get_link_handlers(state: tauri::State<'_, IslandState>) -> Vec<LinkHandler> {
    state.link_handlers.lock().unwrap().clone()
}

#[tauri::command]
pub fn save_link_handlers(
    state: tauri::State<'_, IslandState>,
    handlers: Vec<LinkHandler>,
) -> Result<(), String> {
    *state.link_handlers.lock().unwrap() = handlers.clone();

    let mut settings_data = build_settings_data(&state);
    settings_data.link_handlers = handlers;
    save_settings_to_file(&settings_data)?;

    Ok(())
}

#[tauri::command]
pub fn get_smtc_whitelist(state: tauri::State<'_, IslandState>) -> Vec<String> {
    state.smtc_app_whitelist.lock().unwrap().clone()
}

#[tauri::command]
pub fn get_smtc_whitelist_enabled(state: tauri::State<'_, IslandState>) -> bool {
    state.smtc_whitelist_enabled.load(Ordering::Relaxed)
}

#[tauri::command]
pub fn set_smtc_whitelist_enabled(
    state: tauri::State<'_, IslandState>,
    enabled: bool,
) -> Result<(), String> {
    state.smtc_whitelist_enabled.store(enabled, Ordering::Relaxed);
    let mut settings_data = build_settings_data(&state);
    settings_data.smtc_whitelist_enabled = enabled;
    save_settings_to_file(&settings_data)?;

    let app_ids = state.smtc_app_whitelist.lock().unwrap().clone();
    crate::media::update_smtc_whitelist(enabled, app_ids);

    Ok(())
}

#[tauri::command]
pub fn save_smtc_whitelist(
    state: tauri::State<'_, IslandState>,
    app_ids: Vec<String>,
) -> Result<(), String> {
    let normalized: Vec<String> = app_ids
        .into_iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    *state.smtc_app_whitelist.lock().unwrap() = normalized.clone();

    let mut settings_data = build_settings_data(&state);
    settings_data.smtc_app_whitelist = normalized;
    save_settings_to_file(&settings_data)?;

    let enabled = state.smtc_whitelist_enabled.load(Ordering::Relaxed);
    let app_ids = state.smtc_app_whitelist.lock().unwrap().clone();
    crate::media::update_smtc_whitelist(enabled, app_ids);

    Ok(())
}

// ===== 城市搜索 =====

#[derive(Debug, Clone, Serialize)]
pub struct CityResult {
    pub name: String,
    pub country: String,
    pub admin1: String, // 省/州
    pub latitude: f64,
    pub longitude: f64,
}

#[tauri::command]
pub fn search_city(query: String) -> Result<Vec<CityResult>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }

    let url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=8&language=zh&format=json",
        urlencoding::encode(query.trim())
    );

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let resp = client
        .get(&url)
        .send()
        .map_err(|e| format!("请求失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let json: serde_json::Value = resp.json().map_err(|e| format!("解析失败: {}", e))?;

    let results = json["results"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    Some(CityResult {
                        name: item["name"].as_str()?.to_string(),
                        country: item["country"].as_str().unwrap_or("").to_string(),
                        admin1: item["admin1"].as_str().unwrap_or("").to_string(),
                        latitude: item["latitude"].as_f64()?,
                        longitude: item["longitude"].as_f64()?,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(results)
}

// ===== 开机自启 (Windows Registry) =====

const AUTOSTART_REG_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run";
const AUTOSTART_REG_NAME: &str = "DynamicIsland";

/// 读取注册表判断当前是否设置了自启
#[tauri::command]
pub fn get_auto_start() -> bool {
    #[cfg(windows)]
    {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(run_key) = hkcu.open_subkey(AUTOSTART_REG_KEY) {
            return run_key.get_value::<String, _>(AUTOSTART_REG_NAME).is_ok();
        }
        false
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// 设置或取消开机自启
pub(crate) fn apply_auto_start(enabled: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let run_key = hkcu
            .open_subkey_with_flags(AUTOSTART_REG_KEY, KEY_WRITE)
            .map_err(|e| format!("打开注册表失败: {}", e))?;

        if enabled {
            let exe_path = std::env::current_exe()
                .map_err(|e| format!("获取程序路径失败: {}", e))?
                .to_string_lossy()
                .to_string();
            run_key
                .set_value(AUTOSTART_REG_NAME, &exe_path)
                .map_err(|e| format!("写入注册表失败: {}", e))?;
        } else {
            let _ = run_key.delete_value(AUTOSTART_REG_NAME);
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = enabled;
        Ok(())
    }
}

/// Tauri command: 设置开机自启
#[tauri::command]
pub fn set_auto_start(
    state: tauri::State<'_, IslandState>,
    enabled: bool,
) -> Result<(), String> {
    apply_auto_start(enabled)?;
    state.auto_start.store(enabled, Ordering::Relaxed);

    let mut settings_data = build_settings_data(&state);
    settings_data.auto_start = enabled;
    save_settings_to_file(&settings_data)?;

    Ok(())
}

#[tauri::command]
pub fn get_preview_updates(state: tauri::State<'_, IslandState>) -> bool {
    state.preview_updates.load(Ordering::Relaxed)
}

#[tauri::command]
pub fn set_preview_updates(
    state: tauri::State<'_, IslandState>,
    enabled: bool,
) -> Result<(), String> {
    state.preview_updates.store(enabled, Ordering::Relaxed);
    let settings_data = build_settings_data(&state);
    save_settings_to_file(&settings_data)?;
    Ok(())
}

#[tauri::command]
pub fn get_show_preview_toggle(state: tauri::State<'_, IslandState>) -> bool {
    state.show_preview_toggle.load(Ordering::Relaxed)
}

#[tauri::command]
pub fn set_show_preview_toggle(
    state: tauri::State<'_, IslandState>,
    enabled: bool,
) -> Result<(), String> {
    state.show_preview_toggle.store(enabled, Ordering::Relaxed);
    let settings_data = build_settings_data(&state);
    save_settings_to_file(&settings_data)?;
    Ok(())
}

#[tauri::command]
pub fn get_blacklist(state: tauri::State<'_, IslandState>) -> Vec<String> {
    state.blacklist_processes.lock().unwrap().clone()
}

#[tauri::command]
pub fn get_blacklist_enabled(state: tauri::State<'_, IslandState>) -> bool {
    state.blacklist_enabled.load(Ordering::Relaxed)
}

#[tauri::command]
pub fn set_blacklist_enabled(
    state: tauri::State<'_, IslandState>,
    enabled: bool,
) -> Result<(), String> {
    state.blacklist_enabled.store(enabled, Ordering::Relaxed);
    let mut settings_data = build_settings_data(&state);
    settings_data.blacklist_enabled = enabled;
    save_settings_to_file(&settings_data)?;
    Ok(())
}

#[tauri::command]
pub fn save_blacklist(
    state: tauri::State<'_, IslandState>,
    processes: Vec<String>,
) -> Result<(), String> {
    let normalized: Vec<String> = processes.iter()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    *state.blacklist_processes.lock().unwrap() = normalized.clone();

    let mut settings_data = build_settings_data(&state);
    settings_data.blacklist_processes = normalized;
    save_settings_to_file(&settings_data)?;

    Ok(())
}
