use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};
use std::process::Command;
use std::os::windows::process::CommandExt;
use tauri::{Emitter, Manager};
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::image::Image;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use zip::read::ZipArchive;
use zip::write::FileOptions;
use zip::ZipWriter;
use serde::{Deserialize, Serialize};

const WIN_W: f64 = 340.0;
const TOP_MARGIN: f64 = 0.0;
const CREATE_NO_WINDOW: u32 = 0x08000000;

const CAPSULE_COLLAPSED_W: f64 = 140.0;
const CAPSULE_COLLAPSED_H: f64 = 50.0;
const CAPSULE_LYRIC_W: f64 = 320.0;
const CAPSULE_EXPANDED_W: f64 = 330.0;
const CAPSULE_EXPANDED_H: f64 = 74.0;
const CAPSULE_TOP_PAD: f64 = 5.0;

const WIN_H_DEFAULT: f64 = 84.0;

// 收起态（绿条）尺寸
const MINIMIZED_W: f64 = 70.0;
const MINIMIZED_H: f64 = 12.0;

const SNAP_DURATION_MS: f64 = 300.0;

/// 全局复用的 HTTP client，避免每次歌词请求重新初始化 TLS
fn shared_http_client() -> &'static reqwest::blocking::Client {
    static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(5))
            .pool_max_idle_per_host(2)
            .build()
            .expect("failed to create http client")
    })
}
const SNAP_FRAME_MS: u64 = 10;
const PRIVACY_POLL_MS: u64 = 1200;

const DEFAULT_BETTERNCM_ROOT: &str = r"C:\betterncm";
const BETTERNCM_PLUGINMARKET_OLD_SOURCE: &str =
    "https://raw.gitcode.com/intensity/bncm-plugin-packed/raw/master/";
const BETTERNCM_PLUGINMARKET_NEW_SOURCE: &str =
    "https://raw.githubusercontent.com/BetterNCM/BetterNCM-Packed-Plugins/master/";

fn ease_out_cubic(t: f64) -> f64 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

fn get_cursor_pos() -> Option<(i32, i32)> {
    use windows::Win32::Foundation::POINT;
    let mut pt = POINT { x: 0, y: 0 };
    unsafe { if GetCursorPos(&mut pt).is_ok() { Some((pt.x, pt.y)) } else { None } }
}

fn get_window_rect(hwnd: HWND) -> Option<RECT> {
    let mut rect = RECT::default();
    unsafe {
        if GetWindowRect(hwnd, &mut rect).is_ok() { Some(rect) } else { None }
    }
}

fn set_click_through(hwnd: HWND, through: bool) {
    unsafe {
        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE);
        let has_transparent = (ex & WS_EX_TRANSPARENT.0 as i32) != 0;
        if through && !has_transparent {
            SetWindowLongW(hwnd, GWL_EXSTYLE, ex | WS_EX_TRANSPARENT.0 as i32 | WS_EX_LAYERED.0 as i32);
        } else if !through && has_transparent {
            SetWindowLongW(hwnd, GWL_EXSTYLE, ex & !(WS_EX_TRANSPARENT.0 as i32));
        }
    }
}

fn snap_back(window: &tauri::WebviewWindow, from_x: f64, from_y: f64, to_x: f64, to_y: f64) {
    let start = Instant::now();
    loop {
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        let p = (elapsed / SNAP_DURATION_MS).min(1.0);
        let t = ease_out_cubic(p);
        let _ = window.set_position(tauri::LogicalPosition::new(
            from_x + (to_x - from_x) * t, from_y + (to_y - from_y) * t,
        ));
        if p >= 1.0 { break; }
        thread::sleep(Duration::from_millis(SNAP_FRAME_MS));
    }
}

fn check_internet() -> bool {
    use windows::Win32::Networking::WinInet::{InternetGetConnectedState, INTERNET_CONNECTION};
    let mut flags = INTERNET_CONNECTION::default();
    unsafe { InternetGetConnectedState(&mut flags, None).is_ok() }
}

fn get_settings_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("dynamic-island");
    fs::create_dir_all(&path).ok();
    path.push("settings.json");
    path
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SettingsData {
    #[serde(default)]
    clipboard_enabled: bool,
    #[serde(default = "default_shortcut")]
    shortcut_key: String,
    #[serde(default = "default_lyric_mode")]
    lyric_mode: String,
    #[serde(default)]
    ai_api_url: String,
    #[serde(default)]
    ai_api_key: String,
    #[serde(default)]
    ai_model: String,
    #[serde(default)]
    is_reasoning_model: bool,
    #[serde(default = "default_indicator_color")]
    indicator_color: String,
    #[serde(default = "default_agent_window_size")]
    agent_window_size: String,
}

fn default_shortcut() -> String {
    "Alt+O".to_string()
}

fn default_lyric_mode() -> String {
    "lyric".to_string()
}

fn default_indicator_color() -> String {
    "#2edb67".to_string()
}

fn default_agent_window_size() -> String {
    "medium".to_string()
}

fn get_agent_window_size(size: &str) -> (f64, f64) {
    match size {
        "small" => (380.0, 400.0),
        "large" => (620.0, 640.0),
        _ => (520.0, 540.0), // medium (default)
    }
}

fn load_settings_from_file() -> SettingsData {
    let path = get_settings_path();
    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(data) = serde_json::from_str::<SettingsData>(&content) {
            return data;
        }
    }
    SettingsData {
        clipboard_enabled: true,
        shortcut_key: default_shortcut(),
        lyric_mode: default_lyric_mode(),
        ai_api_url: String::new(),
        ai_api_key: String::new(),
        ai_model: String::new(),
        is_reasoning_model: false,
        indicator_color: default_indicator_color(),
        agent_window_size: default_agent_window_size(),
    }
}

fn save_settings_to_file(data: &SettingsData) -> Result<(), String> {
    let path = get_settings_path();
    let json = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(())
}

fn get_bt_devices() -> HashSet<String> {
    let mut devices = HashSet::new();
    let ps = r#"[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; Get-PnpDevice -Class Bluetooth | Where-Object {$_.Status -eq 'OK'} | Select-Object -ExpandProperty FriendlyName"#;
    if let Ok(output) = Command::new("powershell").args(["-NoProfile", "-Command", ps]).creation_flags(CREATE_NO_WINDOW).output() {
        let text = String::from_utf8_lossy(&output.stdout);
        let exclude = ["鏋氫妇鍣?", "Enumerator", "Adapter", "閫傞厤鍣?", "Radio"];
        for line in text.lines() {
            let n = line.trim().to_string();
            if !n.is_empty() && !exclude.iter().any(|k| n.contains(k)) { devices.insert(n); }
        }
    }
    devices
}

fn read_reg_u64_value(key: &winreg::RegKey, name: &str) -> Option<u64> {
    if let Ok(v) = key.get_value::<u64, _>(name) {
        return Some(v);
    }
    if let Ok(v) = key.get_value::<u32, _>(name) {
        return Some(v as u64);
    }
    if let Ok(v) = key.get_value::<String, _>(name) {
        return v.trim().parse::<u64>().ok();
    }
    None
}

fn is_registry_capability_key_in_use_recursive(key: &winreg::RegKey) -> bool {
    let start = read_reg_u64_value(key, "LastUsedTimeStart").unwrap_or(0);
    let stop = read_reg_u64_value(key, "LastUsedTimeStop").unwrap_or(0);
    if start > 0 && (stop == 0 || stop < start) {
        return true;
    }

    use winreg::enums::KEY_READ;
    for sub in key.enum_keys().flatten() {
        if let Ok(child) = key.open_subkey_with_flags(&sub, KEY_READ) {
            if is_registry_capability_key_in_use_recursive(&child) {
                return true;
            }
        }
    }
    false
}

fn is_capability_in_use(capability: &str) -> bool {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let root_path = format!(
        r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\{}",
        capability
    );

    match hkcu.open_subkey_with_flags(root_path, KEY_READ) {
        Ok(root) => is_registry_capability_key_in_use_recursive(&root),
        Err(_) => false,
    }
}

fn get_privacy_usage_state() -> (bool, bool) {
    let microphone = is_capability_in_use("microphone");
    let camera = is_capability_in_use("webcam");
    (microphone, camera)
}

fn extract_urls(text: &str) -> Vec<String> {
    let mut urls = Vec::new();
    for word in text.split(|c: char| {
        c.is_whitespace()
            || c == '"'
            || c == '\''
            || c == '<'
            || c == '>'
            || c == '('
            || c == ')'
            || c == '['
            || c == ']'
            || c == ','
            || c == '，'
            || c == '。'
            || c == '、'
    }) {
        let w = word.trim();
        if (w.starts_with("http://") || w.starts_with("https://")) && w.len() > 10 {
            urls.push(w.to_string());
        }
    }
    urls.dedup();
    urls
}

