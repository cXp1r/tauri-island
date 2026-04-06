use std::fs;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};
use crate::IslandState;
use crate::link_handler::LinkHandler;

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
    #[serde(default = "default_lyric_ws_enabled")]
    pub lyric_ws_enabled: bool,
    #[serde(default = "default_lyric_api_search_enabled")]
    pub lyric_api_search_enabled: bool,
    #[serde(default = "default_lyric_offset_enabled")]
    pub lyric_offset_enabled: bool,
    #[serde(default = "default_lyric_offset_ms")]
    pub lyric_offset_ms: i64,
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
}

fn default_shortcut() -> String {
    "Alt+O".to_string()
}

fn default_lyric_mode() -> String {
    "lyric".to_string()
}

fn default_lyric_ws_enabled() -> bool {
    true
}

fn default_lyric_api_search_enabled() -> bool {
    true
}

fn default_lyric_offset_enabled() -> bool {
    true
}

fn default_lyric_offset_ms() -> i64 {
    200
}

pub(crate) fn default_indicator_color() -> String {
    "#2edb67".to_string()
}

pub(crate) fn default_agent_window_size() -> String {
    "medium".to_string()
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
    SettingsData {
        clipboard_enabled: false,
        shortcut_key: default_shortcut(),
        lyric_mode: default_lyric_mode(),
        lyric_ws_enabled: default_lyric_ws_enabled(),
        lyric_api_search_enabled: default_lyric_api_search_enabled(),
        lyric_offset_enabled: default_lyric_offset_enabled(),
        lyric_offset_ms: default_lyric_offset_ms(),
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
    }
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
        lyric_ws_enabled: state.lyric_ws_enabled.load(Ordering::Relaxed),
        lyric_api_search_enabled: state.lyric_api_search_enabled.load(Ordering::Relaxed),
        lyric_offset_enabled: state.lyric_offset_enabled.load(Ordering::Relaxed),
        lyric_offset_ms: *state.lyric_offset_ms.lock().unwrap(),
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
    let lyric_ws_enabled = state.lyric_ws_enabled.load(Ordering::Relaxed);
    let lyric_api_search_enabled = state.lyric_api_search_enabled.load(Ordering::Relaxed);
    let lyric_offset_enabled = state.lyric_offset_enabled.load(Ordering::Relaxed);
    let lyric_offset_ms = *state.lyric_offset_ms.lock().unwrap();
    let indicator_color = state.indicator_color.lock().unwrap().clone();
    let agent_window_size = state.agent_window_size.lock().unwrap().clone();
    let weather_city = state.weather_city.lock().unwrap().clone();
    let weather_lat = *state.weather_lat.lock().unwrap();
    let weather_lon = *state.weather_lon.lock().unwrap();
    let auto_start = state.auto_start.load(Ordering::Relaxed);
    serde_json::json!({
        "clipboard_enabled": clipboard_enabled,
        "shortcut_key": shortcut,
        "lyric_mode": lyric_mode,
        "lyric_ws_enabled": lyric_ws_enabled,
        "lyric_api_search_enabled": lyric_api_search_enabled,
        "lyric_offset_enabled": lyric_offset_enabled,
        "lyric_offset_ms": lyric_offset_ms,
        "indicator_color": indicator_color,
        "agent_window_size": agent_window_size,
        "weather_city": weather_city,
        "weather_lat": weather_lat,
        "weather_lon": weather_lon,
        "auto_start": auto_start
    })
}

#[tauri::command]
pub fn save_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, IslandState>,
    clipboard_enabled: bool,
    shortcut_key: String,
    lyric_mode: String,
    lyric_ws_enabled: Option<bool>,
    lyric_api_search_enabled: Option<bool>,
    lyric_offset_enabled: Option<bool>,
    lyric_offset_ms: Option<i64>,
    indicator_color: String,
    agent_window_size: String,
    weather_city: Option<String>,
    weather_lat: Option<f64>,
    weather_lon: Option<f64>,
    auto_start: Option<bool>,
) {
    state.clipboard_enabled.store(clipboard_enabled, Ordering::Relaxed);
    *state.shortcut_key.lock().unwrap() = shortcut_key.clone();
    *state.lyric_mode.lock().unwrap() = lyric_mode.clone();
    if let Some(enabled) = lyric_ws_enabled {
        state.lyric_ws_enabled.store(enabled, Ordering::Relaxed);
    }
    if let Some(enabled) = lyric_api_search_enabled {
        state.lyric_api_search_enabled.store(enabled, Ordering::Relaxed);
    }
    if let Some(enabled) = lyric_offset_enabled {
        state.lyric_offset_enabled.store(enabled, Ordering::Relaxed);
    }
    if let Some(ms) = lyric_offset_ms {
        let clamped = ms.clamp(0, 1500);
        *state.lyric_offset_ms.lock().unwrap() = clamped;
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
    if let Some(enabled) = lyric_ws_enabled {
        settings_data.lyric_ws_enabled = enabled;
    }
    if let Some(enabled) = lyric_api_search_enabled {
        settings_data.lyric_api_search_enabled = enabled;
    }
    if let Some(enabled) = lyric_offset_enabled {
        settings_data.lyric_offset_enabled = enabled;
    }
    if let Some(ms) = lyric_offset_ms {
        settings_data.lyric_offset_ms = ms.clamp(0, 1500);
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
    let _ = save_settings_to_file(&settings_data);
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
