use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::*;
use crate::{logger, IslandState, WIN_W, WIN_H_DEFAULT, TOP_MARGIN, MINIMIZED_W, MINIMIZED_H, SNAP_DURATION_MS, SNAP_FRAME_MS};


const EMAIL_VIEW_W: f64 = 620.0;
const TAG: &str = "Window";

pub(crate) fn get_foreground_process_name() -> Option<String> {
    use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    use windows::core::PWSTR;
    unsafe {
        let fg = GetForegroundWindow();
        if fg.0.is_null() { return None; }
        let mut pid: u32 = 0;
        windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(fg, Some(&mut pid));
        if pid == 0 { return None; }
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 260];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, windows::Win32::System::Threading::PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = windows::Win32::Foundation::CloseHandle(handle);
        ok.ok()?;
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit('\\').next().map(|s| s.to_lowercase())
    }
}

/// 强制窗口成为前台窗口，绕过 Windows 前台锁（AttachThreadInput 技巧）
pub(crate) fn force_foreground(hwnd: HWND) {
    use windows::Win32::System::Threading::{GetCurrentThreadId, AttachThreadInput};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow, BringWindowToTop,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
    unsafe {
        let fg = GetForegroundWindow();
        if fg.0.is_null() {
            let _ = SetForegroundWindow(hwnd);
            let _ = BringWindowToTop(hwnd);
            let _ = SetFocus(Some(hwnd));
            return;
        }
        let fg_thread = GetWindowThreadProcessId(fg, None);
        let cur_thread = GetCurrentThreadId();
        let target_thread = GetWindowThreadProcessId(hwnd, None);
        if fg_thread != 0 && fg_thread != cur_thread {
            let _ = AttachThreadInput(fg_thread, cur_thread, true);
        }
        if target_thread != 0 && target_thread != cur_thread {
            let _ = AttachThreadInput(target_thread, cur_thread, true);
        }
        let _ = SetForegroundWindow(hwnd);
        let _ = BringWindowToTop(hwnd);
        let _ = SetFocus(Some(hwnd));
        if fg_thread != 0 && fg_thread != cur_thread {
            let _ = AttachThreadInput(fg_thread, cur_thread, false);
        }
        if target_thread != 0 && target_thread != cur_thread {
            let _ = AttachThreadInput(target_thread, cur_thread, false);
        }
    }
}


pub(crate) fn is_any_blacklisted_fullscreen(blacklist: &[String]) -> bool {
    use windows::Win32::Foundation::{LPARAM, RECT};
    use windows::core::BOOL;
    use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
    use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows::core::PWSTR;

    struct Ctx<'a> {
        blacklist: &'a [String],
        found: bool,
    }

    unsafe extern "system" fn callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let ctx = &mut *(lparam.0 as *mut Ctx);
        if ctx.found { return BOOL(0); }

        if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
            return BOOL(1);
        }

        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() { return BOOL(1); }

        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if !GetMonitorInfoW(monitor, &mut mi).as_bool() { return BOOL(1); }

        let mr = mi.rcMonitor;
        if rect.left > mr.left || rect.top > mr.top || rect.right < mr.right || rect.bottom < mr.bottom {
            return BOOL(1);
        }

        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 { return BOOL(1); }

        let handle = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(h) => h,
            Err(_) => return BOOL(1),
        };
        let mut buf = [0u16; 260];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = windows::Win32::Foundation::CloseHandle(handle);
        if ok.is_err() { return BOOL(1); }

        let path = String::from_utf16_lossy(&buf[..len as usize]);
        let name = path.rsplit('\\').next().map(|s| s.to_lowercase()).unwrap_or_default();
        if ctx.blacklist.iter().any(|b| *b == name) {
            ctx.found = true;
            return BOOL(0);
        }
        BOOL(1)
    }

    let mut ctx = Ctx { blacklist, found: false };
    unsafe {
        let _ = EnumWindows(Some(callback), LPARAM(&mut ctx as *mut _ as isize));
    }
    ctx.found
}