fn read_clipboard_text() -> Option<String> {
    use windows::Win32::System::DataExchange::{OpenClipboard, CloseClipboard, GetClipboardData, IsClipboardFormatAvailable};
    use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
    unsafe {
        if IsClipboardFormatAvailable(13).is_err() { return None; } // CF_UNICODETEXT = 13
        if OpenClipboard(None).is_err() { return None; }
        let h = GetClipboardData(13); // CF_UNICODETEXT
        let result = if let Ok(h) = h {
            let ptr = GlobalLock(windows::Win32::Foundation::HGLOBAL(h.0));
            if ptr.is_null() {
                None
            } else {
                let wide_ptr = ptr as *const u16;
                let mut len = 0;
                while *wide_ptr.add(len) != 0 { len += 1; }
                let slice = std::slice::from_raw_parts(wide_ptr, len);
                let text = String::from_utf16_lossy(slice);
                GlobalUnlock(windows::Win32::Foundation::HGLOBAL(h.0)).ok();
                Some(text.trim().to_string())
            }
        } else {
            None
        };
        CloseClipboard().ok();
        result
    }
}

#[tauri::command]
fn start_drag(state: tauri::State<'_, IslandState>) {
    state.is_dragging.store(true, Ordering::Relaxed);
}

#[tauri::command]
fn end_drag(window: tauri::WebviewWindow, state: tauri::State<'_, IslandState>) {
    state.is_dragging.store(false, Ordering::Relaxed);

    // Agent 展开态时不自动吸附回顶部
    if state.agent_expanded.load(Ordering::Relaxed) {
        return;
    }

    let target_x = state.home_x;
    let target_y = TOP_MARGIN;
    if let Ok(pos) = window.outer_position() {
        let scale = window.scale_factor().unwrap_or(1.0);
        let cx = pos.x as f64 / scale;
        let cy = pos.y as f64 / scale;
        let w = window.clone();
        thread::spawn(move || { snap_back(&w, cx, cy, target_x, target_y); });
    }
}

#[tauri::command]
fn drag_move(window: tauri::WebviewWindow, dx: i32, dy: i32) {
    if let Ok(pos) = window.outer_position() {
        let scale = window.scale_factor().unwrap_or(1.0);
        let logical_x = pos.x as f64 / scale;
        let logical_y = pos.y as f64 / scale;
        let _ = window.set_position(tauri::LogicalPosition::new(
            logical_x + dx as f64,
            logical_y + dy as f64,
        ));
    }
}

#[tauri::command]
fn open_url(url: String) {
    let _ = open::that(&url);
}

#[tauri::command]
fn get_pending_urls(state: tauri::State<'_, IslandState>) -> Vec<String> {
    state.pending_url.lock().unwrap().clone()
}

#[tauri::command]
fn set_interacting(state: tauri::State<'_, IslandState>, active: bool) {
    state.is_interacting.store(active, Ordering::Relaxed);
    if active {
        // 鐢ㄦ埛姝ｅ湪浜や簰锛屼繚鎸佸睍寮€锛屽彇娑堥€氱煡鐘舵€佽榧犳爣绾跨▼涓嶅共鎵?
        state.is_notifying.store(true, Ordering::Relaxed);
    }
}

#[tauri::command]
fn dismiss_island(state: tauri::State<'_, IslandState>, window: tauri::WebviewWindow) {
    state.is_interacting.store(false, Ordering::Relaxed);
    state.is_notifying.store(false, Ordering::Relaxed);
    state.is_expanded.store(false, Ordering::Relaxed);
    let _ = window.emit("set-expand", false);
    let _ = window.emit("reset-view", ());
}

#[tauri::command]
fn set_current_view(state: tauri::State<'_, IslandState>, view: String) {
    let normalized = match view.as_str() {
        "time" | "lyric" | "agent" => view,
        _ => "time".to_string(),
    };
    *state.current_view.lock().unwrap() = normalized;
}

#[tauri::command]
fn sync_window_height(window: tauri::WebviewWindow, height: f64) {
    // 前端传来胶囊实际高度 + padding，动态调整窗口高度
    let new_h = height.max(60.0).min(600.0);
    if let Ok(size) = window.outer_size() {
        let scale = window.scale_factor().unwrap_or(1.0);
        let cur_w = size.width as f64 / scale;
        let _ = window.set_size(tauri::LogicalSize::new(cur_w, new_h));
    }
}

#[tauri::command]
fn set_agent_expanded(window: tauri::WebviewWindow, state: tauri::State<'_, IslandState>, expanded: bool) {
    state.agent_expanded.store(expanded, Ordering::Relaxed);
    let screen_w = state.screen_w;
    let scale = window.scale_factor().unwrap_or(1.0);

    // 从设置中获取窗口大小档位
    let size_setting = state.agent_window_size.lock().unwrap().clone();
    let (agent_w, agent_h) = get_agent_window_size(&size_setting);

    if expanded {
        // 展开：从当前窗口尺寸动画到 agent 展开尺寸
        let target_w = agent_w;
        let target_h = agent_h + 10.0;
        let target_x = (screen_w - target_w) / 2.0;

        if let Ok(pos) = window.outer_position() {
            let from_x = pos.x as f64 / scale;
            let from_y = pos.y as f64 / scale;
            let from_w = WIN_W;
            let from_h = WIN_H_DEFAULT;
            let target_y = from_y;

            let w = window.clone();
            thread::spawn(move || {
                animate_resize(&w, from_x, from_y, from_w, from_h, target_x, target_y, target_w, target_h, 350.0);
            });
        } else {
            let _ = window.set_size(tauri::LogicalSize::new(target_w, target_h));
        }
    } else {
        // 收起：从 agent 展开尺寸动画缩小到默认尺寸，然后 snap_back 到顶部
        if let Ok(pos) = window.outer_position() {
            let from_x = pos.x as f64 / scale;
            let from_y = pos.y as f64 / scale;
            let from_w = agent_w;
            let from_h = agent_h + 10.0;
            // 缩小后保持中心不变
            let center_x = from_x + from_w / 2.0;
            let target_x = center_x - WIN_W / 2.0;
            let target_y = from_y;
            let target_w = WIN_W;
            let target_h = WIN_H_DEFAULT;

            let home_x = (screen_w - WIN_W) / 2.0;
            let w = window.clone();
            thread::spawn(move || {
                animate_resize(&w, from_x, from_y, from_w, from_h, target_x, target_y, target_w, target_h, 350.0);
                // 缩小完成后吸附回顶部
                snap_back(&w, target_x, target_y, home_x, TOP_MARGIN);
            });
        } else {
            let _ = window.set_size(tauri::LogicalSize::new(WIN_W, WIN_H_DEFAULT));
        }
    }
}

#[tauri::command]
fn set_minimized(window: tauri::WebviewWindow, state: tauri::State<'_, IslandState>, minimized: bool) {
    state.is_minimized.store(minimized, Ordering::Relaxed);
    let screen_w = state.screen_w;
    let scale = window.scale_factor().unwrap_or(1.0);

    if minimized {
        // 收起到绿条：窗口缩小到绿条尺寸
        if let Ok(pos) = window.outer_position() {
            let from_x = pos.x as f64 / scale;
            let from_y = pos.y as f64 / scale;
            let from_w = WIN_W;
            let from_h = WIN_H_DEFAULT;

            // 绿条居中在屏幕顶部
            let target_x = (screen_w - MINIMIZED_W) / 2.0;
            let target_y = TOP_MARGIN;
            let target_w = MINIMIZED_W;
            let target_h = MINIMIZED_H;

            let w = window.clone();
            thread::spawn(move || {
                animate_resize(&w, from_x, from_y, from_w, from_h, target_x, target_y, target_w, target_h, 300.0);
            });
        }
    } else {
        // 从绿条展开：恢复到默认尺寸
        if let Ok(pos) = window.outer_position() {
            let from_x = pos.x as f64 / scale;
            let from_y = pos.y as f64 / scale;
            let from_w = MINIMIZED_W;
            let from_h = MINIMIZED_H;

            // 恢复到屏幕顶部居中
            let target_x = (screen_w - WIN_W) / 2.0;
            let target_y = TOP_MARGIN;
            let target_w = WIN_W;
            let target_h = WIN_H_DEFAULT;

            let w = window.clone();
            thread::spawn(move || {
                animate_resize(&w, from_x, from_y, from_w, from_h, target_x, target_y, target_w, target_h, 300.0);
            });
        }
    }
}

