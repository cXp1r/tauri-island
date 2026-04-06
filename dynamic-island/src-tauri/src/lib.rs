mod privacy;
mod clipboard;
mod betterncm;
mod lyrics;
pub mod link_handler;
mod media;
pub mod settings;
pub mod ai;
mod window;
mod updater;

use std::collections::HashSet;
use std::process::{Command, Stdio};
use std::os::windows::process::CommandExt;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager};
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::image::Image;
use windows::Win32::Foundation::HWND;

use ai::ChatMessage;
use link_handler::LinkHandler;

pub(crate) const WIN_W: f64 = 340.0;
pub(crate) const TOP_MARGIN: f64 = 0.0;
pub(crate) const CREATE_NO_WINDOW: u32 = 0x08000000;

pub(crate) const CAPSULE_COLLAPSED_W: f64 = 140.0;
pub(crate) const CAPSULE_COLLAPSED_H: f64 = 50.0;
pub(crate) const CAPSULE_LYRIC_W: f64 = 320.0;
pub(crate) const CAPSULE_EXPANDED_W: f64 = 330.0;
pub(crate) const CAPSULE_EXPANDED_H: f64 = 74.0;
pub(crate) const CAPSULE_TOP_PAD: f64 = 5.0;

pub(crate) const WIN_H_DEFAULT: f64 = 84.0;

// 收起态（绿条）尺寸
pub(crate) const MINIMIZED_W: f64 = 70.0;
pub(crate) const MINIMIZED_H: f64 = 12.0;

pub(crate) const SNAP_DURATION_MS: f64 = 300.0;

/// 全局复用的 HTTP client，避免每次歌词请求重新初始化 TLS
pub(crate) fn shared_http_client() -> &'static reqwest::blocking::Client {
    static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(4))
            .pool_max_idle_per_host(4)
            .build()
            .expect("failed to create http client")
    })
}
pub(crate) const SNAP_FRAME_MS: u64 = 10;
const PRIVACY_POLL_MS: u64 = 1200;

fn check_internet() -> bool {
    use windows::Win32::Networking::WinInet::{InternetGetConnectedState, INTERNET_CONNECTION};
    let mut flags = INTERNET_CONNECTION::default();
    unsafe { InternetGetConnectedState(&mut flags, None).is_ok() }
}

/// PowerShell 带超时执行，超时自动 kill 进程
fn run_powershell_with_timeout(args: &[&str], timeout: Duration) -> Option<String> {
    use std::io::Read;
    let mut child = Command::new("powershell")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                let mut stdout = String::new();
                if let Some(mut out) = child.stdout.take() {
                    let _ = out.read_to_string(&mut stdout);
                }
                return Some(stdout);
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                thread::sleep(Duration::from_millis(100));
            }
            Err(_) => {
                let _ = child.kill();
                return None;
            }
        }
    }
}

fn get_bt_devices() -> HashSet<String> {
    let mut devices = HashSet::new();
    let ps = r#"[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; Get-PnpDevice -Class Bluetooth | Where-Object {$_.Status -eq 'OK'} | Select-Object -ExpandProperty FriendlyName"#;
    if let Some(stdout) = run_powershell_with_timeout(&["-NoProfile", "-Command", ps], Duration::from_secs(5)) {
        for line in stdout.lines() {
            let name = line.trim().to_string();
            if !name.is_empty() {
                devices.insert(name);
            }
        }
    }
    devices
}

/// 位置信息
#[derive(Debug, Clone, serde::Serialize)]
struct LocationInfo {
    latitude: f64,
    longitude: f64,
    source: String, // "system" 或 "ip"
    city: Option<String>,
}

/// 使用 Windows 系统定位获取位置
fn get_system_location() -> Option<LocationInfo> {
    // 使用 PowerShell 调用 WinRT 地理位置 API
    let ps_script = r#"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
try {
    $locator = [Windows.Devices.Geolocation.Geolocator]::new()
    $locator.DesiredAccuracy = [Windows.Devices.Geolocation.PositionAccuracy]::Default
    $task = $locator.GetGeopositionAsync().AsTask()
    $task.Wait(10000)
    if ($task.IsCompleted -and $task.Result) {
        $pos = $task.Result.Coordinate.Point.Position
        Write-Output "$($pos.Latitude),$($pos.Longitude)"
    }
} catch {
    # 忽略错误，返回空
}
"#;

    let raw = run_powershell_with_timeout(
        &["-NoProfile", "-Command", ps_script],
        Duration::from_secs(12),
    );
    let stdout = match raw {
        Some(s) => s.trim().to_string(),
        None => return None,
    };
    if stdout.is_empty() || !stdout.contains(',') {
        return None;
    }

    let parts: Vec<&str> = stdout.split(',').collect();
    if parts.len() != 2 {
        return None;
    }

    let lat = parts[0].trim().parse::<f64>().ok()?;
    let lon = parts[1].trim().parse::<f64>().ok()?;

    Some(LocationInfo {
        latitude: lat,
        longitude: lon,
        source: "system".to_string(),
        city: None,
    })
}