pub(crate) fn ease_out_cubic(t: f64) -> f64 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

pub(crate) fn get_cursor_pos() -> Option<(i32, i32)> {
    use windows::Win32::Foundation::POINT;
    let mut pt = POINT { x: 0, y: 0 };
    unsafe { if GetCursorPos(&mut pt).is_ok() { Some((pt.x, pt.y)) } else { None } }
}

pub(crate) fn get_window_rect(hwnd: HWND) -> Option<windows::Win32::Foundation::RECT> {
    let mut rect = windows::Win32::Foundation::RECT::default();
    unsafe {
        if GetWindowRect(hwnd, &mut rect).is_ok() { Some(rect) } else { None }
    }
}


#[tauri::command]
pub(crate) fn set_capsule_rect(state: tauri::State<'_, IslandState>, width: u64, height: u64) {
    state.capsule_w.store(width, Ordering::Relaxed);
    state.capsule_h.store(height, Ordering::Relaxed);
    //打日志吃io性能,不打了.有报错自己把这里去掉注释看
    //logger::debug(TAG,&format!("recieve size from webview, width: {}, height: {}", width, height));
}



pub(crate) fn set_click_through(hwnd: HWND, through: bool) {
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

pub(crate) fn snap_back(window: &tauri::WebviewWindow, from_x: f64, from_y: f64, to_x: f64, to_y: f64) {
    logger::debug("Window", &format!("snap_back"));
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

//专门给垂直展开用,避免左右展开弹跳位移,
pub(crate) fn animate_window_height(
    hwnd: HWND,
    scale: f64,
    from_h: f64,
    to_h: f64,
    win_w: f64,
    duration_ms: f64,
    anim_id: Arc<AtomicU64>,
    my_gen: u64,
) {
    let phys_w = (win_w * scale).round() as i32;
    logger::debug("WindowAnim", &format!("animate_window_height start: gen={my_gen}, from_h={from_h:.1}, to_h={to_h:.1}, win_w={win_w:.1}, duration_ms={duration_ms:.0}"));
    let start = Instant::now();
    loop {
        if anim_id.load(Ordering::Relaxed) != my_gen {
            logger::debug("WindowAnim", &format!("animate_window_height interrupted: gen={my_gen}, current_gen={}", anim_id.load(Ordering::Relaxed)));
            return;
        }
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        let p = (elapsed / duration_ms).min(1.0);
        let t = ease_out_cubic(p);
        let cur_h = from_h + (to_h - from_h) * t;
        unsafe {
            let _ = SetWindowPos(
                hwnd, None,
                0, 0,
                phys_w,
                (cur_h * scale).round() as i32,
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOMOVE,
            );
        }
        if p >= 1.0 { break; }
        thread::sleep(Duration::from_millis(SNAP_FRAME_MS));
    }
    let _ = anim_id.compare_exchange(my_gen, 0, Ordering::Relaxed, Ordering::Relaxed);
}

/// 动画插值窗口尺寸和位置，duration_ms 与 CSS transition 同步
pub(crate) fn animate_resize(
    window: &tauri::WebviewWindow,
    from_x: f64, from_y: f64, from_w: f64, from_h: f64,
    to_x: f64, to_y: f64, to_w: f64, to_h: f64,
    duration_ms: f64,
) {
    logger::debug(TAG, &format!("animate_resize: to_w: {}, to_h: {}", to_w, to_h));
    let scale = window.scale_factor().unwrap_or(1.0);
    let hwnd = HWND(window.hwnd().unwrap().0);
    let start = Instant::now();
    loop {
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        let p = (elapsed / duration_ms).min(1.0);
        let t = ease_out_cubic(p);

        let cur_w = from_w + (to_w - from_w) * t;
        let cur_h = from_h + (to_h - from_h) * t;
        let cur_x = from_x + (to_x - from_x) * t;
        let cur_y = from_y + (to_y - from_y) * t;

        unsafe {
            let _ = SetWindowPos(
                hwnd, None,
                (cur_x * scale).round() as i32,
                (cur_y * scale).round() as i32,
                (cur_w * scale).round() as i32,
                (cur_h * scale).round() as i32,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }

        if p >= 1.0 { break; }
        thread::sleep(Duration::from_millis(SNAP_FRAME_MS));
    }
}

#[tauri::command]
pub fn start_drag(state: tauri::State<'_, IslandState>) {
    state.is_dragging.store(true, Ordering::Relaxed);
}

#[tauri::command]
pub fn drag_move(window: tauri::WebviewWindow, dx: i32, dy: i32) {
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
pub fn end_drag(window: tauri::WebviewWindow, state: tauri::State<'_, IslandState>) {
    state.is_dragging.store(false, Ordering::Relaxed);

    // 一下状态不自动吸附
    if state.agent_expanded.load(Ordering::Relaxed)
        || state.music_expanded.load(Ordering::Relaxed)
        || state.sadb_mirroring.load(Ordering::Relaxed)
        || state.email_expanded.load(Ordering::Relaxed)
    {
        logger::debug(TAG, "reach expanded");
        return;
    }

    let scale = window.scale_factor().unwrap_or(1.0);
    // 按当前实际窗口宽度重算居中 X，避免 resize-handle 改过宽度后偏移
    let cur_w = window.inner_size()
        .map(|s| s.width as f64 / scale)
        .unwrap_or(WIN_W);
    let target_x = (state.screen_w - cur_w) / 2.0;
    let target_y = TOP_MARGIN;

    if let Ok(pos) = window.outer_position() {
        let cx = pos.x as f64 / scale;
        let cy = pos.y as f64 / scale;
        let w = window.clone();
        thread::spawn(move || { snap_back(&w, cx, cy, target_x, target_y); });
    }
}

#[tauri::command]
pub fn snap_window_home(window: tauri::WebviewWindow, state: tauri::State<'_, IslandState>) {
    logger::debug("Window", &format!("snap_window_home"));
    let scale = window.scale_factor().unwrap_or(1.0);
    let cur_w = window.inner_size()
        .map(|s| s.width as f64 / scale)
        .unwrap_or(WIN_W);
    let target_x = (state.screen_w - cur_w) / 2.0;
    let target_y = TOP_MARGIN;
    if let Ok(pos) = window.outer_position() {
        let cx = pos.x as f64 / scale;
        let cy = pos.y as f64 / scale;
        let w = window.clone();
        thread::spawn(move || { snap_back(&w, cx, cy, target_x, target_y); });
    }
}

#[tauri::command]
pub fn sync_window_home_size(window: tauri::WebviewWindow, state: tauri::State<'_, IslandState>, width: f64, height: f64) {
    logger::debug("Window", &format!("sync_window_home_size: w={width:.0} h={height:.0}"));
    let scale = window.scale_factor().unwrap_or(1.0);
    let new_w = width.max(200.0).min(state.screen_w.max(700.0));
    let new_h = height.max(60.0).min(1100.0);
    let target_x = (state.screen_w - new_w) / 2.0;
    let target_y = TOP_MARGIN;
    if let (Ok(pos), Ok(size)) = (window.outer_position(), window.inner_size()) {
        let from_x = pos.x as f64 / scale;
        let from_y = pos.y as f64 / scale;
        let from_w = size.width as f64 / scale;
        let from_h = size.height as f64 / scale;
        let w = window.clone();
        thread::spawn(move || {
            animate_resize(&w, from_x, from_y, from_w, from_h, target_x, target_y, new_w, new_h, SNAP_DURATION_MS);
        });
    } else {
        let _ = window.set_size(tauri::LogicalSize::new(new_w, new_h));
        let _ = window.set_position(tauri::LogicalPosition::new(target_x, target_y));
    }
}


#[tauri::command]
pub fn sync_window_size(window: tauri::WebviewWindow, state: tauri::State<'_, IslandState>, width: f64, height: f64, reposition: Option<bool>) {
    if state.expand_anim_id.load(Ordering::Relaxed) != 0 { return; }
    let current_view = state.current_view.lock().unwrap().clone();
    let (max_w, max_h) = if state.sadb_mirroring.load(Ordering::Relaxed) || current_view == "email" {
        (state.screen_w.max(700.0), 1100.0)
    } else {
        (EMAIL_VIEW_W, 700.0)
    };
    let new_w = width.max(200.0).min(max_w);
    let new_h = height.max(60.0).min(max_h);
    logger::debug("Window", &format!("sync_window_size: w={width:.0}→{new_w:.0} h={height:.0}→{new_h:.0}"));
    // 只在明确要求居中时（流启动 / 拖拽释放）才重定位，拖拽过程中不移窗口
    if (state.sadb_mirroring.load(Ordering::Relaxed) || current_view == "email") && reposition.unwrap_or(false) {
        let scale = window.scale_factor().unwrap_or(1.0);
        let cur_y = window.outer_position().map(|p| p.y as f64 / scale).unwrap_or(TOP_MARGIN);
        let new_x = (state.screen_w - new_w) / 2.0;
        let _ = window.set_position(tauri::LogicalPosition::new(new_x, cur_y));
    }
    let _ = window.set_size(tauri::LogicalSize::new(new_w, new_h));
}



#[tauri::command]
pub fn set_sadb_expanded(_window: tauri::WebviewWindow, state: tauri::State<'_, IslandState>, expanded: bool) {
    state.sadb_expanded.store(expanded, Ordering::Relaxed);
    if expanded {
        // 流开始，清除待机 flag
        state.sadb_idle.store(false, Ordering::Relaxed);
    }
    // 窗口大小由前端 autoFitWindow + ResizeObserver 统一管理
}

/// sadb 待机面板：进入时动画到顶部居中 + 420×430 尺寸；退出时动画回默认并吸顶
#[tauri::command]
pub fn sadb_set_idle(window: tauri::WebviewWindow, state: tauri::State<'_, IslandState>, idle: bool) {
    state.sadb_idle.store(idle, Ordering::Relaxed);
    if state.email_expanded.load(Ordering::Relaxed) {
        return;
    }
    let screen_w = state.screen_w;
    let scale = window.scale_factor().unwrap_or(1.0);
    let home_x = (screen_w - WIN_W) / 2.0;

    if idle {
        // 流结束/主动展开待机面板：动画移回顶部并调整到待机尺寸
        let target_h = 430.0; // 420px capsule + 10px body padding
        if let Ok(pos) = window.outer_position() {
            let from_x = pos.x as f64 / scale;
            let from_y = pos.y as f64 / scale;
            let (from_w, from_h) = window.inner_size()
                .map(|s| (s.width as f64 / scale, s.height as f64 / scale))
                .unwrap_or((WIN_W, WIN_H_DEFAULT));
            let w = window.clone();
            thread::spawn(move || {
                animate_resize(&w, from_x, from_y, from_w, from_h, home_x, TOP_MARGIN, WIN_W, target_h, 350.0);
            });
        }
    } else {
        // 收起待机面板回胶囊：动画到默认尺寸并确保吸顶居中
        if let Ok(pos) = window.outer_position() {
            let from_x = pos.x as f64 / scale;
            let from_y = pos.y as f64 / scale;
            let (from_w, from_h) = window.inner_size()
                .map(|s| (s.width as f64 / scale, s.height as f64 / scale))
                .unwrap_or((WIN_W, 430.0));
            let w = window.clone();
            thread::spawn(move || {
                animate_resize(&w, from_x, from_y, from_w, from_h, home_x, TOP_MARGIN, WIN_W, WIN_H_DEFAULT, 300.0);
            });
        }
    }
}


#[tauri::command]
pub fn set_minimized(window: tauri::WebviewWindow, state: tauri::State<'_, IslandState>, minimized: bool) {
    state.is_minimized.store(minimized, Ordering::Relaxed);
    if state.email_expanded.load(Ordering::Relaxed) {
        return;
    }
    let screen_w = state.screen_w;
    let scale = window.scale_factor().unwrap_or(1.0);

    if minimized {
        // 收起到绿条：窗口缩小到绿条尺寸
        if let Ok(pos) = window.outer_position() {
            let from_x = pos.x as f64 / scale;
            let from_y = pos.y as f64 / scale;
            let (from_w, from_h) = window.inner_size()
                .map(|s| (s.width as f64 / scale, s.height as f64 / scale))
                .unwrap_or((WIN_W, WIN_H_DEFAULT));

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
            let (from_w, from_h) = window.inner_size()
                .map(|s| (s.width as f64 / scale, s.height as f64 / scale))
                .unwrap_or((MINIMIZED_W, MINIMIZED_H));

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
pub fn show_context_menu(app: tauri::AppHandle, window: tauri::WebviewWindow) {
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
                crate::settings::open_settings(app);
            });
        }
        _ => {}
    }
}

#[tauri::command]
pub fn get_pending_urls(state: tauri::State<'_, IslandState>) -> Vec<String> {
    state.pending_url.lock().unwrap().clone()
}

#[tauri::command]
pub fn set_interacting(state: tauri::State<'_, IslandState>, active: bool) {
    state.is_interacting.store(active, Ordering::Relaxed);
    if active {
        // 用户正在交互，保持展开，取消通知状态让鼠标线程不干扰
        state.is_notifying.store(true, Ordering::Relaxed);
    }
}

#[tauri::command]
pub fn dismiss_island(state: tauri::State<'_, IslandState>, window: tauri::WebviewWindow) {
    state.is_interacting.store(false, Ordering::Relaxed);
    state.is_notifying.store(false, Ordering::Relaxed);
    state.is_expanded.store(false, Ordering::Relaxed);
    let _ = window.emit("set-expand", false);
    let _ = window.emit("reset-view", ());
}

#[tauri::command]
pub fn set_current_view(state: tauri::State<'_, IslandState>, view: String) {
    let v = state.current_view.lock().unwrap().as_str().to_string();
    logger::debug(TAG, &format!("{} -> {}", v, &view));
    let normalized = match view.as_str() {
        "time" | "lyric" | "agent" | "search" | "sadb" | "email" => view,
        _ => "time".to_string(),
    };
    *state.current_view.lock().unwrap() = normalized;
    //清理旧展开态
    match v.as_str() {
        "lyric" => state.music_expanded.store(false, Ordering::Relaxed),
        "agent" => state.agent_expanded.store(false, Ordering::Relaxed),
        "sadb" => state.sadb_expanded.store(false, Ordering::Relaxed),
        "email" => state.email_expanded.store(false, Ordering::Relaxed),
        _ => return,
    }
    
}

//统一封装函数之通用展开设置
#[tauri::command]
pub fn set_expanded(window: tauri::WebviewWindow, state: tauri::State<'_, IslandState>, expanded: bool, width: u64, height: u64) {
    //一展开必须带一次收回否则会出现状态争夺
    //以下长宽皆为逻辑像素,需要转换成物理像素i32才能交给win32用
    let width: f64 = (width as f64 + 20.0).max(WIN_W);
    let height: f64 = (height as f64 + 20.0).max(WIN_H_DEFAULT);
    state.is_expanded.store(expanded, Ordering::Relaxed);
    let v = state.current_view.lock().unwrap().as_str().to_string();
    logger::debug("Window", &format!("view: {}, expanded: {}, width: {}, height: {}", v, expanded, width, height));
    match v.as_str() {
        "lyric" => state.music_expanded.store(expanded, Ordering::Relaxed),
        "agent" => state.agent_expanded.store(expanded, Ordering::Relaxed),
        "sadb" => state.sadb_expanded.store(expanded, Ordering::Relaxed),
        "email" => state.email_expanded.store(expanded, Ordering::Relaxed),
        _ => return,
    }
    let scale = window.scale_factor().unwrap_or(1.0);

    if expanded {
        let hwnd_raw = window.hwnd().unwrap().0 as usize;
        if let Ok(pos) = window.outer_position() {
            let from_x = pos.x as f64 / scale;
            let from_y = pos.y as f64 / scale;
            let from_w = window.inner_size()
                .map(|s| s.width as f64 / scale)
                .unwrap_or(WIN_W);
            let target_x = from_x + (from_w - width) / 2.0;
            let target_y = from_y;
            thread::spawn(move || {
                let hwnd = HWND(hwnd_raw as *mut _);
                unsafe {
                    let _ = SetWindowPos(
                        hwnd,
                        None,
                        (target_x * scale).round() as i32,
                        (target_y * scale).round() as i32,
                        (width * scale).round() as i32,
                        (height * scale).round() as i32,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }

            });
        } else {
            panic!("");
        }
    } else {
        let hwnd_raw = window.hwnd().unwrap().0 as usize;
        if let Ok(pos) = window.outer_position() {
            let from_x = pos.x as f64 / scale;
            let from_y = pos.y as f64 / scale;
            let from_w = window.inner_size()
                .map(|s| s.width as f64 / scale)
                .unwrap_or(WIN_W);
            let target_x = from_x + (from_w - WIN_W) / 2.0;
            let target_y = from_y;
            thread::spawn(move || {
                let hwnd = HWND(hwnd_raw as *mut _);
                unsafe {
                    let _ = SetWindowPos(
                        hwnd,
                        None,
                        (target_x * scale).round() as i32,
                        (target_y * scale).round() as i32,
                        (width * scale).round() as i32,
                        (height * scale).round() as i32,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
            });
            let scale = window.scale_factor().unwrap_or(1.0);
            // 按当前实际窗口宽度重算居中 X，避免 resize-handle 改过宽度后偏移
            let cur_w = window.inner_size()
                .map(|s| s.width as f64 / scale)
                .unwrap_or(WIN_W);
            let target_x = (state.screen_w - cur_w) / 2.0;
            let target_y = TOP_MARGIN;

            if let Ok(pos) = window.outer_position() {
                let cx = pos.x as f64 / scale;
                let cy = pos.y as f64 / scale;
                let w = window.clone();
                thread::spawn(move || { snap_back(&w, cx, cy, target_x, target_y); });
            }
        } else {
            panic!("");
        }
    }
}
#[tauri::command]
pub fn open_email_window(app: tauri::AppHandle, uid: Option<String>) {
    // 如果已有 email 窗口则聚焦
    if let Some(win) = app.get_webview_window("email") {
        let _ = win.set_focus();
        if let Some(uid) = uid {
            let _ = win.emit("email-open-uid", uid);
        }
        return;
    }
    let url = uid
        .as_ref()
        .map(|uid| format!("email.html?uid={uid}"))
        .unwrap_or_else(|| "email.html".to_string());
    let builder = tauri::WebviewWindowBuilder::new(
        &app,
        "email",
        tauri::WebviewUrl::App(url.into()),
    )
    .title("邮件")
    .inner_size(960.0, 640.0)
    .min_inner_size(720.0, 480.0)
    .center()
    .decorations(true)
    .resizable(true);

    match builder.build() {
        Ok(_) => logger::info("Window", "email window opened"),
        Err(e) => logger::info("Window", &format!("failed to open email window: {e}")),
    }
}

