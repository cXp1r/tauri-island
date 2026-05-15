pub mod logger;
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
mod ceverything;
mod sadb;
mod email;
mod cc;
mod tools;

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
use email::Email;
use link_handler::LinkHandler;

pub(crate) const WIN_W: f64 = 140.0;
pub(crate) const TOP_MARGIN: f64 = 0.0;
pub(crate) const CREATE_NO_WINDOW: u32 = 0x08000000;

// ── 胶囊尺寸（与 base.css :root 变量对应） ──
#[allow(unused)]
pub(crate) const CAPSULE_COLLAPSED_W: f64 = 140.0; // CSS --collapsed-w
#[allow(unused)]
pub(crate) const CAPSULE_COLLAPSED_H: f64 = 50.0;  // CSS --collapsed-h
#[allow(unused)]
pub(crate) const CAPSULE_LYRIC_W: f64 = 340.0;     // CSS --lyric-collapsed-w
#[allow(unused)]
pub(crate) const CAPSULE_EXPANDED_W: f64 = 330.0;  // CSS --expanded-w
#[allow(unused)]
pub(crate) const CAPSULE_EXPANDED_H: f64 = 74.0;   // CSS --expanded-h
#[allow(unused)]
pub(crate) const CAPSULE_TOP_PAD: f64 = 5.0;       // body padding-top

pub(crate) const WIN_H_DEFAULT: f64 = 84.0;        // CAPSULE_EXPANDED_H + padding


pub(crate) const SNAP_DURATION_MS: f64 = 300.0;
pub(crate) const SNAP_FRAME_MS: u64 = 16;
const PRIVACY_POLL_MS: u64 = 1200;



/// 全局复用的 HTTP client，天气处调用
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


/// 位置信息
#[derive(Debug, Clone, serde::Serialize)]
struct LocationInfo {
    latitude: f64,
    longitude: f64,
    source: String, // "system" 或 "ip"
    city: Option<String>,
}