/// 使用 IP 定位获取位置（备用方案）
fn get_ip_location() -> Option<LocationInfo> {
    let url = "http://ip-api.com/json?fields=status,lat,lon,city&lang=zh-CN";

    let resp = shared_http_client()
        .get(url)
        .send()
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let json: serde_json::Value = resp.json().ok()?;
    if json["status"].as_str()? != "success" {
        return None;
    }

    Some(LocationInfo {
        latitude: json["lat"].as_f64()?,
        longitude: json["lon"].as_f64()?,
        source: "ip".to_string(),
        city: json["city"].as_str().map(|s| s.to_string()),
    })
}

#[tauri::command]
fn get_location() -> Option<LocationInfo> {
    // 优先使用系统定位
    if let Some(loc) = get_system_location() {
        println!("[Location] 系统定位成功: {:.4}, {:.4}", loc.latitude, loc.longitude);
        return Some(loc);
    }

    // 备用：IP 定位
    if let Some(loc) = get_ip_location() {
        println!("[Location] IP定位成功: {:.4}, {:.4} ({})", loc.latitude, loc.longitude, loc.city.as_deref().unwrap_or("未知"));
        return Some(loc);
    }

    println!("[Location] 定位失败");
    None
}

// ===== Open-Meteo 天气代码映射 =====
fn weather_code_to_cn(code: i64) -> &'static str {
    match code {
        0 | 1 => "晴",
        2 => "少云",
        3 => "多云",
        45 => "雾",
        48 => "雾凇",
        51 | 53 | 55 => "毛毛雨",
        56 | 57 => "冻雨",
        61 => "小雨",
        63 => "中雨",
        65 => "大雨",
        66 | 67 => "冰雨",
        71 => "小雪",
        73 => "中雪",
        75 | 77 => "大雪",
        80 | 81 => "阵雨",
        82 => "强阵雨",
        85 | 86 => "阵雪",
        95 => "雷暴",
        96 | 99 => "雷暴雨",
        _ => "未知",
    }
}

#[derive(Clone, serde::Serialize)]
pub struct WeatherResult {
    desc: String,
    temp: i64,
    city: String,
}