#[tauri::command]
fn show_context_menu(app: tauri::AppHandle, window: tauri::WebviewWindow) {
    // 获取鼠标位置
    let Some((x, y)) = get_cursor_pos() else { return };
    let Ok(hwnd) = window.hwnd() else { return };

    let cmd_id: i32 = unsafe {
        let hwnd = HWND(hwnd.0);

        // 创建菜单
        let Ok(h_menu) = CreatePopupMenu() else { return };

        // 添加菜单项
        let _ = AppendMenuW(h_menu, MF_STRING, 1, windows::core::w!("收起"));
        let _ = AppendMenuW(h_menu, MF_STRING, 2, windows::core::w!("设置"));

        // 显示菜单并跟踪选择（阻塞直到用户选择或取消）
        let cmd = TrackPopupMenu(
            h_menu,
            TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD,
            x,
            y,
            None,
            hwnd,
            None,
        );

        let _ = DestroyMenu(h_menu);
        cmd.0
    };

    // TrackPopupMenu 返回后，在新线程中异步执行菜单动作，
    // 避免在当前 command 上下文中创建窗口导致死锁。
    match cmd_id {
        1 => {
            let _ = app.emit("context-menu-action", "minimize");
        }
        2 => {
            thread::spawn(move || {
                // 短暂延迟确保主线程 command 调用完全返回
                thread::sleep(Duration::from_millis(50));
                open_settings(app);
            });
        }
        _ => {}
    }
}

/// 动画插值窗口尺寸和位置，duration_ms 与 CSS transition 同步
fn animate_resize(
    window: &tauri::WebviewWindow,
    from_x: f64, from_y: f64, from_w: f64, from_h: f64,
    to_x: f64, to_y: f64, to_w: f64, to_h: f64,
    duration_ms: f64,
) {
    let start = Instant::now();
    loop {
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        let p = (elapsed / duration_ms).min(1.0);
        let t = ease_out_cubic(p);

        let cur_w = from_w + (to_w - from_w) * t;
        let cur_h = from_h + (to_h - from_h) * t;
        let cur_x = from_x + (to_x - from_x) * t;
        let cur_y = from_y + (to_y - from_y) * t;

        let _ = window.set_size(tauri::LogicalSize::new(cur_w, cur_h));
        let _ = window.set_position(tauri::LogicalPosition::new(cur_x, cur_y));

        if p >= 1.0 { break; }
        thread::sleep(Duration::from_millis(SNAP_FRAME_MS));
    }
}

#[tauri::command]
fn open_settings(app: tauri::AppHandle) {
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
fn get_settings(state: tauri::State<'_, IslandState>) -> serde_json::Value {
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
fn ai_get_settings(state: tauri::State<'_, IslandState>) -> serde_json::Value {
    let api_url = state.ai_api_url.lock().unwrap().clone();
    let api_key = state.ai_api_key.lock().unwrap().clone();
    let model = state.ai_model.lock().unwrap().clone();
    let is_reasoning = state.is_reasoning_model.load(Ordering::Relaxed);

    // 掩码 API Key，只显示前 4 位和后 4 位
    let masked_key = if api_key.len() > 8 {
        format!("{}...{}", &api_key[..4], &api_key[api_key.len()-4..])
    } else if !api_key.is_empty() {
        "****".to_string()
    } else {
        String::new()
    };

    serde_json::json!({
        "api_url": api_url,
        "api_key": masked_key,
        "model": model,
        "is_reasoning_model": is_reasoning
    })
}

#[tauri::command]
fn ai_save_settings(
    state: tauri::State<'_, IslandState>,
    api_url: String,
    api_key: String,
    model: String,
) -> Result<(), String> {
    *state.ai_api_url.lock().unwrap() = api_url.clone();
    *state.ai_api_key.lock().unwrap() = api_key.clone();
    *state.ai_model.lock().unwrap() = model.clone();

    // 检查是否已配置
    let enabled = !api_url.is_empty() && !api_key.is_empty() && !model.is_empty();
    state.ai_enabled.store(enabled, Ordering::Relaxed);

    // 持久化到文件
    let settings_data = SettingsData {
        clipboard_enabled: state.clipboard_enabled.load(Ordering::Relaxed),
        shortcut_key: state.shortcut_key.lock().unwrap().clone(),
        lyric_mode: state.lyric_mode.lock().unwrap().clone(),
        ai_api_url: api_url,
        ai_api_key: api_key,
        ai_model: model,
        is_reasoning_model: state.is_reasoning_model.load(Ordering::Relaxed),
        indicator_color: state.indicator_color.lock().unwrap().clone(),
        agent_window_size: state.agent_window_size.lock().unwrap().clone(),
    };

    save_settings_to_file(&settings_data)?;
    Ok(())
}

#[tauri::command]
fn save_settings(
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

    // 閫氱煡鍓嶇姝岃瘝妯″紡鍙樻洿
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.emit("lyric-mode-changed", &lyric_mode);
    }

    // 閲嶆柊娉ㄥ唽蹇嵎閿?
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
    let settings_data = SettingsData {
        clipboard_enabled,
        shortcut_key,
        lyric_mode,
        ai_api_url: state.ai_api_url.lock().unwrap().clone(),
        ai_api_key: state.ai_api_key.lock().unwrap().clone(),
        ai_model: state.ai_model.lock().unwrap().clone(),
        is_reasoning_model: state.is_reasoning_model.load(Ordering::Relaxed),
        indicator_color,
        agent_window_size,
    };

    let _ = save_settings_to_file(&settings_data);
}

#[tauri::command]
fn ai_detect_model_type(state: tauri::State<'_, IslandState>) -> Result<serde_json::Value, String> {
    let api_url = state.ai_api_url.lock().unwrap().clone();
    let api_key = state.ai_api_key.lock().unwrap().clone();
    let model = state.ai_model.lock().unwrap().clone();

    if api_url.is_empty() || api_key.is_empty() || model.is_empty() {
        return Err("AI 配置不完整".to_string());
    }

    // 构建测试请求
    let request_body = serde_json::json!({
        "model": model,
        "messages": [
            {
                "role": "user",
                "content": "Hi"
            }
        ],
        "stream": false,
        "max_tokens": 10
    });

    let client = shared_http_client();
    let url = if api_url.ends_with("/chat/completions") {
        api_url.clone()
    } else if api_url.ends_with('/') {
        format!("{}chat/completions", api_url)
    } else {
        format!("{}/chat/completions", api_url)
    };

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .map_err(|e| format!("请求失败: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().unwrap_or_default();
        return Err(format!("API 返回错误 {}: {}", status, error_text));
    }

    let response_json: serde_json::Value = response
        .json()
        .map_err(|e| format!("解析响应失败: {}", e))?;

    // 检测是否为思考模型
    let is_reasoning = response_json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| {
            // 检查是否有 reasoning_content 或 thinking 字段
            if m.get("reasoning_content").is_some() {
                Some(true)
            } else if m.get("thinking").is_some() {
                Some(true)
            } else {
                None
            }
        })
        .unwrap_or(false);

    // 更新状态
    state.is_reasoning_model.store(is_reasoning, Ordering::Relaxed);

    // 持久化
    let settings_data = SettingsData {
        clipboard_enabled: state.clipboard_enabled.load(Ordering::Relaxed),
        shortcut_key: state.shortcut_key.lock().unwrap().clone(),
        lyric_mode: state.lyric_mode.lock().unwrap().clone(),
        ai_api_url: api_url,
        ai_api_key: api_key,
        ai_model: model,
        is_reasoning_model: is_reasoning,
        indicator_color: state.indicator_color.lock().unwrap().clone(),
        agent_window_size: state.agent_window_size.lock().unwrap().clone(),
    };

    save_settings_to_file(&settings_data)?;

    Ok(serde_json::json!({
        "is_reasoning_model": is_reasoning
    }))
}

#[tauri::command]
fn ai_stop_generation(state: tauri::State<'_, IslandState>) {
    state.ai_generating.store(false, Ordering::Relaxed);
}

#[tauri::command]
fn ai_clear_history(state: tauri::State<'_, IslandState>) {
    state.ai_history.lock().unwrap().clear();
}

#[tauri::command]
fn ai_send_message(
    app: tauri::AppHandle,
    state: tauri::State<'_, IslandState>,
    content: String,
) -> Result<(), String> {
    let api_url = state.ai_api_url.lock().unwrap().clone();
    let api_key = state.ai_api_key.lock().unwrap().clone();
    let model = state.ai_model.lock().unwrap().clone();
    let is_reasoning = state.is_reasoning_model.load(Ordering::Relaxed);

    if api_url.is_empty() || api_key.is_empty() || model.is_empty() {
        return Err("AI 配置不完整".to_string());
    }

    // 添加用户消息到历史
    {
        let mut history = state.ai_history.lock().unwrap();
        history.push(ChatMessage {
            role: "user".to_string(),
            content: content.clone(),
            reasoning_content: None,
        });

        // 限制历史长度为最近 20 轮对话（40 条消息）
        if history.len() > 40 {
            let excess = history.len() - 40;
            history.drain(0..excess);
        }
    }

    // 设置生成状态
    state.ai_generating.store(true, Ordering::Relaxed);

    // 在新线程中执行流式请求
    let ai_history = state.ai_history.clone();
    let ai_generating = state.ai_generating.clone();

    thread::spawn(move || {
        // 发送开始状态
        let window = if let Some(win) = app.get_webview_window("main") {
            win
        } else {
            return;
        };

        let _ = window.emit("ai-status", serde_json::json!({
            "status": if is_reasoning { "thinking" } else { "generating" }
        }));

        // 构建请求
        let messages: Vec<serde_json::Value> = {
            let history = ai_history.lock().unwrap();
            history.iter().map(|msg| {
                let mut obj = serde_json::json!({
                    "role": msg.role,
                    "content": msg.content
                });
                if let Some(ref reasoning) = msg.reasoning_content {
                    obj["reasoning_content"] = serde_json::Value::String(reasoning.clone());
                }
                obj
            }).collect()
        };

        let request_body = serde_json::json!({
            "model": model,
            "messages": messages,
            "stream": true
        });

        let url = if api_url.ends_with("/chat/completions") {
            api_url.clone()
        } else if api_url.ends_with("/v1") || api_url.ends_with("/v1/") {
            let base = api_url.trim_end_matches('/');
            format!("{}/chat/completions", base)
        } else if api_url.ends_with('/') {
            format!("{}v1/chat/completions", api_url)
        } else {
            format!("{}/v1/chat/completions", api_url)
        };

        println!("[AI] Requesting URL: {}", url);
        println!("[AI] Model: {}, Messages: {}", model, messages.len());

        // 发起流式请求 — 不设置总超时，只设置连接超时
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .build()
            .unwrap();

        let response = match client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
        {
            Ok(resp) => resp,
            Err(e) => {
                println!("[AI] Request failed: {}", e);
                let _ = window.emit("ai-status", serde_json::json!({
                    "status": "error",
                    "error": format!("请求失败: {}", e)
                }));
                ai_generating.store(false, Ordering::Relaxed);
                return;
            }
        };

        println!("[AI] Response status: {}", response.status());

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().unwrap_or_default();
            println!("[AI] API error {}: {}", status, error_text);
            let _ = window.emit("ai-status", serde_json::json!({
                "status": "error",
                "error": format!("API 返回错误 {}: {}", status, error_text)
            }));
            ai_generating.store(false, Ordering::Relaxed);
            return;
        }

        // 解析 SSE 流
        let mut assistant_content = String::new();
        let mut reasoning_content = String::new();
        let mut in_thinking_phase = is_reasoning;

        use std::io::BufRead;
        let reader = std::io::BufReader::new(response);

        for line in reader.lines() {
            // 检查是否被停止
            if !ai_generating.load(Ordering::Relaxed) {
                break;
            }

            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    let _ = window.emit("ai-status", serde_json::json!({
                        "status": "error",
                        "error": format!("读取响应失败: {}", e)
                    }));
                    break;
                }
            };

            let line = line.trim();
            if line.is_empty() || line == "data: [DONE]" {
                continue;
            }

            if !line.starts_with("data: ") {
                continue;
            }

            let json_str = &line[6..];
            let chunk: serde_json::Value = match serde_json::from_str(json_str) {
                Ok(v) => v,
                Err(_) => continue,
            };

            // 提取 delta
            if let Some(choices) = chunk.get("choices").and_then(|c| c.as_array()) {
                if let Some(choice) = choices.get(0) {
                    if let Some(delta) = choice.get("delta") {
                        // 检查思考内容
                        if let Some(reasoning) = delta.get("reasoning_content").and_then(|r| r.as_str()) {
                            reasoning_content.push_str(reasoning);
                            let _ = window.emit("ai-thinking-token", serde_json::json!({
                                "text": reasoning
                            }));
                        }

                        // 检查普通内容
                        if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                            // 如果之前在思考阶段，现在切换到生成阶段
                            if in_thinking_phase && !content.is_empty() {
                                in_thinking_phase = false;
                                let _ = window.emit("ai-status", serde_json::json!({
                                    "status": "generating"
                                }));
                            }

                            assistant_content.push_str(content);
                            let _ = window.emit("ai-token", serde_json::json!({
                                "text": content
                            }));
                        }
                    }
                }
            }
        }

        // 保存完整的 assistant 回复到历史
        if !assistant_content.is_empty() || !reasoning_content.is_empty() {
            let mut history = ai_history.lock().unwrap();
            history.push(ChatMessage {
                role: "assistant".to_string(),
                content: assistant_content,
                reasoning_content: if reasoning_content.is_empty() {
                    None
                } else {
                    Some(reasoning_content)
                },
            });

            // 限制历史长度
            if history.len() > 40 {
                let excess = history.len() - 40;
                history.drain(0..excess);
            }
        }

        // 发送完成状态
        let _ = window.emit("ai-status", serde_json::json!({
            "status": "completed"
        }));

        ai_generating.store(false, Ordering::Relaxed);
    });

    Ok(())
}