#[tauri::command]
fn get_location() -> Option<LocationInfo> {
    let url = "http://ip-api.com/json?fields=status,lat,lon,city&lang=zh-CN";

    let resp = shared_http_client()
        .get(url)
        .send()
        .ok()?;

    if !resp.status().is_success() {
        logger::warn("init", "定位失败");
        return None;
    }

    let json: serde_json::Value = resp.json().ok()?;
    if json["status"].as_str()? != "success" {
        logger::warn("init", "定位失败");
        return None;
    }

    Some(LocationInfo {
        latitude: json["lat"].as_f64()?,
        longitude: json["lon"].as_f64()?,
        source: "ip".to_string(),
        city: json["city"].as_str().map(|s| s.to_string()),
    })
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
    //注册函数
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            window::resize_raf, window::snap_back_fast, 
            window::start_drag, window::end_drag, window::drag_move,//三个移动函数
            link_handler::open_url, link_handler::open_url_with_whitelist,//两个url跳转函数
            window::get_pending_urls, window::set_interacting, window::dismiss_island, window::set_current_view,
            window::sync_window_size, window::set_minimized, window::show_context_menu,
            window::set_capsule_rect, window::open_email_window, window::set_expanded,
            settings::open_settings, settings::get_settings, settings::save_settings,
            settings::get_lyric_offset_players, settings::set_lyric_offset_for_player,
            settings::set_lyric_offset_enabled, settings::delete_lyric_offset_player,
            betterncm::install_betterncm_support,
            media::media_play_pause, media::media_next, media::media_prev,
            ai::ai_get_settings, ai::ai_save_settings, ai::ai_detect_model_type,
            ai::ai_send_message, ai::ai_stop_generation, ai::ai_clear_history,
            settings::get_link_handlers, settings::save_link_handlers,
            link_handler::open_link_with_handler, link_handler::test_link_handler,
            ceverything::search_query, ceverything::search_execute,
            get_location, get_weather, refresh_weather, save_weather_city, settings::search_city,
            media::media_seek,
            media::media_volume_up, media::media_volume_down,
            media::media_get_volume, media::media_set_volume,
            settings::get_auto_start, settings::set_auto_start,
            settings::get_blacklist, settings::save_blacklist,
            settings::get_blacklist_enabled, settings::set_blacklist_enabled,
            settings::get_smtc_whitelist, settings::save_smtc_whitelist,
            settings::get_smtc_whitelist_enabled, settings::set_smtc_whitelist_enabled,
            settings::get_preview_updates, settings::set_preview_updates,
            settings::get_show_preview_toggle, settings::set_show_preview_toggle,
            updater::get_app_version, updater::check_for_updates, updater::download_and_install_update,
            logger::get_log_path, logger::open_log_dir, logger::get_log_level_num,
            logger::get_log_level, logger::set_log_level,
            sadb::sadb_start_mirroring, sadb::sadb_stop_mirroring,
            sadb::sadb_send_touch_event, sadb::sadb_send_scroll_event,
            sadb::sadb_send_keycode, sadb::sadb_inject_text,
            sadb::sadb_set_clipboard,
            sadb::sadb_connect_device, sadb::sadb_disconnect_device,
            tools::tools_check_adb, tools::tools_check_adb_devices, tools::tools_kill_adb_server, tools::tools_find_adb_in_path, tools::tools_download_adb,
            tools::tools_extract_adb, tools::tools_download_and_install_adb,
            email::is_email_configured, email::fetch_emails, email::refresh_emails, email::get_email_cache_dir, email::diagnose_email_cache, email::clear_email_cache,
            email::fetch_email_uid_list, email::fetch_email_metas_by_uids, email::fetch_email_bodies_by_uids, email::fetch_email_metas_and_bodies_by_uids, email::fetch_email_body_by_uid, email::read_email_body_by_uid,
        ])
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            
            let scale = window.scale_factor().unwrap_or(1.0);
            let screen_w = if let Ok(Some(monitor)) = window.current_monitor() {
                monitor.size().width as f64 / monitor.scale_factor()
            } else { 1920.0 };

            let capsule_w: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));
            let capsule_h: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));

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
            logger::set_level(&settings.log_level);
            logger::set_filter(settings.log_filter_tags.clone(), settings.log_filter_invert);
            let clipboard_enabled = Arc::new(AtomicBool::new(settings.clipboard_enabled));
            let pending_url: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
            let shortcut_key = Arc::new(Mutex::new(settings.shortcut_key.clone()));
            let search_shortcut = Arc::new(Mutex::new(settings.search_shortcut.clone()));
            let lyric_mode = Arc::new(Mutex::new(settings.lyric_mode.clone()));
            let lyric_offset_enabled = Arc::new(AtomicBool::new(settings.lyric_offset_enabled));
            // 按播放器存储的歌词补偿，启动时规范化键值
            let lyric_offsets_by_player: Arc<Mutex<std::collections::HashMap<String, i64>>> =
                Arc::new(Mutex::new(settings::normalize_lyric_offsets(&settings.lyric_offsets_by_player)));
            // 当前命中播放器 app_id（供 settings 子页高亮）
            let active_player_app_id: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
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
            
            //窗口态注册
            let email_expanded = Arc::new(AtomicBool::new(false));
            let agent_expanded = Arc::new(AtomicBool::new(false));
            let sadb_expanded = Arc::new(AtomicBool::new(false));
            let sadb_idle = Arc::new(AtomicBool::new(false));
            let sadb_mirroring = Arc::new(AtomicBool::new(false));
            let music_expanded = Arc::new(AtomicBool::new(false));
            let is_music = Arc::new(AtomicBool::new(false));
            let is_minimized = Arc::new(AtomicBool::new(false));
            let expand_anim_id = Arc::new(AtomicU64::new(0));
            let move_anim_id = Arc::new(AtomicU64::new(0));
            let indicator_color = Arc::new(Mutex::new(settings.indicator_color.clone()));
            let agent_window_size = Arc::new(Mutex::new(settings.agent_window_size.clone()));
            let link_handlers = Arc::new(Mutex::new(settings.link_handlers.clone()));
            let url_whitelist: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
            let auto_start = Arc::new(AtomicBool::new(settings.auto_start));
            let blacklist_processes: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(
                settings.blacklist_processes.iter().map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect()
            ));
            let blacklist_enabled = Arc::new(AtomicBool::new(settings.blacklist_enabled));
            let smtc_app_whitelist: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(
                settings.smtc_app_whitelist.iter().map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect()
            ));
            let smtc_whitelist_enabled = Arc::new(AtomicBool::new(settings.smtc_whitelist_enabled));
            let preview_updates = Arc::new(AtomicBool::new(settings.preview_updates));
            let show_preview_toggle = Arc::new(AtomicBool::new(settings.show_preview_toggle));
            let weather_city = Arc::new(Mutex::new(settings.weather_city.clone()));
            let weather_lat = Arc::new(Mutex::new(settings.weather_lat));
            let weather_lon = Arc::new(Mutex::new(settings.weather_lon));
            let weather_cache: Arc<Mutex<Option<WeatherResult>>> = Arc::new(Mutex::new(None));
            let weather_force_refresh = Arc::new(AtomicBool::new(true)); // 启动后立即获取
            let email_config = Arc::new(Mutex::new(Email {
                username: settings.email_username.clone(),
                auth: settings.email_auth.clone(),
                address: settings.email_address.clone(),
                port: settings.email_port,
            }));
            let email_poll_interval_secs = Arc::new(AtomicU64::new(settings.email_poll_interval_secs.max(1)));
            let latest_email_uid: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
            let email_shortcut = Arc::new(Mutex::new(settings.email_shortcut.clone()));
            let cached_email_metas: Arc<Mutex<Vec<email::EmailMeta>>> = Arc::new(Mutex::new(email::load_email_metas()));
            let cc_routes: Arc<Mutex<Vec<cc::CcRoute>>> = Arc::new(Mutex::new(settings.cc.clone()));
            
            media::update_smtc_whitelist(
                smtc_whitelist_enabled.load(Ordering::Relaxed),
                smtc_app_whitelist.lock().unwrap().clone(),
            );

            app.manage(IslandState {
                capsule_w: capsule_w.clone(),
                capsule_h: capsule_h.clone(),
                sadb_session: tokio::sync::Mutex::new(None),
                sadb_ip: Arc::new(Mutex::new(settings.sadb_ip.clone())),
                sadb_port: Arc::new(Mutex::new(settings.sadb_port)),
                adb_install_dir: Arc::new(Mutex::new(settings.adb_install_dir.clone())),
                adb_path: Arc::new(Mutex::new(settings.adb_path.clone())),
                is_notifying: is_notifying.clone(),
                is_expanded: is_expanded.clone(),
                is_dragging: is_dragging.clone(),
                is_interacting: is_interacting.clone(),
                clipboard_enabled: clipboard_enabled.clone(),
                pending_url: pending_url.clone(),
                shortcut_key: shortcut_key.clone(),
                search_shortcut: search_shortcut.clone(),
                lyric_mode: lyric_mode.clone(),
                lyric_offset_enabled: lyric_offset_enabled.clone(),
                lyric_offsets_by_player: lyric_offsets_by_player.clone(),
                active_player_app_id: active_player_app_id.clone(),
                current_view: current_view.clone(),
                email_expanded: email_expanded.clone(),
                agent_expanded: agent_expanded.clone(),
                sadb_expanded: sadb_expanded.clone(),
                sadb_idle: sadb_idle.clone(),
                sadb_mirroring: sadb_mirroring.clone(),
                music_expanded: music_expanded.clone(),
                is_minimized: is_minimized.clone(),
                expand_anim_id: expand_anim_id.clone(),
                move_anim_id: move_anim_id.clone(),
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
                blacklist_processes: blacklist_processes.clone(),
                blacklist_enabled: blacklist_enabled.clone(),
                smtc_app_whitelist: smtc_app_whitelist.clone(),
                smtc_whitelist_enabled: smtc_whitelist_enabled.clone(),
                preview_updates: preview_updates.clone(),
                show_preview_toggle: show_preview_toggle.clone(),
                email_config: email_config.clone(),
                email_poll_interval_secs: email_poll_interval_secs.clone(),
                latest_email_uid: latest_email_uid.clone(),
                email_shortcut: email_shortcut.clone(),
                cached_email_metas: cached_email_metas.clone(),
                cc_routes: cc_routes.clone(),
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

            // --- 搜索快捷键（从设置读取键位） ---
            {
                use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
                let win_search = window.clone();
                let hwnd_search = hwnd.0 as usize;
                let search_sc = settings.search_shortcut.clone();
                let _ = app.global_shortcut().on_shortcut(search_sc.as_str(), move |_app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        let h = HWND(hwnd_search as *mut _);
                        // 仅当窗口不在前台时才抢焦点，避免覆盖 webview 内部 input focus
                        let fg = unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
                        if fg != h {
                            window::force_foreground(h);
                            let _ = win_search.set_focus();
                            // 强制 DWM 重组合窗口，修复 WebView2 透明窗口黑屏问题
                            unsafe {
                                use windows::Win32::UI::WindowsAndMessaging::SetWindowPos;
                                let _ = SetWindowPos(
                                    h,
                                    None,
                                    0, 0, 0, 0,
                                    windows::Win32::UI::WindowsAndMessaging::SWP_NOMOVE
                                        | windows::Win32::UI::WindowsAndMessaging::SWP_NOSIZE
                                        | windows::Win32::UI::WindowsAndMessaging::SWP_NOZORDER
                                        | windows::Win32::UI::WindowsAndMessaging::SWP_NOACTIVATE
                                        | windows::Win32::UI::WindowsAndMessaging::SWP_FRAMECHANGED,
                                );
                            }
                        }
                        let _ = win_search.emit("activate-search", ());
                    }
                });
            }

            // --- 邮件快捷键 ---
            {
                use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
                let email_sc = settings.email_shortcut.clone();
                let app_h = app.handle().clone();
                logger::info("Shortcut", &format!("registering email shortcut: {}", email_sc));
                match app.global_shortcut().on_shortcut(email_sc.as_str(), move |_app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        window::open_email_window(app_h.clone(), None);
                    }
                }) {
                    Ok(_) => logger::info("Shortcut", "email shortcut registered ok"),
                    Err(e) => logger::info("Shortcut", &format!("email shortcut register FAILED: {e}")),
                }
            }

            // --- 鼠标监控线程 ---
            let win_m = window.clone();
            let drag_m = is_dragging.clone();
            let is_expanded_m = is_expanded.clone();
            let hwnd_raw = hwnd.0 as usize;
            let current_view_m = current_view.clone();
            let capsule_h_m = capsule_h.clone();
            let capsule_w_m = capsule_w.clone();
            let is_minimized_m = is_minimized.clone();
            thread::spawn(move || {
                let mut was_on_capsule = false;//穿透快照
                let mut was_in_zone = false;//顶部展开快照
                let hwnd = HWND(hwnd_raw as *mut _);
                //顶部展开
                loop {
                    if let Some((mx, my)) = window::get_cursor_pos() {
                        // 直接用实际窗口矩形判断鼠标是否在胶囊上
                        let Some(rect) = window::get_window_rect(hwnd) else { 
                            thread::sleep(Duration::from_millis(33)); 
                            continue 
                        };
                        let current_scale = win_m.scale_factor().unwrap_or(scale).max(0.1);
                        let fmx = (mx as f64 - rect.left as f64) / current_scale;
                        let fmy = (my as f64 - rect.top as f64) / current_scale;
                        let minimized = is_minimized_m.load(Ordering::Relaxed);
                        let dw = capsule_w_m.load(Ordering::Relaxed).max(if minimized { 60 } else { 1 }) as f64;
                        let dh = capsule_h_m.load(Ordering::Relaxed).max(if minimized { 4 } else { 1 }) as f64;
                        //logger::debug("LIB", &format!("{} {} {} {}",win_x + (win_w - dw) / 2.0 , win_x + (win_w + dw) / 2.0, win_y , win_y + dh));
                        // 大于左起的x 
                        let capsule_left = 0.0;
                        let capsule_right = capsule_left + dw;
                        let hit_top = if minimized { 5.0 } else { 10.0 };
                        let on_capsule = (fmx >= capsule_left) && (fmx <= capsule_right) && (fmy >= hit_top) && (fmy <= hit_top + dh);
                        let hit_on_capsule = on_capsule || drag_m.load(Ordering::Relaxed);

                        if hit_on_capsule && !was_on_capsule {
                            logger::debug("HitTest", "mouse ON capsule -> click-through OFF");
                            window::set_click_through(hwnd, false);
                            was_on_capsule = true;
                        } else if !hit_on_capsule && was_on_capsule {
                            logger::debug("HitTest", "mouse OFF capsule -> click-through ON");
                            window::set_click_through(hwnd, true);
                            was_on_capsule = false;
                        }

                        let v = current_view_m.lock().unwrap().as_str().to_string();
                        let allow_top_hover = (v == "time" || v == "lyric") && !minimized;
                        if !allow_top_hover {
                            was_in_zone = false;
                        }
                        if allow_top_hover && (!is_expanded_m.load(Ordering::Relaxed) || was_in_zone) {
                            let in_zone = (fmx >= capsule_left) && (fmx <= capsule_right) && (fmy >= 0.0) && (fmy <= 10.0);
                            if in_zone && !was_in_zone {
                                logger::debug("HitTest", "in_zone");
                                was_in_zone = true;
                                is_expanded_m.store(true, Ordering::Relaxed);
                                let _ = win_m.emit("set-expand", true);

                            }else if was_in_zone && !in_zone && !hit_on_capsule {
                                logger::debug("HitTest", "in_zone -> not_in_zone");
                                was_in_zone = false;
                                is_expanded_m.store(false, Ordering::Relaxed);
                                let _ = win_m.emit("set-expand", false);
                            } 
                        }
                    }
                    thread::sleep(Duration::from_millis(33));
                }
            });

            // --- 黑名单监控：全屏扫描线程（慢，独立跑，结果存原子变量）---
            let blacklist_fs_cache = Arc::new(AtomicBool::new(false));
            {
                let blacklist = blacklist_processes.clone();
                let bl_enabled = blacklist_enabled.clone();
                let fs_cache = blacklist_fs_cache.clone();
                thread::Builder::new().name("bl-fullscreen-scan".into()).spawn(move || {
                    loop {
                        thread::sleep(Duration::from_millis(800));
                        if !bl_enabled.load(Ordering::Relaxed) {
                            fs_cache.store(false, Ordering::Relaxed);
                            continue;
                        }
                        let list = blacklist.lock().unwrap().clone();
                        let found = if list.is_empty() {
                            false
                        } else {
                            window::is_any_blacklisted_fullscreen(&list)
                        };
                        fs_cache.store(found, Ordering::Relaxed);
                    }
                }).ok();
            }

            // --- 黑名单监控：前台进程检测 + 隐藏/显示线程（快，200ms）---
            {
                let blacklist = blacklist_processes.clone();
                let bl_enabled = blacklist_enabled.clone();
                let fs_cache = blacklist_fs_cache.clone();
                let hwnd_bl = hwnd.0 as usize;
                thread::Builder::new().name("bl-monitor".into()).spawn(move || {
                    let hwnd = HWND(hwnd_bl as *mut _);
                    let mut hidden = false;
                    loop {
                        thread::sleep(Duration::from_millis(200));
                        if !bl_enabled.load(Ordering::Relaxed) {
                            if hidden {
                                unsafe { let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(hwnd, windows::Win32::UI::WindowsAndMessaging::SW_SHOWNOACTIVATE); }
                                hidden = false;
                            }
                            continue;
                        }
                        let list = blacklist.lock().unwrap().clone();
                        if list.is_empty() {
                            if hidden {
                                unsafe { let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(hwnd, windows::Win32::UI::WindowsAndMessaging::SW_SHOWNOACTIVATE); }
                                hidden = false;
                            }
                            continue;
                        }
                        let fg_match = window::get_foreground_process_name()
                            .map(|n| list.iter().any(|b| n == *b))
                            .unwrap_or(false);
                        let fs_match = fs_cache.load(Ordering::Relaxed);
                        let should_hide = fg_match || fs_match;
                        if should_hide && !hidden {
                            if let Some(ref name) = window::get_foreground_process_name() {
                                crate::logger::info("Blacklist", &format!("hiding island: fg_process='{}'", name));
                            }
                            unsafe { let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(hwnd, windows::Win32::UI::WindowsAndMessaging::SW_HIDE); }
                            hidden = true;
                        } else if !should_hide && hidden {
                            crate::logger::info("Blacklist", "showing island: fg_process no longer blacklisted");
                            unsafe { let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(hwnd, windows::Win32::UI::WindowsAndMessaging::SW_SHOWNOACTIVATE); }
                            hidden = false;
                        }
                    }
                }).ok();
            }

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
            let current_view_cb = current_view.clone();

            thread::spawn(move || {
                logger::info("Clipboard", "polling thread started");
                let mut last_text = String::new();
                let mut logged_disabled = false;
                loop {
                    thread::sleep(Duration::from_millis(1200));
                    if !cb_enabled.load(Ordering::Relaxed) {
                        if !logged_disabled {
                            logger::debug("Clipboard", "clipboard_enabled = false, skipping");
                            logged_disabled = true;
                        }
                        continue;
                    }
                    logged_disabled = false;
                    let read = clipboard::read_clipboard_text();
                    if read.is_none() {
                        //logger::debug("Clipboard", "read_clipboard_text returned None");
                        continue;
                    }
                    let text = read.unwrap();
                    if text != last_text {
                        last_text = text.clone();
                        logger::debug("Clipboard", &format!("text changed (len={}): {:?}", text.len(), &text[..text.len().min(200)]));
                        let urls = clipboard::extract_urls(&text);
                        logger::debug("Clipboard", &format!("extract_urls => {} url(s)", urls.len()));
                        if !urls.is_empty() {
                            logger::info("Clipboard", &format!("detected {} url(s): {:?}", urls.len(), urls));
                            *pending_url_cb.lock().unwrap() = urls.clone();
                            if *current_view_cb.lock().unwrap() == "email" {
                                continue;
                            }
                            noti_cb.store(true, Ordering::Relaxed);
                            exp_cb.store(true, Ordering::Relaxed);
                            let _ = win_cb.set_size(tauri::LogicalSize::new(WIN_W, WIN_H_DEFAULT));
                            let _ = win_cb.emit("set-expand", true);
                            let _ = win_cb.emit("clipboard-urls", urls.clone());
                        }
                    }
                }
            });

            // --- 邮件 UID 轮询线程 ---
            let app_handle_email = app.handle().clone();
            let win_email = window.clone();
            let noti_email = is_notifying.clone();
            let exp_email = is_expanded.clone();
            let email_config_t = email_config.clone();
            let email_interval_t = email_poll_interval_secs.clone();
            let latest_email_uid_t = latest_email_uid.clone();
            let cached_metas_t = cached_email_metas.clone();
            let current_view_email = current_view.clone();

            thread::spawn(move || {
                logger::info("EmailPoll", "polling thread started");
                let mut is_configured = false;
                thread::sleep(Duration::from_secs(3));
                loop {
                    let interval = email_interval_t.load(Ordering::Relaxed).max(1); 
                    thread::sleep(Duration::from_secs(interval));
                    let config = email_config_t.lock().unwrap().clone();
                    if !config.is_configured() {
                        if is_configured {
                            let _ = app_handle_email.emit("email-configured", false);//状态更改才发
                        }
                        is_configured = false;
                        thread::sleep(Duration::from_secs(2));
                        continue;
                    }else if !is_configured && config.is_configured() {//状态更改
                        is_configured = true;//配置快照 避免重复发送
                        let _ = app_handle_email.emit("email-configured", true);
                        thread::sleep(Duration::from_secs(1));
                        logger::info("EmailPoll", "initial fetch: pulling latest 10 emails");
                        let metas = tauri::async_runtime::block_on(config.fetch_latest_emails());
                        logger::info("EmailPoll", &format!("initial fetch done: {} emails cached", metas.len()));
                        if let Some(first) = metas.first() {
                            *latest_email_uid_t.lock().unwrap() = Some(first.uid.clone());
                        }
                        // 保存到内存 + 磁盘
                        if metas.is_empty() {
                            logger::info("EmailPoll", "initial fetch returned 0 metas, keeping existing cache");
                        } else {
                            let mut cached = cached_metas_t.lock().unwrap();
                            email::merge_email_metas(&mut cached, &metas);
                            email::save_email_metas(&cached);
                            let _ = app_handle_email.emit("email-updated", ());
                        }
                        continue;
                    }
                    


                    // 增量检查：对比服务器最新 UID 与本地已知 UID
                    let uid = tauri::async_runtime::block_on(config.get_latest_uid());
                    let Some(uid) = uid else { continue; };

                    let mut latest = latest_email_uid_t.lock().unwrap();
                    let need_fetch = match latest.as_ref() {
                        None => { *latest = Some(uid.clone()); true }
                        Some(current) if current != &uid => {
                            logger::info("EmailPoll", &format!("uid changed: {} -> {}", current, uid));
                            *latest = Some(uid.clone());
                            true
                        }
                        _ => false,
                    };
                    drop(latest);

                    if need_fetch {
                        let metas = tauri::async_runtime::block_on(config.fetch_latest_emails());
                        logger::info("EmailPoll", &format!("fetch done: {} emails", metas.len()));

                        let old_top = cached_metas_t.lock().unwrap().first().map(|m| m.uid.clone());
                        if metas.is_empty() {
                            logger::info("EmailPoll", "fetch returned 0 metas, keeping existing cache");
                            continue;
                        }
                        {
                            let mut cached = cached_metas_t.lock().unwrap();
                            email::merge_email_metas(&mut cached, &metas);
                            email::save_email_metas(&cached);
                        }
                        let _ = app_handle_email.emit("email-updated", ());

                        // 仅当有真正新邮件时发通知（新最大 UID > 旧最大 UID）
                        let new_top = metas.first().map(|m| m.uid.clone());
                        if new_top != old_top {
                            if *current_view_email.lock().unwrap() == "email" {
                                continue;
                            }
                            noti_email.store(true, Ordering::Relaxed);
                            exp_email.store(true, Ordering::Relaxed);
                            let _ = win_email.set_size(tauri::LogicalSize::new(WIN_W, WIN_H_DEFAULT));
                            let _ = win_email.emit("set-expand", true);
                            let _ = win_email.emit("email-notice", serde_json::json!({
                                "uid": uid,
                                "message": "收到新邮件"
                            }));
                        }
                    }
                }
            });

            // --- Claude Code 本地通知服务器 ---
            let win_cc = window.clone();
            let noti_cc = is_notifying.clone();
            let exp_cc = is_expanded.clone();
            let cc_routes_t = cc_routes.clone();
            thread::spawn(move || {
                cc::start_server(win_cc, noti_cc, exp_cc, cc_routes_t);
            });

            // --- 天气后台线程 ---
            let win_weather = window.clone();
            let weather_city_t = weather_city.clone();
            let weather_lat_t = weather_lat.clone();
            let weather_lon_t = weather_lon.clone();
            let weather_cache_t = weather_cache.clone();
            let weather_refresh_t = weather_force_refresh.clone();

            thread::spawn(move || {
                const WEATHER_INTERVAL_SECS: u64 = 20 * 60; // 正常成功间隔：20 分钟
                const WEATHER_RETRY_SECS: u64 = 60;          // 连续失败时的快速重试间隔：1 分钟
                const WEATHER_COOLDOWN_SECS: u64 = 30 * 60;  // 达到上限后的冷却时长：30 分钟
                const WEATHER_MAX_FAILURES: u32 = 3;         // 触发冷却的连续失败次数

                let mut last_fetch = Instant::now() - Duration::from_secs(WEATHER_INTERVAL_SECS);
                // 当前「快速重试窗口」内已失败次数（0..=WEATHER_MAX_FAILURES）
                let mut consecutive_failures: u32 = 0;
                // 下次允许发起请求的最早时间点；None 表示不受退避限制
                let mut next_retry_at: Option<Instant> = None;

                loop {
                    // 手动强制刷新：彻底重置失败状态，立即放行
                    let force = weather_refresh_t.compare_exchange(
                        true, false, Ordering::SeqCst, Ordering::Relaxed,
                    ).is_ok();
                    if force {
                        consecutive_failures = 0;
                        next_retry_at = None;
                    }

                    let now = Instant::now();
                    let retry_gate_passed = next_retry_at.map(|t| now >= t).unwrap_or(true);
                    let interval_elapsed = last_fetch.elapsed() >= Duration::from_secs(WEATHER_INTERVAL_SECS);
                    let should_fetch = force || (retry_gate_passed && interval_elapsed);

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
                                consecutive_failures = 0;
                                next_retry_at = None;
                                println!("[Weather] 天气更新成功: {} {} {}°C", result.city, result.desc, result.temp);
                            }
                            Err(e) => {
                                consecutive_failures += 1;
                                if consecutive_failures >= WEATHER_MAX_FAILURES {
                                    next_retry_at = Some(now + Duration::from_secs(WEATHER_COOLDOWN_SECS));
                                    consecutive_failures = 0; // 冷却结束后重新给 3 次机会
                                    println!(
                                        "[Weather] 连续 {} 次失败，进入 {} 秒冷却后再重试: {}",
                                        WEATHER_MAX_FAILURES, WEATHER_COOLDOWN_SECS, e,
                                    );
                                } else {
                                    next_retry_at = Some(now + Duration::from_secs(WEATHER_RETRY_SECS));
                                    println!(
                                        "[Weather] 天气获取失败 ({}/{}), {} 秒后重试: {}",
                                        consecutive_failures, WEATHER_MAX_FAILURES, WEATHER_RETRY_SECS, e,
                                    );
                                }
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
                match updater::check_for_updates(app_handle_update.clone(), None) {
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
            let is_music_media = is_music.clone();
            // 歌词补偿：总开关 + 按播放器表 + 当前命中 app_id；以及 AppHandle 用于持久化/广播事件
            let lyric_offset_enabled_media = lyric_offset_enabled.clone();
            let lyric_offsets_media = lyric_offsets_by_player.clone();
            let active_player_media = active_player_app_id.clone();
            let app_handle_media = app.handle().clone();

            // 歌词异步获取：用 Arc<Mutex> 共享结果 + 代数计数器防止竞态
            let lyrics_result: Arc<Mutex<Option<(u64, Vec<lyrics::LyricLine>, bool)>>> = Arc::new(Mutex::new(None));
            // (generation, lyrics, not_found)
            use std::sync::atomic::AtomicU64 as AtomicU64Import;
            let lyrics_generation: Arc<AtomicU64Import> = Arc::new(AtomicU64Import::new(0));
            // 封面代数计数器，防止旧封面覆盖新歌
            let thumb_generation: Arc<AtomicU64Import> = Arc::new(AtomicU64Import::new(0));

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
                // SMTC 会话丢失宽限期：部分播放器（如汽水音乐）在自动切歌瞬间会短暂关闭
                // 并重建会话，若立即发 lyric-update:null 会导致前端从歌词视图回退到时间视图。
                // 轮询周期 80ms，阈值 63 次 ≈ 5s，确认确实没有任何音乐会话后再关闭视图。
                let mut no_session_count: u32 = 0;
                const NO_SESSION_GRACE_CYCLES: u32 = 63;

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
                            crate::logger::warn("Lyrics", "playback state=stopped reason=lyric_mode_off");
                            was_playing = false;
                            last_is_playing = false;
                            current_track.clear();
                            is_music_media.store(false, Ordering::Relaxed);
                            let _ = win_media.emit("lyric-update", serde_json::json!(null));
                        }
                        continue;
                    }

                    let info = media::get_smtc_media_info();
                    let (status, media_info, position_ms_raw, is_playing, raw_app_id) = match info {
                        Some(v) => {
                            // 拿到有效会话，重置宽限期计数
                            no_session_count = 0;
                            v
                        }
                        None => {
                            if was_playing {
                                // 会话短暂丢失：先走宽限期，避免切歌瞬间被误判为停止播放
                                no_session_count = no_session_count.saturating_add(1);
                                if no_session_count < NO_SESSION_GRACE_CYCLES {
                                    continue;
                                }
                                crate::logger::warn(
                                    "Lyrics",
                                    "playback state=stopped reason=no_smtc_session (grace expired)",
                                );
                                no_session_count = 0;
                                was_playing = false;
                                last_is_playing = false;
                                current_track.clear();
                                is_music_media.store(false, Ordering::Relaxed);
                                let _ = win_media.emit("lyric-update", serde_json::json!(null));
                            }
                            continue;
                        }
                    };
                    // Closed (4) 表示会话已关闭，立即清空状态通知前端
                    if status == 4 {
                        if was_playing {
                            crate::logger::warn("Lyrics", "playback state=stopped reason=smtc_session_closed");
                            was_playing = false;
                            last_is_playing = false;
                            current_track.clear();
                            is_music_media.store(false, Ordering::Relaxed);
                            let _ = win_media.emit("lyric-update", serde_json::json!(null));
                        }
                        continue;
                    }

                    let app_id = settings::normalize_app_id(&raw_app_id);

                    // --- 活跃播放器变化：更新 state 并广播，供 settings 子页高亮 ---
                    {
                        let mut active = active_player_media.lock().unwrap();
                        let changed = active.as_deref() != Some(app_id.as_str());
                        if changed {
                            *active = Some(app_id.clone());
                            drop(active);
                            let _ = app_handle_media.emit(
                                "lyric-offset-active-player-changed",
                                serde_json::json!({ "app_id": app_id }),
                            );
                        }
                    }

                    // --- 自动发现：新播放器首次出现时，默认 0ms 入表并落盘广播 ---
                    let offset_ms = {
                        let needs_insert = !app_id.is_empty() && {
                            let map = lyric_offsets_media.lock().unwrap();
                            !map.contains_key(&app_id)
                        };
                        if needs_insert {
                            {
                                let mut map = lyric_offsets_media.lock().unwrap();
                                map.entry(app_id.clone()).or_insert(0);
                            }
                            // 持久化（通过 Tauri State 访问完整配置）
                            let state_ref = app_handle_media.state::<IslandState>();
                            let data = settings::build_settings_data(&state_ref);
                            if let Err(e) = settings::save_settings_to_file(&data) {
                                crate::logger::warn(
                                    "Lyrics",
                                    &format!("persist lyric_offsets_by_player failed: {}", e),
                                );
                            }
                            let _ = app_handle_media.emit(
                                "lyric-offset-players-changed",
                                serde_json::json!({ "new_app_id": app_id }),
                            );
                        }
                        let map = lyric_offsets_media.lock().unwrap();
                        *map.get(&app_id).unwrap_or(&0)
                    };

                    let offset_enabled = lyric_offset_enabled_media.load(Ordering::Relaxed);
                    let position_ms = if offset_enabled {
                        position_ms_raw.saturating_add(offset_ms).max(0)
                    } else {
                        position_ms_raw
                    };

                    // 播放/暂停状态变化
                    if is_playing != last_is_playing {
                        last_is_playing = is_playing;
                        crate::logger::info("Lyrics", &format!(
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
                            crate::logger::info("Lyrics", &format!(
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
                        crate::logger::info("Lyrics", &format!(
                            "\nsmtc: track changed title='{}' artist='{}' genre='{}' duration_ms={} position_ms={} is_playing={} offset_enabled={} offset_ms={}",
                            media_info.title, media_info.artist, media_info.genre,
                            media_info.duration_ms, position_ms_raw, is_playing,
                            offset_enabled, offset_ms
                        ));
                        current_track = track_key.clone();
                        media::dump_smtc_session("");
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
                            "duration_ms": media_info.duration_ms,
                            "seekable": media_info.seekable
                        }));

                        // 异步获取封面（独立线程，不阻塞轮询）
                        {
                            let win_thumb = win_media.clone();
                            let thumb_gen_val = thumb_generation.fetch_add(1, Ordering::Relaxed) + 1;
                            let thumb_gen_ref = thumb_generation.clone();
                            thread::Builder::new()
                                .name("thumb-fetch".into())
                                .spawn(move || {
                                    // 最多重试 3 次，每次间隔递增（150ms / 400ms / 800ms）
                                    let delays = [150u64, 400, 800];
                                    for (i, &delay_ms) in delays.iter().enumerate() {
                                        thread::sleep(std::time::Duration::from_millis(delay_ms));
                                        // 代数已变说明新歌切换，放弃
                                        if thumb_gen_ref.load(Ordering::Relaxed) != thumb_gen_val {
                                            return;
                                        }
                                        if let Some(thumb) = media::get_smtc_thumbnail() {
                                            if thumb_gen_ref.load(Ordering::Relaxed) == thumb_gen_val {
                                                let _ = win_thumb.emit("media-thumbnail", serde_json::json!({
                                                    "thumbnail": thumb
                                                }));
                                            }
                                            return;
                                        }
                                        let _ = i; // suppress unused warning on last iter
                                    }
                                }).ok();
                        }

                        // 异步获取歌词（不阻塞主循环，LRCLIB 和网易云并行）
                        if mode == "lyric" {
                            let title = media_info.title.clone();
                            let artist = media_info.artist.clone();
                            let album_title = media_info.album_title.clone();
                            let album_artist = media_info.album_artist.clone();
                            let duration_ms = media_info.duration_ms;
                            let genre = media_info.genre.clone();
                            let gen = current_gen;
                            let result_ref = lyrics_result.clone();
                            let gen_ref = lyrics_generation.clone();
                            fetch_pending = true;
                            crate::logger::info("Lyrics", &format!(
                                "lyric fetch start gen={} title='{}' artist='{}' genre='{}' strategy=genre_ncmid",
                                gen, title, artist, genre
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
                                    &album_title,
                                    &album_artist,
                                    &raw_app_id,
                                    duration_ms,
                                    &genre,
                                    gen_ref.clone(),
                                    gen,
                                );
                                // 只有当前代才写入结果；已有 found 结果时不允许被 not_found 覆盖
                                if gen_ref.load(Ordering::Relaxed) == gen {
                                    let not_found = fetched_lyrics.is_none();
                                    let line_count = fetched_lyrics.as_ref().map(|v| v.len()).unwrap_or(0);
                                    let mut guard = result_ref.lock().unwrap_or_else(|e| e.into_inner());
                                    let already_found = guard.as_ref()
                                        .map(|(g, _, nf)| *g == gen && !nf)
                                        .unwrap_or(false);
                                    if already_found && not_found {
                                        crate::logger::warn("Lyrics", &format!(
                                            "lyric fetch skip stale not_found gen={} (already have result)",
                                            gen
                                        ));
                                    } else {
                                        crate::logger::info("Lyrics", &format!(
                                            "lyric fetch done gen={} found={} lines={}",
                                            gen, !not_found, line_count
                                        ));
                                        *guard = Some((gen, fetched_lyrics.unwrap_or_default(), not_found));
                                    }
                                } else {
                                    crate::logger::warn("Lyrics", &format!(
                                        "lyric fetch drop stale gen={} current_gen={}",
                                        gen,
                                        gen_ref.load(Ordering::Relaxed)
                                    ));
                                }
                            }).ok();
                        }
                    }

                    was_playing = true;

                    // 当 SMTC 不提供时长时，用最后一句歌词时间 +5s 做估算
                    let effective_duration_ms = if media_info.duration_ms > 0 {
                        media_info.duration_ms
                    } else if let Some(last) = current_lyrics.last() {
                        last.time_ms + 5000
                    } else {
                        0
                    };

                    if mode == "lyric" {
                        // 构建歌词文本和附近歌词（文本去重，但始终发送位置）
                        let (text_val, nearby_json, line_tokens, line_start_ms, next_line_time_ms) = if fetch_pending && current_lyrics.is_empty() {
                            // 正在获取歌词中
                            (serde_json::json!("♪"), None, None, None, None)
                        } else if lyrics_not_found || (!fetch_pending && current_lyrics.is_empty()) {
                            // 歌词未找到
                            (serde_json::json!(null), None, None, None, None)
                        } else if let Some(line_idx) = current_lyrics.iter().rposition(|l| l.time_ms <= position_ms) {
                            let line = &current_lyrics[line_idx];
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
                            let tokens = if line.tokens.is_empty() {
                                None
                            } else {
                                Some(line.tokens.clone())
                            };
                            let next_switch_ms = if line_idx + 1 < current_lyrics.len() {
                                current_lyrics[line_idx + 1].time_ms
                            } else {
                                line.end_time_ms
                            };
                            (serde_json::json!(line.text), nearby, tokens, Some(line.time_ms), Some(next_switch_ms))
                        } else {
                            let nearby = lyrics::get_nearby_lyrics(&current_lyrics, position_ms);
                            let nearby_json = Some(nearby.iter().map(|(text, is_current)| {
                                serde_json::json!({"text": text, "is_current": is_current})
                            }).collect::<Vec<_>>());
                            (serde_json::json!("♪"), nearby_json, None, None, None)
                        };

                        // 始终发送，确保进度条持续更新
                        let mut payload = serde_json::json!({
                            "text": text_val,
                            "title": media_info.title,
                            "artist": media_info.artist,
                            "genre": media_info.genre,
                            "position_ms": position_ms,
                            "duration_ms": effective_duration_ms,
                            "is_playing": is_playing,
                            "seekable": media_info.seekable
                        });
                        if let Some(nearby) = nearby_json {
                            payload["nearby_lyrics"] = serde_json::json!(nearby);
                        }
                        if let Some(tokens) = line_tokens {
                            payload["tokens"] = serde_json::json!(tokens);
                        }
                        if let Some(v) = line_start_ms {
                            payload["line_start_ms"] = serde_json::json!(v);
                        }
                        if let Some(v) = next_line_time_ms {
                            payload["next_line_time_ms"] = serde_json::json!(v);
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
                            "duration_ms": effective_duration_ms,
                            "is_playing": is_playing,
                            "seekable": media_info.seekable
                        }));
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[allow(dead_code)]
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
    pub capsule_w: Arc<AtomicU64>,
    pub capsule_h: Arc<AtomicU64>,
    pub is_notifying: Arc<AtomicBool>,
    pub is_expanded: Arc<AtomicBool>,
    pub is_dragging: Arc<AtomicBool>,
    pub is_interacting: Arc<AtomicBool>,
    pub clipboard_enabled: Arc<AtomicBool>,
    pub pending_url: Arc<Mutex<Vec<String>>>,
    pub shortcut_key: Arc<Mutex<String>>,
    pub search_shortcut: Arc<Mutex<String>>,
    pub lyric_mode: Arc<Mutex<String>>, // "off" | "info" | "lyric"
    pub lyric_offset_enabled: Arc<AtomicBool>,
    /// 按 SMTC app_id 存储的歌词补偿（ms），key 已规范化为小写
    pub lyric_offsets_by_player: Arc<Mutex<std::collections::HashMap<String, i64>>>,
    /// 当前命中的播放器 app_id（小写），供 settings 子页高亮
    pub active_player_app_id: Arc<Mutex<Option<String>>>,
    pub current_view: Arc<Mutex<String>>, // "time" | "notice" | "urls" | "lyric" | "agent"
    pub email_expanded: Arc<AtomicBool>,
    pub agent_expanded: Arc<AtomicBool>,
    pub music_expanded: Arc<AtomicBool>,
    pub is_minimized: Arc<AtomicBool>,
    pub expand_anim_id: Arc<AtomicU64>,
    pub move_anim_id: Arc<AtomicU64>,
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
    // 黑名单进程列表（小写）
    pub blacklist_processes: Arc<Mutex<Vec<String>>>,
    // 黑名单功能总开关
    pub blacklist_enabled: Arc<AtomicBool>,
    // SMTC app_id 白名单
    pub smtc_app_whitelist: Arc<Mutex<Vec<String>>>,
    pub smtc_whitelist_enabled: Arc<AtomicBool>,
    // 预览更新通道开关
    pub preview_updates: Arc<AtomicBool>,
    // 是否显示预览版开关（UI 可见性）
    pub show_preview_toggle: Arc<AtomicBool>,
    // 邮件
    pub email_config: Arc<Mutex<Email>>,
    pub email_poll_interval_secs: Arc<AtomicU64>,
    pub latest_email_uid: Arc<Mutex<Option<String>>>,
    pub email_shortcut: Arc<Mutex<String>>,
    pub cached_email_metas: Arc<Mutex<Vec<email::EmailMeta>>>,
    // Claude Code 通知路由
    pub cc_routes: Arc<Mutex<Vec<cc::CcRoute>>>,
    // ADB / 屏幕镜像 相关
    pub(crate) sadb_session: tokio::sync::Mutex<Option<sadb::SessionHandle>>,
    pub sadb_ip: Arc<Mutex<String>>,
    pub sadb_port: Arc<Mutex<u16>>,
    pub adb_install_dir: Arc<Mutex<String>>,
    pub adb_path: Arc<Mutex<String>>,
    pub sadb_expanded: Arc<AtomicBool>,
    /// 待机面板展开中（已点击展开但尚未开始镜像，或镜像结束后回退）
    pub sadb_idle: Arc<AtomicBool>,
    /// 镜像流正常推送中（视频帧在传输），用于允许拖动不回弹
    pub sadb_mirroring: Arc<AtomicBool>,
}

unsafe impl Send for IslandState {}
unsafe impl Sync for IslandState {}