/// 内部天气获取逻辑（在后台线程中调用，不阻塞 command）
fn fetch_weather_internal(
    manual_city: &str,
    manual_lat: f64,
    manual_lon: f64,
) -> Result<WeatherResult, String> {
    let (lat, lon, city_name) = if !manual_city.is_empty() && (manual_lat != 0.0 || manual_lon != 0.0) {
        println!("[Weather] 使用手动设置城市: {}", manual_city);
        (manual_lat, manual_lon, manual_city.to_string())
    } else {
        let loc = get_location().ok_or("无法获取位置信息".to_string())?;
        let city = loc.city.clone().unwrap_or_default();
        (loc.latitude, loc.longitude, city)
    };

    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=temperature_2m,weather_code&timezone=auto",
        lat, lon
    );

    let resp = shared_http_client()
        .get(&url)
        .send()
        .map_err(|e| format!("天气请求失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let json: serde_json::Value = resp.json().map_err(|e| format!("解析失败: {}", e))?;
    let current = &json["current"];
    let weather_code = current["weather_code"].as_i64().unwrap_or(0);
    let temp = current["temperature_2m"].as_f64().unwrap_or(0.0).round() as i64;
    let desc = weather_code_to_cn(weather_code).to_string();

    Ok(WeatherResult { desc, temp, city: city_name })
}

#[tauri::command]
fn get_weather(state: tauri::State<'_, IslandState>) -> Result<WeatherResult, String> {
    // 仅读取缓存，零阻塞
    state.weather_cache.lock().unwrap().clone()
        .ok_or_else(|| "天气数据尚未获取".to_string())
}

#[tauri::command]
fn refresh_weather(state: tauri::State<'_, IslandState>) {
    state.weather_force_refresh.store(true, Ordering::Relaxed);
}

#[tauri::command]
fn save_weather_city(app: tauri::AppHandle, state: tauri::State<'_, IslandState>, city: String, lat: f64, lon: f64) {
    *state.weather_city.lock().unwrap() = city;
    *state.weather_lat.lock().unwrap() = lat;
    *state.weather_lon.lock().unwrap() = lon;

    // 清除旧缓存
    *state.weather_cache.lock().unwrap() = None;

    // 持久化
    let settings_data = settings::build_settings_data(&state);
    let _ = settings::save_settings_to_file(&settings_data);

    // 触发后台线程立即刷新天气
    state.weather_force_refresh.store(true, Ordering::Relaxed);

    // 通知前端城市已变更
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.emit("weather-city-changed", ());
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            window::start_drag, window::end_drag, window::drag_move,
            link_handler::open_url, link_handler::open_url_with_whitelist,
            window::get_pending_urls, window::set_interacting, window::dismiss_island, window::set_current_view,
            window::set_agent_expanded, window::sync_window_height, window::set_minimized, window::show_context_menu,
            window::set_music_expanded,
            settings::open_settings, settings::get_settings, settings::save_settings,
            betterncm::install_betterncm_support,
            media::media_play_pause, media::media_next, media::media_prev,
            ai::ai_get_settings, ai::ai_save_settings, ai::ai_detect_model_type,
            ai::ai_send_message, ai::ai_stop_generation, ai::ai_clear_history,
            settings::get_link_handlers, settings::save_link_handlers,
            link_handler::open_link_with_handler, link_handler::test_link_handler,
            get_location, get_weather, refresh_weather, save_weather_city, settings::search_city,
            media::media_seek,
            media::media_volume_up, media::media_volume_down,
            media::media_get_volume, media::media_set_volume,
            settings::get_auto_start, settings::set_auto_start,
            updater::get_app_version, updater::check_for_updates, updater::download_and_install_update
        ])
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();

            let scale = window.scale_factor().unwrap_or(1.0);
            let screen_w = if let Ok(Some(monitor)) = window.current_monitor() {
                monitor.size().width as f64 / monitor.scale_factor()
            } else { 1920.0 };

            let home_x = (screen_w - WIN_W) / 2.0;
            let _ = window.set_position(tauri::LogicalPosition::new(home_x, TOP_MARGIN));
            let _ = window.set_size(tauri::LogicalSize::new(WIN_W, WIN_H_DEFAULT));

            let hwnd = HWND(window.hwnd().unwrap().0);
            window::set_click_through(hwnd, true);

            let is_expanded = Arc::new(AtomicBool::new(false));
            let is_notifying = Arc::new(AtomicBool::new(false));
            let is_dragging = Arc::new(AtomicBool::new(false));
            let is_interacting = Arc::new(AtomicBool::new(false));

            // 从文件加载设置
            let settings = settings::load_settings_from_file();
            let clipboard_enabled = Arc::new(AtomicBool::new(settings.clipboard_enabled));
            let pending_url: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
            let shortcut_key = Arc::new(Mutex::new(settings.shortcut_key.clone()));
            let lyric_mode = Arc::new(Mutex::new(settings.lyric_mode.clone()));
            let lyric_ws_enabled = Arc::new(AtomicBool::new(settings.lyric_ws_enabled));
            let lyric_api_search_enabled = Arc::new(AtomicBool::new(settings.lyric_api_search_enabled));
            let lyric_offset_enabled = Arc::new(AtomicBool::new(settings.lyric_offset_enabled));
            let lyric_offset_ms = Arc::new(Mutex::new(settings.lyric_offset_ms.clamp(0, 1500)));
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
            let music_expanded = Arc::new(AtomicBool::new(false));
            let is_minimized = Arc::new(AtomicBool::new(false));
            let expand_anim_id = Arc::new(AtomicU64::new(0));
            let indicator_color = Arc::new(Mutex::new(settings.indicator_color.clone()));
            let agent_window_size = Arc::new(Mutex::new(settings.agent_window_size.clone()));
            let link_handlers = Arc::new(Mutex::new(settings.link_handlers.clone()));
            let url_whitelist: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
            let auto_start = Arc::new(AtomicBool::new(settings.auto_start));
            let weather_city = Arc::new(Mutex::new(settings.weather_city.clone()));
            let weather_lat = Arc::new(Mutex::new(settings.weather_lat));
            let weather_lon = Arc::new(Mutex::new(settings.weather_lon));
            let weather_cache: Arc<Mutex<Option<WeatherResult>>> = Arc::new(Mutex::new(None));
            let weather_force_refresh = Arc::new(AtomicBool::new(true)); // 启动后立即获取

            app.manage(IslandState {
                is_notifying: is_notifying.clone(),
                is_expanded: is_expanded.clone(),
                is_dragging: is_dragging.clone(),
                is_interacting: is_interacting.clone(),
                clipboard_enabled: clipboard_enabled.clone(),
                pending_url: pending_url.clone(),
                shortcut_key: shortcut_key.clone(),
                lyric_mode: lyric_mode.clone(),
                lyric_ws_enabled: lyric_ws_enabled.clone(),
                lyric_api_search_enabled: lyric_api_search_enabled.clone(),
                lyric_offset_enabled: lyric_offset_enabled.clone(),
                lyric_offset_ms: lyric_offset_ms.clone(),
                current_view: current_view.clone(),
                agent_expanded: agent_expanded.clone(),
                music_expanded: music_expanded.clone(),
                is_minimized: is_minimized.clone(),
                expand_anim_id: expand_anim_id.clone(),
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
                link_handlers: link_handlers.clone(),
                url_whitelist: url_whitelist.clone(),
                weather_city: weather_city.clone(),
                weather_lat: weather_lat.clone(),
                weather_lon: weather_lon.clone(),
                weather_cache: weather_cache.clone(),
                weather_force_refresh: weather_force_refresh.clone(),
                auto_start: auto_start.clone(),
            });

            // --- 系统托盘 ---
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

            // --- 注册默认快捷键 Alt+O ---
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

            // --- 鼠标监控线程 ---
            let win_m = window.clone();
            let noti_m = is_notifying.clone();
            let exp_m = is_expanded.clone();
            let drag_m = is_dragging.clone();
            let interact_m = is_interacting.clone();
            let lyric_mode_m = lyric_mode.clone();
            let current_view_m = current_view.clone();
            let agent_expanded_m = agent_expanded.clone();
            let music_expanded_m = music_expanded.clone();
            let agent_window_size_m = agent_window_size.clone();
            let expand_anim_id_m = expand_anim_id.clone();
            let is_minimized_m = is_minimized.clone();
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
                    if let Some((mx, my)) = window::get_cursor_pos() {
                        // 根据当前状态确定胶囊宽度
                        let expanded = exp_m.load(Ordering::Relaxed);
                        let agent_exp = agent_expanded_m.load(Ordering::Relaxed);
                        let music_exp = music_expanded_m.load(Ordering::Relaxed);
                        let view = current_view_m.lock().unwrap().clone();
                        let lyric_mode = lyric_mode_m.lock().unwrap().clone();
                        let (cw, ch, cur_win_w) = if is_minimized_m.load(Ordering::Relaxed) {
                            (MINIMIZED_W, MINIMIZED_H, MINIMIZED_W)
                        } else if music_exp && view == "lyric" {
                            // 音乐面板展开态：使用实际窗口尺寸
                            if let Some(r) = window::get_window_rect(hwnd) {
                                let w = (r.right - r.left) as f64 / scale;
                                let h = (r.bottom - r.top) as f64 / scale;
                                (w, h, w)
                            } else {
                                (380.0, 360.0, 380.0)
                            }
                        } else if agent_exp && view == "agent" {
                            let size_setting = agent_window_size_m.lock().unwrap().clone();
                            let (aw, ah) = window::get_agent_window_size(&size_setting);
                            (aw, ah, aw)
                        } else if expanded {
                            (CAPSULE_EXPANDED_W, CAPSULE_EXPANDED_H, WIN_W)
                        } else if view == "lyric" && is_music_m.load(Ordering::Relaxed) && lyric_mode != "off" {
                            (CAPSULE_LYRIC_W, CAPSULE_COLLAPSED_H, WIN_W)
                        } else {
                            (CAPSULE_COLLAPSED_W, CAPSULE_COLLAPSED_H, WIN_W)
                        };

                        let rect = window::get_window_rect(hwnd);
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
                            window::set_click_through(hwnd, false);
                            was_on_capsule = true;
                        } else if !on_capsule && was_on_capsule {
                            window::set_click_through(hwnd, true);
                            was_on_capsule = false;
                        }

                        if !agent_exp && !music_exp && !is_minimized_m.load(Ordering::Relaxed) && !noti_m.load(Ordering::Relaxed) && !drag_m.load(Ordering::Relaxed) && !interact_m.load(Ordering::Relaxed) {
                            let in_zone = mx > center_x - zone_half && mx < center_x + zone_half && my < zone_top;
                            if in_zone && !exp_m.load(Ordering::Relaxed) {
                                exp_m.store(true, Ordering::Relaxed);
                                let _ = win_m.emit("set-expand", true);
                                let gen = expand_anim_id_m.fetch_add(1, Ordering::Relaxed) + 1;
                                let from_h = window::get_window_rect(hwnd).map(|r| (r.bottom - r.top) as f64 / scale).unwrap_or(60.0);
                                let anim_id = expand_anim_id_m.clone();
                                let h_raw = hwnd.0 as usize;
                                thread::spawn(move || {
                                    window::animate_window_height(HWND(h_raw as *mut _), scale, from_h, WIN_H_DEFAULT, WIN_W, 350.0, anim_id, gen);
                                });
                            } else if my > zone_bottom && exp_m.load(Ordering::Relaxed) {
                                exp_m.store(false, Ordering::Relaxed);
                                let _ = win_m.emit("set-expand", false);
                                let gen = expand_anim_id_m.fetch_add(1, Ordering::Relaxed) + 1;
                                let from_h = window::get_window_rect(hwnd).map(|r| (r.bottom - r.top) as f64 / scale).unwrap_or(WIN_H_DEFAULT);
                                let collapsed_h = CAPSULE_COLLAPSED_H + 10.0;
                                let anim_id = expand_anim_id_m.clone();
                                let h_raw = hwnd.0 as usize;
                                thread::spawn(move || {
                                    window::animate_window_height(HWND(h_raw as *mut _), scale, from_h, collapsed_h, WIN_W, 350.0, anim_id, gen);
                                });
                            }
                        }
                    }
                    thread::sleep(Duration::from_millis(16));
                }
            });

            // --- 硬件监控线程 ---
            let win_hw = window.clone();
            let noti_hw = is_notifying.clone();
            let exp_hw = is_expanded.clone();

            thread::spawn(move || {
                thread::sleep(Duration::from_secs(2));
                let mut was_online = check_internet();
                let mut last_bt = get_bt_devices();
                let mut offline_streak: u32 = 0;
                const OFFLINE_CONFIRM: u32 = 3;
                let mut bt_counter: u32 = 0;
                const BT_CHECK_EVERY: u32 = 4; // 每 4 轮（~32 秒）检测一次蓝牙

                loop {
                    thread::sleep(Duration::from_secs(8));

                    // 网络状态检测（每轮）
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

                    // 蓝牙检测（每 BT_CHECK_EVERY 轮）
                    bt_counter += 1;
                    if bt_counter >= BT_CHECK_EVERY {
                        bt_counter = 0;
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
                }
            });

            // --- 麦克风/摄像头使用状态监控 ---
            let win_privacy = window.clone();
            thread::spawn(move || {
                let mut last = privacy::get_privacy_usage_state();
                let _ = win_privacy.emit("privacy-usage", serde_json::json!({
                    "microphone": last.0,
                    "camera": last.1
                }));

                loop {
                    thread::sleep(Duration::from_millis(PRIVACY_POLL_MS));
                    let current = privacy::get_privacy_usage_state();
                    if current != last {
                        last = current;
                        let _ = win_privacy.emit("privacy-usage", serde_json::json!({
                            "microphone": current.0,
                            "camera": current.1
                        }));
                    }
                }
            });

            // --- 剪贴板监控线程 ---
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
                    if let Some(text) = clipboard::read_clipboard_text() {
                        if text != last_text {
                            last_text = text.clone();
                            let urls = clipboard::extract_urls(&text);
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

            // --- 天气后台线程 ---
            let win_weather = window.clone();
            let weather_city_t = weather_city.clone();
            let weather_lat_t = weather_lat.clone();
            let weather_lon_t = weather_lon.clone();
            let weather_cache_t = weather_cache.clone();
            let weather_refresh_t = weather_force_refresh.clone();

            thread::spawn(move || {
                const WEATHER_INTERVAL_SECS: u64 = 20 * 60; // 20 分钟
                let mut last_fetch = Instant::now() - Duration::from_secs(WEATHER_INTERVAL_SECS);

                loop {
                    let should_fetch = weather_refresh_t.compare_exchange(
                        true, false, Ordering::SeqCst, Ordering::Relaxed
                    ).is_ok() || last_fetch.elapsed() >= Duration::from_secs(WEATHER_INTERVAL_SECS);

                    if should_fetch {
                        let city = weather_city_t.lock().unwrap().clone();
                        let lat = *weather_lat_t.lock().unwrap();
                        let lon = *weather_lon_t.lock().unwrap();

                        match fetch_weather_internal(&city, lat, lon) {
                            Ok(result) => {
                                *weather_cache_t.lock().unwrap() = Some(result.clone());
                                let _ = win_weather.emit("weather-updated", serde_json::json!({
                                    "desc": result.desc,
                                    "temp": result.temp,
                                    "city": result.city
                                }));
                                last_fetch = Instant::now();
                                println!("[Weather] 天气更新成功: {} {} {}°C", result.city, result.desc, result.temp);
                            }
                            Err(e) => {
                                println!("[Weather] 天气获取失败: {}", e);
                                let _ = win_weather.emit("weather-error", serde_json::json!({
                                    "error": e
                                }));
                            }
                        }
                    }

                    thread::sleep(Duration::from_secs(5)); // 每 5 秒检查是否需要刷新
                }
            });

            // --- 启动时自动检查更新 ---
            let app_handle_update = app.handle().clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_secs(10));
                match updater::check_for_updates(app_handle_update.clone()) {
                    Ok(info) => {
                        if info.has_update {
                            println!("[Updater] 发现新版本: v{}", info.latest_version);
                            let _ = app_handle_update.emit("update-available", info);
                        } else {
                            println!("[Updater] 当前已是最新版本");
                        }
                    }
                    Err(e) => {
                        println!("[Updater] 启动检查更新失败: {}", e);
                    }
                }
            });

            // --- 媒体/歌词监控线程 ---
            let win_media = window.clone();
            let lyric_mode_media = lyric_mode.clone();
            let lyric_ws_enabled_media = lyric_ws_enabled.clone();
            let lyric_api_search_enabled_media = lyric_api_search_enabled.clone();
            let is_music_media = is_music.clone();

            // 歌词异步获取：用 Arc<Mutex> 共享结果 + 代数计数器防止竞态
            let lyrics_result: Arc<Mutex<Option<(u64, Vec<lyrics::LyricLine>, bool)>>> = Arc::new(Mutex::new(None));
            // (generation, lyrics, not_found)
            use std::sync::atomic::AtomicU64 as AtomicU64Import;
            let lyrics_generation: Arc<AtomicU64Import> = Arc::new(AtomicU64Import::new(0));

            thread::spawn(move || {
                let mut current_lyrics: Vec<lyrics::LyricLine> = Vec::new();
                let mut current_track = String::new();
                let mut last_lyric_text = String::new();
                let mut last_info_track = String::new();
                let mut was_playing = false;
                let mut last_is_playing = false;
                let mut lyrics_not_found = false;
                let mut current_gen: u64 = 0;
                let mut fetch_pending = false; // 当前代是否还在等待结果

                loop {
                    thread::sleep(Duration::from_millis(80));

                    // 检查异步歌词获取结果（只接受当前代的结果）
                    {
                        let mut result = lyrics_result.lock().unwrap_or_else(|e| e.into_inner());
                        if let Some((gen, ref lyric_lines, not_found)) = result.take() {
                            if gen == current_gen {
                                // 当前代的结果，接受
                                current_lyrics = lyric_lines.clone();
                                lyrics_not_found = not_found;
                                fetch_pending = false;
                                last_lyric_text.clear();
                                last_info_track.clear();
                            }
                            // 旧代的结果直接丢弃（take 已经移除了）
                        }
                    }

                    let mode = lyric_mode_media.lock().unwrap().clone();
                    if mode == "off" {
                        if was_playing {
                            lyrics::lyric_log_warn("playback state=stopped reason=lyric_mode_off");
                            was_playing = false;
                            last_is_playing = false;
                            current_track.clear();
                            is_music_media.store(false, Ordering::Relaxed);
                            let _ = win_media.emit("lyric-update", serde_json::json!(null));
                        }
                        continue;
                    }

                    let info = media::get_smtc_media_info();
                    let (media_info, position_ms_raw, is_playing) = match info {
                        Some(v) => v,
                        None => {
                            if was_playing {
                                lyrics::lyric_log_warn("playback state=stopped reason=no_smtc_session");
                                was_playing = false;
                                last_is_playing = false;
                                current_track.clear();
                                is_music_media.store(false, Ordering::Relaxed);
                                let _ = win_media.emit("lyric-update", serde_json::json!(null));
                            }
                            continue;
                        }
                    };

                    let offset_enabled = lyric_offset_enabled.load(Ordering::Relaxed);
                    let offset_ms = *lyric_offset_ms.lock().unwrap();
                    let position_ms = if offset_enabled {
                        position_ms_raw.saturating_add(offset_ms)
                    } else {
                        position_ms_raw
                    };

                    // 播放/暂停状态变化
                    if is_playing != last_is_playing {
                        last_is_playing = is_playing;
                        lyrics::lyric_log_info(&format!(
                            "playback state={} title='{}' artist='{}' genre='{}' position_raw_ms={} position_effective_ms={}",
                            if is_playing { "playing" } else { "paused" },
                            media_info.title,
                            media_info.artist,
                            media_info.genre,
                            position_ms_raw,
                            position_ms
                        ));
                        let _ = win_media.emit("playback-state", is_playing);
                    }

                    is_music_media.store(true, Ordering::Relaxed);

                    if !is_playing {
                        if was_playing {
                            was_playing = false;
                            lyrics::lyric_log_info(&format!(
                                "playback paused title='{}' artist='{}'",
                                media_info.title, media_info.artist
                            ));
                            let _ = win_media.emit("media-paused", serde_json::json!({
                                "title": media_info.title,
                                "artist": media_info.artist
                            }));
                        }
                        continue;
                    }

                    // 歌曲切换时重新获取歌词
                    let track_key = format!("{} - {}", media_info.artist, media_info.title);
                    if track_key != current_track {
                        lyrics::lyric_log_info(&format!(
                            "track changed title='{}' artist='{}' genre='{}' duration_ms={}",
                            media_info.title, media_info.artist, media_info.genre, media_info.duration_ms
                        ));
                        current_track = track_key.clone();
                        last_lyric_text.clear();
                        last_info_track.clear();
                        current_lyrics.clear();
                        lyrics_not_found = false;

                        // 递增代数，使旧线程的结果自动失效
                        current_gen = lyrics_generation.fetch_add(1, Ordering::Relaxed) + 1;
                        fetch_pending = false;

                        let _ = win_media.emit("media-changed", serde_json::json!({
                            "title": media_info.title,
                            "artist": media_info.artist,
                            "genre": media_info.genre,
                            "thumbnail": null,
                            "duration_ms": media_info.duration_ms
                        }));

                        // 异步获取封面（独立线程，不阻塞轮询）
                        {
                            let win_thumb = win_media.clone();
                            thread::Builder::new()
                                .name("thumb-fetch".into())
                                .spawn(move || {
                                    if let Some(thumb) = media::get_smtc_thumbnail() {
                                        let _ = win_thumb.emit("media-thumbnail", serde_json::json!({
                                            "thumbnail": thumb
                                        }));
                                    }
                                }).ok();
                        }

                        // 异步获取歌词（不阻塞主循环，LRCLIB 和网易云并行）
                        if mode == "lyric" {
                            let title = media_info.title.clone();
                            let artist = media_info.artist.clone();
                            let genre = media_info.genre.clone();
                            let ncm_genre_hit_enabled = lyric_ws_enabled_media.load(Ordering::Relaxed);
                            let api_search_enabled = lyric_api_search_enabled_media.load(Ordering::Relaxed);
                            let gen = current_gen;
                            let result_ref = lyrics_result.clone();
                            let gen_ref = lyrics_generation.clone();
                            fetch_pending = true;
                            lyrics::lyric_log_info(&format!(
                                "lyric fetch start gen={} title='{}' artist='{}' genre='{}' strategy=genre_ncmid ncm_genre_hit_enabled={} api_search_enabled={}",
                                gen, title, artist, genre, ncm_genre_hit_enabled, api_search_enabled
                            ));
                            thread::Builder::new()
                                .name("lyric-fetch".into())
                                .stack_size(512 * 1024)
                                .spawn(move || {
                                // 提前检查代数
                                if gen_ref.load(Ordering::Relaxed) != gen { return; }
                                let fetched_lyrics = lyrics::fetch_lyrics_parallel(
                                    &title,
                                    &artist,
                                    &genre,
                                    ncm_genre_hit_enabled,
                                    api_search_enabled,
                                );
                                // 只有当前代才写入结果
                                if gen_ref.load(Ordering::Relaxed) == gen {
                                    let not_found = fetched_lyrics.is_none();
                                    let line_count = fetched_lyrics.as_ref().map(|v| v.len()).unwrap_or(0);
                                    lyrics::lyric_log_info(&format!(
                                        "lyric fetch done gen={} found={} lines={}",
                                        gen,
                                        !not_found,
                                        line_count
                                    ));
                                    let mut guard = result_ref.lock().unwrap_or_else(|e| e.into_inner());
                                    *guard = Some((gen, fetched_lyrics.unwrap_or_default(), not_found));
                                } else {
                                    lyrics::lyric_log_warn(&format!(
                                        "lyric fetch drop stale gen={} current_gen={}",
                                        gen,
                                        gen_ref.load(Ordering::Relaxed)
                                    ));
                                }
                            }).ok();
                        }
                    }

                    was_playing = true;

                    if mode == "lyric" {
                        // 构建歌词文本和附近歌词（文本去重，但始终发送位置）
                        let (text_val, nearby_json) = if fetch_pending && current_lyrics.is_empty() {
                            // 正在获取歌词中
                            (serde_json::json!("♪"), None)
                        } else if lyrics_not_found || (!fetch_pending && current_lyrics.is_empty()) {
                            // 歌词未找到
                            (serde_json::json!(null), None)
                        } else if let Some(line) = lyrics::get_current_lyric(&current_lyrics, position_ms) {
                            // 仅在歌词行变化时计算附近歌词
                            let nearby = if line.text != last_lyric_text {
                                last_lyric_text = line.text.clone();
                                let nearby = lyrics::get_nearby_lyrics(&current_lyrics, position_ms);
                                Some(nearby.iter().map(|(text, is_current)| {
                                    serde_json::json!({"text": text, "is_current": is_current})
                                }).collect::<Vec<_>>())
                            } else {
                                None
                            };
                            (serde_json::json!(line.text), nearby)
                        } else {
                            (serde_json::json!("♪"), None)
                        };

                        // 始终发送，确保进度条持续更新
                        let mut payload = serde_json::json!({
                            "text": text_val,
                            "title": media_info.title,
                            "artist": media_info.artist,
                            "genre": media_info.genre,
                            "position_ms": position_ms,
                            "duration_ms": media_info.duration_ms,
                            "is_playing": is_playing
                        });
                        if let Some(nearby) = nearby_json {
                            payload["nearby_lyrics"] = serde_json::json!(nearby);
                        }
                        let _ = win_media.emit("lyric-update", payload);
                    } else {
                        // info mode: 始终发送位置
                        let _ = win_media.emit("lyric-update", serde_json::json!({
                            "text": null,
                            "title": media_info.title,
                            "artist": media_info.artist,
                            "genre": media_info.genre,
                            "position_ms": position_ms,
                            "duration_ms": media_info.duration_ms,
                            "is_playing": is_playing
                        }));
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
    // 防重入：如果已有通知正在显示，跳过
    if is_notifying.compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed).is_err() {
        return;
    }

    if !is_expanded.load(Ordering::Relaxed) {
        is_expanded.store(true, Ordering::Relaxed);
        let _ = window.emit("set-expand", true);
    }
    let _ = window.emit("show-notice", message);

    // 在独立线程中等待超时，不阻塞调用者
    let noti = is_notifying.clone();
    let exp = is_expanded.clone();
    let win = window.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(3500));
        noti.store(false, Ordering::Relaxed);
        exp.store(false, Ordering::Relaxed);
        let _ = win.emit("set-expand", false);
        let _ = win.emit("notice-timeout", ());
    });
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

