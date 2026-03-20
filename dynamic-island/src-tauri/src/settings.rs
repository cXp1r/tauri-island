use std::fs;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};
use crate::IslandState;
use crate::link_handler::LinkHandler;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SettingsData {
    #[serde(default)]
    pub clipboard_enabled: bool,
    #[serde(default = "default_shortcut")]
    pub shortcut_key: String,
    #[serde(default = "default_lyric_mode")]
    pub lyric_mode: String,
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
}

fn default_shortcut() -> String {
    "Alt+O".to_string()
}

fn default_lyric_mode() -> String {
    "lyric".to_string()
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
        ai_api_url: String::new(),
        ai_api_key: String::new(),
        ai_model: String::new(),
        is_reasoning_model: false,
        indicator_color: default_indicator_color(),
        agent_window_size: default_agent_window_size(),
        link_handlers: crate::link_handler::get_default_link_handlers(),
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
        ai_api_url: state.ai_api_url.lock().unwrap().clone(),
        ai_api_key: state.ai_api_key.lock().unwrap().clone(),
        ai_model: state.ai_model.lock().unwrap().clone(),
        is_reasoning_model: state.is_reasoning_model.load(Ordering::Relaxed),
        indicator_color: state.indicator_color.lock().unwrap().clone(),
        agent_window_size: state.agent_window_size.lock().unwrap().clone(),
        link_handlers: state.link_handlers.lock().unwrap().clone(),
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
    let indicator_color = state.indicator_color.lock().unwrap().clone();
    let agent_window_size = state.agent_window_size.lock().unwrap().clone();
    serde_json::json!({
        "clipboard_enabled": clipboard_enabled,
        "shortcut_key": shortcut,
        "lyric_mode": lyric_mode,
        "indicator_color": indicator_color,
        "agent_window_size": agent_window_size
    })
}

#[tauri::command]
pub fn save_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, IslandState>,
    clipboard_enabled: bool,
    shortcut_key: String,
    lyric_mode: String,
    indicator_color: String,
    agent_window_size: String,
) {
    state.clipboard_enabled.store(clipboard_enabled, Ordering::Relaxed);
    *state.shortcut_key.lock().unwrap() = shortcut_key.clone();
    *state.lyric_mode.lock().unwrap() = lyric_mode.clone();
    *state.indicator_color.lock().unwrap() = indicator_color.clone();
    *state.agent_window_size.lock().unwrap() = agent_window_size.clone();

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
    settings_data.indicator_color = indicator_color;
    settings_data.agent_window_size = agent_window_size;
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