// --- 濯掍綋鎺у埗 ---
fn normalize_betterncm_root(install_root: Option<String>) -> PathBuf {
    let root = install_root
        .map(|v| v.trim().trim_matches('"').to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| DEFAULT_BETTERNCM_ROOT.to_string());
    PathBuf::from(root)
}

fn patch_pluginmarket_source_text(input: &str) -> (String, bool) {
    if input.contains(BETTERNCM_PLUGINMARKET_OLD_SOURCE) {
        (
            input.replace(
                BETTERNCM_PLUGINMARKET_OLD_SOURCE,
                BETTERNCM_PLUGINMARKET_NEW_SOURCE,
            ),
            true,
        )
    } else {
        (input.to_string(), false)
    }
}

fn patch_pluginmarket_runtime(root: &Path) -> Result<bool, String> {
    let runtime_main = root.join("plugins_runtime").join("PluginMarket").join("main.js");
    if !runtime_main.exists() {
        return Ok(false);
    }

    let content = fs::read_to_string(&runtime_main)
        .map_err(|e| format!("读取运行时 PluginMarket 失败 {}: {e}", runtime_main.display()))?;
    let (patched, changed) = patch_pluginmarket_source_text(&content);
    if changed {
        fs::write(&runtime_main, patched)
            .map_err(|e| format!("写入运行时 PluginMarket 失败 {}: {e}", runtime_main.display()))?;
    }
    Ok(changed)
}

fn patch_pluginmarket_archive(root: &Path) -> Result<bool, String> {
    let plugin_archive = root.join("plugins").join("PluginMarket.plugin");
    if !plugin_archive.exists() {
        return Ok(false);
    }

    let src_file = fs::File::open(&plugin_archive)
        .map_err(|e| format!("读取 PluginMarket.plugin 失败 {}: {e}", plugin_archive.display()))?;
    let mut src_zip =
        ZipArchive::new(src_file).map_err(|e| format!("解析 PluginMarket.plugin 失败: {e}"))?;

    let tmp_archive = root.join("plugins").join("PluginMarket.plugin.tmp");
    let tmp_file = fs::File::create(&tmp_archive)
        .map_err(|e| format!("创建临时文件失败 {}: {e}", tmp_archive.display()))?;
    let mut writer = ZipWriter::new(tmp_file);
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let mut changed = false;
    for i in 0..src_zip.len() {
        let mut entry = src_zip
            .by_index(i)
            .map_err(|e| format!("读取 PluginMarket 条目失败: {e}"))?;
        let name = entry.name().replace('\\', "/");
        if entry.is_dir() || name.ends_with('/') {
            writer
                .add_directory(name, options.clone())
                .map_err(|e| format!("写入目录条目失败: {e}"))?;
            continue;
        }

        let mut data = Vec::new();
        entry
            .read_to_end(&mut data)
            .map_err(|e| format!("读取条目内容失败: {e}"))?;

        if name.eq_ignore_ascii_case("main.js") {
            let text = String::from_utf8_lossy(&data).to_string();
            let (patched, did_change) = patch_pluginmarket_source_text(&text);
            if did_change {
                data = patched.into_bytes();
                changed = true;
            }
        }

        writer
            .start_file(name, options.clone())
            .map_err(|e| format!("写入文件条目失败: {e}"))?;
        writer
            .write_all(&data)
            .map_err(|e| format!("写入文件内容失败: {e}"))?;
    }

    writer
        .finish()
        .map_err(|e| format!("写入临时插件包失败: {e}"))?;
    drop(src_zip);

    if changed {
        let backup = root.join("plugins").join("PluginMarket.plugin.bak");
        if !backup.exists() {
            fs::copy(&plugin_archive, &backup)
                .map_err(|e| format!("创建 PluginMarket 备份失败 {}: {e}", backup.display()))?;
        }
        fs::copy(&tmp_archive, &plugin_archive).map_err(|e| {
            format!(
                "写回 PluginMarket.plugin 失败 {}: {e}",
                plugin_archive.display()
            )
        })?;
    }

    let _ = fs::remove_file(&tmp_archive);
    Ok(changed)
}