pub struct IslandState {
    pub is_notifying: Arc<AtomicBool>,
    pub is_expanded: Arc<AtomicBool>,
    pub is_dragging: Arc<AtomicBool>,
    pub is_interacting: Arc<AtomicBool>,
    pub clipboard_enabled: Arc<AtomicBool>,
    pub pending_url: Arc<Mutex<Vec<String>>>,
    pub shortcut_key: Arc<Mutex<String>>,
    pub lyric_mode: Arc<Mutex<String>>, // "off" | "info" | "lyric"
    pub lyric_ws_enabled: Arc<AtomicBool>,
    pub lyric_api_search_enabled: Arc<AtomicBool>,
    pub lyric_offset_enabled: Arc<AtomicBool>,
    pub lyric_offset_ms: Arc<Mutex<i64>>,
    pub current_view: Arc<Mutex<String>>, // "time" | "notice" | "urls" | "lyric" | "agent"
    pub agent_expanded: Arc<AtomicBool>,
    pub music_expanded: Arc<AtomicBool>,
    pub is_minimized: Arc<AtomicBool>,
    pub expand_anim_id: Arc<AtomicU64>,
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
    // 自定义链接处理器
    pub link_handlers: Arc<Mutex<Vec<LinkHandler>>>,
    // URL 域名白名单（可选）
    pub url_whitelist: Arc<Mutex<Vec<String>>>,
    pub weather_city: Arc<Mutex<String>>,
    pub weather_lat: Arc<Mutex<f64>>,
    pub weather_lon: Arc<Mutex<f64>>,
    // 天气缓存（后台线程写入，command 读取）
    pub weather_cache: Arc<Mutex<Option<WeatherResult>>>,
    pub weather_force_refresh: Arc<AtomicBool>,
    // 开机自启
    pub auto_start: Arc<AtomicBool>,
}

unsafe impl Send for IslandState {}
unsafe impl Sync for IslandState {}