#[tauri::command]
fn install_betterncm_support(install_root: Option<String>) -> Result<serde_json::Value, String> {
    let root = normalize_betterncm_root(install_root);
    if !root.exists() {
        return Err(format!("BetterNCM 目录不存在: {}", root.display()));
    }

    let runtime_patched = patch_pluginmarket_runtime(&root)?;
    let archive_patched = patch_pluginmarket_archive(&root)?;

    Ok(serde_json::json!({
        "root": root.to_string_lossy().to_string(),
        "runtime_patched": runtime_patched,
        "archive_patched": archive_patched
    }))
}

fn is_preferred_music_app(app_id: &str) -> bool {
    let id = app_id.to_ascii_lowercase();
    [
        "cloudmusic", // 缃戞槗浜戦煶涔?
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
    for sep in [" - ", " 鈥?", " 鈥?", " / "] {
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
            });
        }
    }

    Some(MediaInfo {
        title,
        artist: String::new(),
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

fn send_media_virtual_key(vk: u8) {
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

    let position_ms = session
        .GetTimelineProperties()
        .ok()
        .and_then(|t| t.Position().ok())
        .map(|p| p.Duration / 10_000) // 100ns ticks -> ms
        .unwrap_or(0);

    let (title, artist) = match session.TryGetMediaPropertiesAsync().ok().and_then(|op| op.get().ok()) {
        Some(props) => {
            let title = props.Title().ok().map(|v| v.to_string_lossy()).unwrap_or_default();
            let artist = props.Artist().ok().map(|v| v.to_string_lossy()).unwrap_or_default();
            (title, artist)
        }
        None => (String::new(), String::new()),
    };

    Some((MediaInfo { title, artist }, position_ms, is_playing))
}

fn select_best_smtc_session(
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

fn get_smtc_session() -> Option<windows::Media::Control::GlobalSystemMediaTransportControlsSession> {
    select_best_smtc_session().map(|(session, _, _, _, _)| session)
}

#[tauri::command]
fn media_play_pause() {
    if let Some(session) = get_smtc_session() {
        let _ = session.TryTogglePlayPauseAsync();
    } else {
        use windows::Win32::UI::Input::KeyboardAndMouse::VK_MEDIA_PLAY_PAUSE;
        send_media_virtual_key(VK_MEDIA_PLAY_PAUSE.0 as u8);
    }
}

#[tauri::command]
fn media_next() {
    if let Some(session) = get_smtc_session() {
        let _ = session.TrySkipNextAsync();
    } else {
        use windows::Win32::UI::Input::KeyboardAndMouse::VK_MEDIA_NEXT_TRACK;
        send_media_virtual_key(VK_MEDIA_NEXT_TRACK.0 as u8);
    }
}

#[tauri::command]
fn media_prev() {
    if let Some(session) = get_smtc_session() {
        let _ = session.TrySkipPreviousAsync();
    } else {
        use windows::Win32::UI::Input::KeyboardAndMouse::VK_MEDIA_PREV_TRACK;
        send_media_virtual_key(VK_MEDIA_PREV_TRACK.0 as u8);
    }
}

// --- 姝岃瘝鐩稿叧 ---
#[derive(Clone, Debug)]
struct LyricLine {
    time_ms: i64,
    text: String,
}

#[derive(Clone, Debug, Default)]
struct MediaInfo {
    title: String,
    artist: String,
}

fn parse_synced_lyrics(lrc: &str) -> Vec<LyricLine> {
    let mut lines = Vec::new();
    let meta_prefixes = ["浣滆瘝", "浣滄洸", "缂栨洸", "鍒朵綔", "娣烽煶", "姣嶅甫", "褰曢煶", "Lyrics by", "Composed by", "Produced by", "Arranged by"];
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

/// 娓呯悊姝屾洸鏍囬锛屽幓闄ゆ嫭鍙峰唴瀹广€乫eat淇℃伅绛夊共鎵版悳绱㈢殑閮ㄥ垎
fn clean_title(title: &str) -> String {
    let mut s = title.to_string();
    // 鍘婚櫎鍚勭鎷彿鍐呭: (feat. X), [Remix], 锛堢炕鍞憋級绛?
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
    // 鍘婚櫎 " - " 鍚庨潰鐨勫壇鏍囬
    if let Some(idx) = s.find(" - ") {
        s = s[..idx].to_string();
    }
    s.trim().to_string()
}

/// 浠庢悳绱㈢粨鏋滄暟缁勪腑鎻愬彇绗竴涓湁 syncedLyrics 鐨勭粨鏋?
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

/// 浠庡崟涓粨鏋滃璞′腑鎻愬彇 syncedLyrics
fn extract_synced_from_object(json: &serde_json::Value) -> Option<Vec<LyricLine>> {
    let synced = json.get("syncedLyrics").and_then(|v| v.as_str())?;
    if synced.is_empty() { return None; }
    let lines = parse_synced_lyrics(synced);
    if lines.is_empty() { None } else { Some(lines) }
}

/// 浠庣綉鏄撲簯闊充箰鑾峰彇姝岃瘝锛堜綔涓?LRCLIB 鐨勫鐢ㄦ簮锛?
fn fetch_lyrics_from_netease(title: &str, artist: &str) -> Option<Vec<LyricLine>> {
    let client = shared_http_client();

    let cleaned_title = clean_title(title);
    let cleaned_artist = artist.split(['/', ',']).next().unwrap_or(artist).trim();
    let query = format!("{} {}", cleaned_title, cleaned_artist);

    // 鎼滅储姝屾洸
    let search_resp = client.post("https://music.163.com/api/search/get")
        .header("Referer", "https://music.163.com")
        .header("User-Agent", "Mozilla/5.0")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!("s={}&type=1&limit=5&offset=0", urlencoding::encode(&query)))
        .send().ok()?;
    let search_json: serde_json::Value = search_resp.json().ok()?;
    let songs = search_json.get("result")?.get("songs")?.as_array()?;
    if songs.is_empty() { return None; }

    // 鍙栫涓€涓粨鏋滅殑 ID
    let song_id = songs[0].get("id")?.as_i64()?;

    // 鑾峰彇姝岃瘝
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

fn fetch_lyrics_from_lrclib(title: &str, artist: &str) -> Option<Vec<LyricLine>> {
    let client = shared_http_client();
    let ua = "DynamicIsland/1.0 (https://github.com/user/dynamic-island)";

    let cleaned_title = clean_title(title);
    let cleaned_artist = artist.split(['/', ',']).next().unwrap_or(artist).trim();

    // 绛栫暐1: /api/search?track_name=X&artist_name=Y (鍘熷鏍囬)
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

    // 绛栫暐2: /api/search 鐢ㄦ竻鐞嗗悗鐨勬爣棰樺拰绗竴涓壓鏈
    if cleaned_title != title || cleaned_artist != artist {
        let url2 = format!(
            "https://lrclib.net/api/search?track_name={}&artist_name={}",
            urlencoding::encode(&cleaned_title), urlencoding::encode(cleaned_artist)
        );
        if let Ok(resp) = client.get(&url2).header("User-Agent", ua).send() {
            if let Ok(json) = resp.json::<serde_json::Value>() {
                if let Some(lines) = extract_synced_from_array(&json) {
                    return Some(lines);
                }
            }
        }
    }

    // 绛栫暐3: /api/search?q= 鑷敱鎼滅储
    let query = format!("{} {}", cleaned_title, cleaned_artist);
    let url3 = format!(
        "https://lrclib.net/api/search?q={}",
        urlencoding::encode(&query)
    );
    if let Ok(resp) = client.get(&url3).header("User-Agent", ua).send() {
        if let Ok(json) = resp.json::<serde_json::Value>() {
            if let Some(lines) = extract_synced_from_array(&json) {
                return Some(lines);
            }
        }
    }

    // 绛栫暐4: /api/get 绮剧‘鍖归厤锛堜笉闇€瑕乤lbum鍜宒uration涔熷彲浠ヨ瘯锛?
    let url4 = format!(
        "https://lrclib.net/api/get?track_name={}&artist_name={}&album_name=&duration=0",
        urlencoding::encode(&cleaned_title), urlencoding::encode(cleaned_artist)
    );
    if let Ok(resp) = client.get(&url4).header("User-Agent", ua).send() {
        if resp.status().is_success() {
            if let Ok(json) = resp.json::<serde_json::Value>() {
                if let Some(lines) = extract_synced_from_object(&json) {
                    return Some(lines);
                }
            }
        }
    }

    None
}

fn get_current_lyric(lyrics: &[LyricLine], position_ms: i64) -> Option<&LyricLine> {
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

fn get_smtc_media_info() -> Option<(MediaInfo, i64, bool)> {
    let cloud_fallback = get_cloudmusic_fallback_info();

    if let Some((_, media, position_ms, is_playing, app_id)) = select_best_smtc_session() {
        let has_meta = !media.title.trim().is_empty() || !media.artist.trim().is_empty();
        let is_preferred = is_preferred_music_app(&app_id);

        // SMTC 鏄庣‘鏈夋晥锛氱洿鎺ヤ娇鐢?
        if has_meta && is_playing && is_preferred {
            return Some((media, position_ms, is_playing));
        }

        // SMTC 涓嶅彲闈犳椂锛屼紭鍏堢敤缃戞槗浜戠獥鍙ｆ爣棰樺洖閫€
        if (!has_meta || !is_playing || !is_preferred) && cloud_fallback.is_some() {
            let (fallback_media, fallback_pos, fallback_playing) = cloud_fallback.unwrap();
            return Some((fallback_media, fallback_pos, fallback_playing));
        }

        return Some((media, position_ms, is_playing));
    }

    cloud_fallback
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            start_drag, end_drag, drag_move,
            open_url, get_pending_urls, set_interacting, dismiss_island, set_current_view,
            set_agent_expanded, sync_window_height, set_minimized, show_context_menu,
            open_settings, get_settings, save_settings, install_betterncm_support,
            media_play_pause, media_next, media_prev,
            ai_get_settings, ai_save_settings, ai_detect_model_type,
            ai_send_message, ai_stop_generation, ai_clear_history
        ])
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();

            let scale = window.scale_factor().unwrap_or(1.0);
            let screen_w = if let Ok(Some(monitor)) = window.current_monitor() {
                monitor.size().width as f64 / monitor.scale_factor()
            } else { 1920.0 };

            let home_x = (screen_w - WIN_W) / 2.0;
            let _ = window.set_position(tauri::LogicalPosition::new(home_x, TOP_MARGIN));

            let hwnd = HWND(window.hwnd().unwrap().0);
            set_click_through(hwnd, true);

            let is_expanded = Arc::new(AtomicBool::new(false));
            let is_notifying = Arc::new(AtomicBool::new(false));
            let is_dragging = Arc::new(AtomicBool::new(false));
            let is_interacting = Arc::new(AtomicBool::new(false));

            // 从文件加载设置
            let settings = load_settings_from_file();
            let clipboard_enabled = Arc::new(AtomicBool::new(settings.clipboard_enabled));
            let pending_url: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
            let shortcut_key = Arc::new(Mutex::new(settings.shortcut_key.clone()));
            let lyric_mode = Arc::new(Mutex::new(settings.lyric_mode.clone()));
            let current_view = Arc::new(Mutex::new("time".to_string()));

            // AI 相关字段
            let ai_api_url = Arc::new(Mutex::new(settings.ai_api_url.clone()));
            let ai_api_key = Arc::new(Mutex::new(settings.ai_api_key.clone()));
            let ai_model = Arc::new(Mutex::new(settings.ai_model.clone()));
            let is_reasoning_model = Arc::new(AtomicBool::new(settings.is_reasoning_model));
            let ai_enabled = Arc::new(AtomicBool::new(
                !settings.ai_api_url.is_empty() && !settings.ai_api_key.is_empty() && !settings.ai_model.is_empty()
            ));
            let ai_generating = Arc::new(AtomicBool::new(false));
            let ai_history: Arc<Mutex<Vec<ChatMessage>>> = Arc::new(Mutex::new(Vec::new()));
            let agent_expanded = Arc::new(AtomicBool::new(false));
            let is_minimized = Arc::new(AtomicBool::new(false));
            let indicator_color = Arc::new(Mutex::new(settings.indicator_color.clone()));
            let agent_window_size = Arc::new(Mutex::new(settings.agent_window_size.clone()));

            app.manage(IslandState {
                is_notifying: is_notifying.clone(),
                is_expanded: is_expanded.clone(),
                is_dragging: is_dragging.clone(),
                is_interacting: is_interacting.clone(),
                clipboard_enabled: clipboard_enabled.clone(),
                pending_url: pending_url.clone(),
                shortcut_key: shortcut_key.clone(),
                lyric_mode: lyric_mode.clone(),
                current_view: current_view.clone(),
                agent_expanded: agent_expanded.clone(),
                is_minimized: is_minimized.clone(),
                screen_w, home_x, hwnd, scale,
                ai_api_url: ai_api_url.clone(),
                ai_api_key: ai_api_key.clone(),
                ai_model: ai_model.clone(),
                is_reasoning_model: is_reasoning_model.clone(),
                ai_enabled: ai_enabled.clone(),
                ai_generating: ai_generating.clone(),
                ai_history: ai_history.clone(),
                indicator_color: indicator_color.clone(),
                agent_window_size: agent_window_size.clone(),
            });

            // --- 绯荤粺鎵樼洏 ---
            let app_handle = app.handle().clone();
            let quit_item = MenuItemBuilder::with_id("quit", "退出").build(app)?;
            let settings_item = MenuItemBuilder::with_id("settings", "设置").build(app)?;
            let menu = MenuBuilder::new(app).item(&settings_item).item(&quit_item).build()?;
            let _tray = TrayIconBuilder::new()
                .icon(Image::new_owned(create_tray_icon(), 32, 32))
                .menu(&menu).tooltip("灵动岛")
                .on_menu_event(move |app, event| {
                    match event.id().as_ref() {
                        "quit" => app_handle.exit(0),
                        "settings" => {
                            if let Some(win) = app.get_webview_window("settings") {
                                let _ = win.show();
                                let _ = win.set_focus();
                            } else {
                                let _ = tauri::WebviewWindowBuilder::new(app, "settings", tauri::WebviewUrl::App("settings.html".into()))
                                    .title("灵动岛 - 设置")
                                    .inner_size(1000.0, 600.0)
                                    .min_inner_size(800.0, 500.0)
                                    .resizable(true)
                                    .center()
                                    .build();
                            }
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            // --- 娉ㄥ唽榛樿蹇嵎閿?Alt+O ---
            {
                use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
                let pending_url_sc = pending_url.clone();
                let shortcut_str = settings.shortcut_key.clone();
                let _ = app.global_shortcut().on_shortcut(shortcut_str.as_str(), move |_app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        let urls = pending_url_sc.lock().unwrap();
                        if let Some(url) = urls.first() {
                            let _ = open::that(url);
                        }
                    }
                });
            }

            // --- 榧犳爣鐩戞帶绾跨▼ ---
            let win_m = window.clone();
            let noti_m = is_notifying.clone();
            let exp_m = is_expanded.clone();
            let drag_m = is_dragging.clone();
            let interact_m = is_interacting.clone();
            let lyric_mode_m = lyric_mode.clone();
            let current_view_m = current_view.clone();
            let agent_expanded_m = agent_expanded.clone();
            let agent_window_size_m = agent_window_size.clone();
            let hwnd_raw = hwnd.0 as usize;
            let is_music = Arc::new(AtomicBool::new(false));
            let is_music_m = is_music.clone();

            thread::spawn(move || {
                let hwnd = HWND(hwnd_raw as *mut _);
                let center_x = (screen_w * scale / 2.0) as i32;
                let zone_half = (75.0 * scale) as i32;
                let zone_top = (12.0 * scale) as i32;
                let zone_bottom = (90.0 * scale) as i32;
                let mut was_on_capsule = false;

                loop {
                    if let Some((mx, my)) = get_cursor_pos() {
                        // 鏍规嵁褰撳墠鐘舵€佺‘瀹氳兌鍥婂搴?
                        let expanded = exp_m.load(Ordering::Relaxed);
                        let agent_exp = agent_expanded_m.load(Ordering::Relaxed);
                        let view = current_view_m.lock().unwrap().clone();
                        let lyric_mode = lyric_mode_m.lock().unwrap().clone();
                        let (cw, ch, cur_win_w) = if agent_exp && view == "agent" {
                            let size_setting = agent_window_size_m.lock().unwrap().clone();
                            let (aw, ah) = get_agent_window_size(&size_setting);
                            (aw, ah, aw)
                        } else if expanded {
                            (CAPSULE_EXPANDED_W, CAPSULE_EXPANDED_H, WIN_W)
                        } else if view == "lyric" && is_music_m.load(Ordering::Relaxed) && lyric_mode != "off" {
                            (CAPSULE_LYRIC_W, CAPSULE_COLLAPSED_H, WIN_W)
                        } else {
                            (CAPSULE_COLLAPSED_W, CAPSULE_COLLAPSED_H, WIN_W)
                        };

                        let rect = get_window_rect(hwnd);
                        let on_capsule = if let Some(rect) = rect {
                            let win_x = rect.left as f64;
                            let win_y = rect.top as f64;
                            let capsule_x = win_x + (cur_win_w * scale - cw * scale) / 2.0;
                            let capsule_y = win_y + CAPSULE_TOP_PAD * scale;
                            let fmx = mx as f64;
                            let fmy = my as f64;
                            fmx >= capsule_x && fmx <= capsule_x + cw * scale && fmy >= capsule_y && fmy <= capsule_y + ch * scale
                        } else { false };

                        if on_capsule && !was_on_capsule {
                            set_click_through(hwnd, false);
                            was_on_capsule = true;
                        } else if !on_capsule && was_on_capsule {
                            set_click_through(hwnd, true);
                            was_on_capsule = false;
                        }

                        if !agent_exp && !noti_m.load(Ordering::Relaxed) && !drag_m.load(Ordering::Relaxed) && !interact_m.load(Ordering::Relaxed) {
                            let in_zone = mx > center_x - zone_half && mx < center_x + zone_half && my < zone_top;
                            if in_zone && !exp_m.load(Ordering::Relaxed) {
                                exp_m.store(true, Ordering::Relaxed);
                                let _ = win_m.emit("set-expand", true);
                            } else if my > zone_bottom && exp_m.load(Ordering::Relaxed) {
                                exp_m.store(false, Ordering::Relaxed);
                                let _ = win_m.emit("set-expand", false);
                            }
                        }
                    }
                    thread::sleep(Duration::from_millis(16));
                }
            });

            // --- 纭欢鐩戞帶绾跨▼ ---
            let win_hw = window.clone();
            let noti_hw = is_notifying.clone();
            let exp_hw = is_expanded.clone();

            thread::spawn(move || {
                thread::sleep(Duration::from_secs(2));
                let mut was_online = check_internet();
                let mut last_bt = get_bt_devices();
                let mut offline_streak: u32 = 0;
                const OFFLINE_CONFIRM: u32 = 3; // 连续 3 次失败才判定断网

                loop {
                    thread::sleep(Duration::from_secs(8));
                    let online = check_internet();
                    if !online {
                        offline_streak = offline_streak.saturating_add(1);
                        if was_online && offline_streak >= OFFLINE_CONFIRM {
                            was_online = false;
                            trigger_notification(&win_hw, &noti_hw, &exp_hw, "网络已断开");
                        }
                    } else {
                        offline_streak = 0;
                        if !was_online {
                            was_online = true;
                            trigger_notification(&win_hw, &noti_hw, &exp_hw, "网络已连接");
                        }
                    }

                    let bt = get_bt_devices();
                    let new_devs: Vec<_> = bt.difference(&last_bt).cloned().collect();
                    if let Some(name) = new_devs.first() {
                        trigger_notification(&win_hw, &noti_hw, &exp_hw, &format!("蓝牙已连接: {}", name));
                    }
                    let lost_devs: Vec<_> = last_bt.difference(&bt).cloned().collect();
                    if let Some(name) = lost_devs.first() {
                        trigger_notification(&win_hw, &noti_hw, &exp_hw, &format!("蓝牙已断开: {}", name));
                    }
                    last_bt = bt;
                }
            });

            // --- 麦克风/摄像头使用状态监控 ---
            let win_privacy = window.clone();
            thread::spawn(move || {
                let mut last = get_privacy_usage_state();
                let _ = win_privacy.emit("privacy-usage", serde_json::json!({
                    "microphone": last.0,
                    "camera": last.1
                }));

                loop {
                    thread::sleep(Duration::from_millis(PRIVACY_POLL_MS));
                    let current = get_privacy_usage_state();
                    if current != last {
                        last = current;
                        let _ = win_privacy.emit("privacy-usage", serde_json::json!({
                            "microphone": current.0,
                            "camera": current.1
                        }));
                    }
                }
            });

            // --- 鍓创鏉跨洃鎺х嚎绋?---
            let win_cb = window.clone();
            let noti_cb = is_notifying.clone();
            let exp_cb = is_expanded.clone();
            let cb_enabled = clipboard_enabled.clone();
            let pending_url_cb = pending_url.clone();

            thread::spawn(move || {
                let mut last_text = String::new();
                loop {
                    thread::sleep(Duration::from_millis(1200));
                    if !cb_enabled.load(Ordering::Relaxed) { continue; }
                    if let Some(text) = read_clipboard_text() {
                        if text != last_text {
                            last_text = text.clone();
                            let urls = extract_urls(&text);
                            if !urls.is_empty() {
                                *pending_url_cb.lock().unwrap() = urls.clone();
                                let shortcut = "Alt+O";
                                if urls.len() == 1 {
                                    let msg = format!("已复制链接，按 {} 或点击打开", shortcut);
                                    let _ = win_cb.emit("clipboard-urls", urls.clone());
                                    trigger_notification(&win_cb, &noti_cb, &exp_cb, &msg);
                                } else {
                                    let msg = format!("检测到 {} 个链接，点击查看", urls.len());
                                    let _ = win_cb.emit("clipboard-urls", urls.clone());
                                    trigger_notification(&win_cb, &noti_cb, &exp_cb, &msg);
                                }
                            }
                        }
                    }
                }
            });

            // --- 濯掍綋/姝岃瘝鐩戞帶绾跨▼ ---
            let win_media = window.clone();
            let lyric_mode_media = lyric_mode.clone();
            let is_music_media = is_music.clone();

            // 姝岃瘝寮傛鑾峰彇锛氱敤 Arc<Mutex> 鍏变韩缁撴灉 + 浠ｆ暟璁℃暟鍣ㄩ槻姝㈢珵鎬?
            let lyrics_result: Arc<Mutex<Option<(u64, Vec<LyricLine>, bool)>>> = Arc::new(Mutex::new(None));
            // (generation, lyrics, not_found)
            use std::sync::atomic::AtomicU64;
            let lyrics_generation: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));

            thread::spawn(move || {
                let mut current_lyrics: Vec<LyricLine> = Vec::new();
                let mut current_track = String::new();
                let mut last_lyric_text = String::new();
                let mut last_info_track = String::new();
                let mut was_playing = false;
                let mut last_is_playing = false;
                let mut lyrics_not_found = false;
                let mut current_gen: u64 = 0;
                let mut fetch_pending = false; // 褰撳墠浠ｆ槸鍚﹁繕鍦ㄧ瓑寰呯粨鏋?

                loop {
                    thread::sleep(Duration::from_millis(200));

                    // 妫€鏌ュ紓姝ユ瓕璇嶈幏鍙栫粨鏋滐紙鍙帴鍙楀綋鍓嶄唬鐨勭粨鏋滐級
                    {
                        let mut result = lyrics_result.lock().unwrap_or_else(|e| e.into_inner());
                        if let Some((gen, ref lyrics, not_found)) = result.take() {
                            if gen == current_gen {
                                // 褰撳墠浠ｇ殑缁撴灉锛屾帴鍙?
                                current_lyrics = lyrics.clone();
                                lyrics_not_found = not_found;
                                fetch_pending = false;
                                last_lyric_text.clear();
                                last_info_track.clear();
                            }
                            // 鏃т唬鐨勭粨鏋滅洿鎺ヤ涪寮冿紙take 宸茬粡绉婚櫎浜嗭級
                        }
                    }

                    let mode = lyric_mode_media.lock().unwrap().clone();
                    if mode == "off" {
                        if was_playing {
                            was_playing = false;
                            current_track.clear();
                            is_music_media.store(false, Ordering::Relaxed);
                            let _ = win_media.emit("lyric-update", serde_json::json!(null));
                        }
                        continue;
                    }

                    let info = get_smtc_media_info();
                    let (media, position_ms, is_playing) = match info {
                        Some(v) => v,
                        None => {
                            if was_playing {
                                was_playing = false;
                                current_track.clear();
                                is_music_media.store(false, Ordering::Relaxed);
                                let _ = win_media.emit("lyric-update", serde_json::json!(null));
                            }
                            continue;
                        }
                    };

                    // 鎾斁/鏆傚仠鐘舵€佸彉鍖?
                    if is_playing != last_is_playing {
                        last_is_playing = is_playing;
                        let _ = win_media.emit("playback-state", is_playing);
                    }

                    is_music_media.store(true, Ordering::Relaxed);

                    if !is_playing {
                        if was_playing {
                            was_playing = false;
                            let _ = win_media.emit("media-paused", serde_json::json!({
                                "title": media.title,
                                "artist": media.artist
                            }));
                        }
                        continue;
                    }

                    // 姝屾洸鍒囨崲鏃堕噸鏂拌幏鍙栨瓕璇?
                    let track_key = format!("{} - {}", media.artist, media.title);
                    if track_key != current_track {
                        current_track = track_key.clone();
                        last_lyric_text.clear();
                        last_info_track.clear();
                        current_lyrics.clear();
                        lyrics_not_found = false;

                        // 閫掑浠ｆ暟锛屼娇鏃х嚎绋嬬殑缁撴灉鑷姩澶辨晥
                        current_gen = lyrics_generation.fetch_add(1, Ordering::Relaxed) + 1;
                        fetch_pending = false;

                        let _ = win_media.emit("media-changed", serde_json::json!({
                            "title": media.title,
                            "artist": media.artist
                        }));

                        // 寮傛鑾峰彇姝岃瘝锛堜笉闃诲涓诲惊鐜級
                        if mode == "lyric" {
                            let title = media.title.clone();
                            let artist = media.artist.clone();
                            let gen = current_gen;
                            let result_ref = lyrics_result.clone();
                            let gen_ref = lyrics_generation.clone();
                            fetch_pending = true;
                            thread::Builder::new()
                                .name("lyric-fetch".into())
                                .stack_size(512 * 1024) // 512KB 栈，默认 8MB
                                .spawn(move || {
                                // 姣忎釜绛栫暐鍓嶆鏌ヤ唬鏁帮紝濡傛灉宸茶繃鏈熷氨鎻愬墠閫€鍑?
                                let res = std::panic::catch_unwind(|| {
                                    if gen_ref.load(Ordering::Relaxed) != gen { return None; }
                                    let lrclib = fetch_lyrics_from_lrclib(&title, &artist);
                                    if lrclib.is_some() { return lrclib; }
                                    if gen_ref.load(Ordering::Relaxed) != gen { return None; }
                                    fetch_lyrics_from_netease(&title, &artist)
                                });
                                let lyrics = res.unwrap_or(None);
                                // 鍙湁褰撳墠浠ｆ墠鍐欏叆缁撴灉
                                if gen_ref.load(Ordering::Relaxed) == gen {
                                    let not_found = lyrics.is_none();
                                    let mut guard = result_ref.lock().unwrap_or_else(|e| e.into_inner());
                                    *guard = Some((gen, lyrics.unwrap_or_default(), not_found));
                                }
                            }).ok();
                        }
                    }

                    was_playing = true;

                    if mode == "lyric" {
                        // 姝ｅ湪鑾峰彇姝岃瘝涓紝鏄剧ず鍔犺浇鐘舵€?
                        if fetch_pending && current_lyrics.is_empty() {
                            if last_lyric_text != "loading" {
                                last_lyric_text = "loading".to_string();
                                let _ = win_media.emit("lyric-update", serde_json::json!({
                                    "text": "♪",
                                    "title": media.title,
                                    "artist": media.artist
                                }));
                            }
                        } else if lyrics_not_found || (!fetch_pending && current_lyrics.is_empty()) {
                            if last_info_track != track_key {
                                last_info_track = track_key.clone();
                                let _ = win_media.emit("lyric-update", serde_json::json!({
                                    "text": null,
                                    "title": media.title,
                                    "artist": media.artist
                                }));
                            }
                        } else if let Some(line) = get_current_lyric(&current_lyrics, position_ms) {
                            if line.text != last_lyric_text {
                                last_lyric_text = line.text.clone();
                                let _ = win_media.emit("lyric-update", serde_json::json!({
                                    "text": line.text,
                                    "title": media.title,
                                    "artist": media.artist
                                }));
                            }
                        } else if last_lyric_text != "..." {
                            last_lyric_text = "...".to_string();
                            let _ = win_media.emit("lyric-update", serde_json::json!({
                                "text": "♪",
                                "title": media.title,
                                "artist": media.artist
                            }));
                        }
                    } else {
                        // info mode: 鍙彂閫佹瓕鏇蹭俊鎭紙鍘婚噸锛?
                        if last_info_track != track_key {
                            last_info_track = track_key.clone();
                            let _ = win_media.emit("lyric-update", serde_json::json!({
                                "text": null,
                                "title": media.title,
                                "artist": media.artist
                            }));
                        }
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn trigger_notification(
    window: &tauri::WebviewWindow,
    is_notifying: &Arc<AtomicBool>,
    is_expanded: &Arc<AtomicBool>,
    message: &str,
) {
    is_notifying.store(true, Ordering::Relaxed);
    if !is_expanded.load(Ordering::Relaxed) {
        is_expanded.store(true, Ordering::Relaxed);
        let _ = window.emit("set-expand", true);
    }
    let _ = window.emit("show-notice", message);
    thread::sleep(Duration::from_millis(3500));

    // 閫氱煡缁撴潫锛屼絾涓嶅己鍒舵敹缂?鈥?璁╁墠绔喅瀹氫綍鏃舵敹缂?
    is_notifying.store(false, Ordering::Relaxed);
    is_expanded.store(false, Ordering::Relaxed);
    let _ = window.emit("set-expand", false);
    let _ = window.emit("notice-timeout", ());
}

fn create_tray_icon() -> Vec<u8> {
    let (size, center, radius) = (32u32, 16.0, 12.0);
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let dist = ((x as f64 - center).powi(2) + (y as f64 - center).powi(2)).sqrt();
            let idx = ((y * size + x) * 4) as usize;
            if dist <= radius {
                let a = if dist > radius - 1.0 { ((radius - dist).max(0.0) * 255.0) as u8 } else { 255 };
                rgba[idx] = 255; rgba[idx+1] = 255; rgba[idx+2] = 255; rgba[idx+3] = a;
            }
        }
    }
    rgba
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String, // "system" | "user" | "assistant"
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
}

pub struct IslandState {
    pub is_notifying: Arc<AtomicBool>,
    pub is_expanded: Arc<AtomicBool>,
    pub is_dragging: Arc<AtomicBool>,
    pub is_interacting: Arc<AtomicBool>,
    pub clipboard_enabled: Arc<AtomicBool>,
    pub pending_url: Arc<Mutex<Vec<String>>>,
    pub shortcut_key: Arc<Mutex<String>>,
    pub lyric_mode: Arc<Mutex<String>>, // "off" | "info" | "lyric"
    pub current_view: Arc<Mutex<String>>, // "time" | "notice" | "urls" | "lyric" | "agent"
    pub agent_expanded: Arc<AtomicBool>,
    pub is_minimized: Arc<AtomicBool>,
    pub screen_w: f64,
    pub home_x: f64,
    pub hwnd: HWND,
    pub scale: f64,
    // AI Agent 相关字段
    pub ai_api_url: Arc<Mutex<String>>,
    pub ai_api_key: Arc<Mutex<String>>,
    pub ai_model: Arc<Mutex<String>>,
    pub is_reasoning_model: Arc<AtomicBool>,
    pub ai_enabled: Arc<AtomicBool>,
    pub ai_generating: Arc<AtomicBool>,
    pub ai_history: Arc<Mutex<Vec<ChatMessage>>>,
    // 收起状态小横条颜色
    pub indicator_color: Arc<Mutex<String>>,
    // AI 窗口大小档位
    pub agent_window_size: Arc<Mutex<String>>,
}

unsafe impl Send for IslandState {}
unsafe impl Sync for IslandState {}



