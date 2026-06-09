mod audio;
mod autostart;
mod fullscreen_detect;
mod idle_detect;
mod keyboard_hook;
mod screen_lock;
mod settings;
mod state;
mod tray;

use settings::{ForceMode, Settings};
use state::{AppStateEnum, AppTimer};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::os::windows::ffi::OsStrExt;
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    Emitter, Listener, Manager, Position,
};

struct SafeHandle(windows::Win32::Foundation::HANDLE);
unsafe impl Send for SafeHandle {}

static SINGLE_INSTANCE_MUTEX: Mutex<Option<SafeHandle>> = Mutex::new(None);
static THREE_MINUTE_WARNING_SHOWN: AtomicBool = AtomicBool::new(false);
static JUST_UNLOCKED: AtomicBool = AtomicBool::new(false);

struct AppState {
    settings: Mutex<Settings>,
    timer: Mutex<AppTimer>,
    lock_manager: Mutex<screen_lock::LockScreenManager>,
}

fn check_single_instance() -> bool {
    use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
    use windows::Win32::System::Threading::CreateMutexW;
    use windows::core::PCWSTR;

    let mutex_name: Vec<u16> = std::ffi::OsStr::new("Global\\EyeGuardSingleInstance")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let handle = CreateMutexW(None, false, PCWSTR(mutex_name.as_ptr()));
        if let Ok(h) = handle {
            let last_error = GetLastError();
            if last_error == ERROR_ALREADY_EXISTS {
                CloseHandle(h).ok();
                return false;
            }
            let mut guard = SINGLE_INSTANCE_MUTEX.lock().unwrap();
            *guard = Some(SafeHandle(h));
            return true;
        }
        false
    }
}

#[tauri::command]
fn get_settings(state: tauri::State<'_, AppState>) -> Result<Settings, String> {
    state.settings.lock().map_err(|e| e.to_string()).map(|s| s.clone())
}

#[tauri::command]
fn save_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    settings: Settings,
) -> Result<(), String> {
    settings::save_settings(&settings)?;
    {
        let mut current = state.settings.lock().map_err(|e| e.to_string())?;
        *current = settings.clone();
    }
    idle_detect::set_idle_config(
        settings.idle_detect_enabled,
        settings.idle_threshold_minutes as i64 * 60,
    );
    {
        let mut timer = state.timer.lock().map_err(|e| e.to_string())?;
        timer.reset_work(settings.work_interval_minutes);
    }
    if let Some(window) = app.get_webview_window("warning") {
        let _ = window.close();
    }
    THREE_MINUTE_WARNING_SHOWN.store(false, Ordering::Relaxed);
    let _ = app.emit("settings_updated", ());
    Ok(())
}

#[tauri::command]
fn start_rest_now(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let settings = state.settings.lock().map_err(|e| e.to_string())?.clone();
    let mut timer = state.timer.lock().map_err(|e| e.to_string())?;

    if let Some(window) = app.get_webview_window("countdown") {
        let _ = window.hide();
    }
    if let Some(window) = app.get_webview_window("warning") {
        let _ = window.close();
    }

    timer.reset_rest(settings.rest_duration_minutes);
    let rest_secs = timer.remaining_secs;
    drop(timer);

    let initial_can_unlock = match settings.force_mode {
        ForceMode::None => true,
        ForceMode::Soft => false,
        ForceMode::Hard => false,
    };
    screen_lock::set_lock_state(rest_secs, initial_can_unlock);

    let sounds_dir = audio::get_sounds_dir(&app);
    let break_path = format!("{}\\break.mid", sounds_dir);
    if let Err(e) = audio::play_midi(&break_path) {
        eprintln!("Audio error: {}", e);
    }

    let mut lock_mgr = state.lock_manager.lock().map_err(|e| e.to_string())?;
    lock_mgr.lock_all_screens(&app)?;

    keyboard_hook::set_locked(true, false);
    THREE_MINUTE_WARNING_SHOWN.store(false, Ordering::Relaxed);

    Ok(())
}

#[tauri::command]
fn postpone_rest(state: tauri::State<'_, AppState>, minutes: u32) -> Result<(), String> {
    let settings = state.settings.lock().map_err(|e| e.to_string())?;
    let max = settings.max_postpone_count;
    drop(settings);

    let mut timer = state.timer.lock().map_err(|e| e.to_string())?;
    if !timer.postpone(minutes, max) {
        return Err("Postpone limit reached".to_string());
    }
    THREE_MINUTE_WARNING_SHOWN.store(false, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
fn set_always_on_top(app: tauri::AppHandle, top: bool) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("countdown") {
        window.set_always_on_top(top).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn open_settings_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.show();
        let _ = win.set_focus();
    } else {
        tauri::WebviewWindowBuilder::new(
            &app,
            "settings",
            tauri::WebviewUrl::App("settings.html".into()),
        )
        .title("爱眼卫士 设置")
        .inner_size(533.0, 460.0)
        .center()
        .decorations(true)
        .resizable(false)
        .build()
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn unlock_screen(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let settings = state.settings.lock().map_err(|e| e.to_string())?.clone();

    let mut lock_mgr = state.lock_manager.lock().map_err(|e| e.to_string())?;
    lock_mgr.unlock_all();
    drop(lock_mgr);

    keyboard_hook::set_locked(false, false);

    JUST_UNLOCKED.store(true, Ordering::Release);

    let mut timer = state.timer.lock().map_err(|e| e.to_string())?;
    timer.reset_work(settings.work_interval_minutes);
    drop(timer);

    if let Some(window) = app.get_webview_window("countdown") {
        let _ = window.show();
    }
    if let Some(window) = app.get_webview_window("warning") {
        let _ = window.close();
    }

    let sounds_dir = audio::get_sounds_dir(&app);
    let unlock_path = format!("{}\\unlock.mid", sounds_dir);
    if let Err(e) = audio::play_midi(&unlock_path) {
        eprintln!("Audio error: {}", e);
    }

    THREE_MINUTE_WARNING_SHOWN.store(false, Ordering::Relaxed);

    Ok(())
}

#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    idle_detect::uninstall_idle_hooks();
    keyboard_hook::uninstall_hook();
    app.exit(0);
}

#[tauri::command]
fn get_autostart(_app: tauri::AppHandle) -> Result<bool, String> {
    Ok(autostart::is_autostart_enabled())
}

#[tauri::command]
fn set_autostart(_app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let exe_path = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .to_string();
    autostart::set_autostart(enabled, &exe_path)
}

#[tauri::command]
fn confirm_dialog(message: String) -> Result<bool, String> {
    extern "system" {
        fn MessageBoxW(hwnd: isize, lptext: *const u16, lpcaption: *const u16, utype: u32) -> i32;
    }
    const MB_YESNO: u32 = 0x00000004;
    const MB_ICONQUESTION: u32 = 0x00000020;
    const MB_TOPMOST: u32 = 0x00040000;
    const IDYES: i32 = 6;

    let msg_wide: Vec<u16> = std::ffi::OsStr::new(&message)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let title_wide: Vec<u16> = std::ffi::OsStr::new("爱眼卫士")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        let result = MessageBoxW(
            0,
            msg_wide.as_ptr(),
            title_wide.as_ptr(),
            MB_YESNO | MB_ICONQUESTION | MB_TOPMOST,
        );
        Ok(result == IDYES)
    }
}

#[tauri::command]
fn show_context_menu(app: tauri::AppHandle, x: f64, y: f64) -> Result<(), String> {
    let wv_window = app.get_webview_window("countdown").ok_or("countdown window not found")?;
    let is_always_on_top = wv_window.is_always_on_top().unwrap_or(false);

    let state = app.state::<AppState>();
    let settings = state.settings.lock().map_err(|e| e.to_string())?;
    let timer = state.timer.lock().map_err(|e| e.to_string())?;
    let postpone_used = timer.postpone_count;
    let max_postpone = settings.max_postpone_count;
    let can_postpone = postpone_used < max_postpone;
    drop(timer);
    drop(settings);

    let immediate_rest = MenuItem::with_id(&app, "ctx_immediate_rest", "立即休息", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let sep1 = PredefinedMenuItem::separator(&app).map_err(|e| e.to_string())?;
    let postpone3 = MenuItem::with_id(&app, "ctx_postpone_3", "推迟休息 3 分钟", can_postpone, None::<&str>)
        .map_err(|e| e.to_string())?;
    let postpone5 = MenuItem::with_id(&app, "ctx_postpone_5", "推迟休息 5 分钟", can_postpone, None::<&str>)
        .map_err(|e| e.to_string())?;
    let postpone10 = MenuItem::with_id(&app, "ctx_postpone_10", "推迟休息 10 分钟", can_postpone, None::<&str>)
        .map_err(|e| e.to_string())?;
    let sep2 = PredefinedMenuItem::separator(&app).map_err(|e| e.to_string())?;
    let top_item = CheckMenuItem::with_id(&app, "ctx_top", "总在最前显示", true, is_always_on_top, None::<&str>)
        .map_err(|e| e.to_string())?;
    let sep3 = PredefinedMenuItem::separator(&app).map_err(|e| e.to_string())?;
    let open_settings = MenuItem::with_id(&app, "ctx_open_settings", "设置属性", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let sep4 = PredefinedMenuItem::separator(&app).map_err(|e| e.to_string())?;
    let quit = MenuItem::with_id(&app, "ctx_quit", "关闭退出", true, None::<&str>)
        .map_err(|e| e.to_string())?;

    let menu = Menu::with_items(&app, &[
        &immediate_rest,
        &sep1,
        &postpone3,
        &postpone5,
        &postpone10,
        &sep2,
        &top_item,
        &sep3,
        &open_settings,
        &sep4,
        &quit,
    ]).map_err(|e| e.to_string())?;

    let position = Position::Logical(tauri::LogicalPosition::new(x, y));
    wv_window.popup_menu_at(&menu, position).map_err(|e| e.to_string())?;

    Ok(())
}

fn show_warning_window(app: &tauri::AppHandle) {
    if app.get_webview_window("warning").is_some() {
        return;
    }
    let _ = tauri::WebviewWindowBuilder::new(
        app,
        "warning",
        tauri::WebviewUrl::App("warning.html".into()),
    )
    .title("爱眼卫士 提醒")
    .inner_size(300.0, 120.0)
    .center()
    .decorations(true)
    .resizable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .build();
}

pub fn run() {
    if !check_single_instance() {
        extern "system" {
            fn MessageBoxW(hwnd: isize, lptext: *const u16, lpcaption: *const u16, utype: u32) -> i32;
        }
        const MB_OK: u32 = 0x00000000;
        const MB_ICONWARNING: u32 = 0x00000030;
        const MB_TOPMOST: u32 = 0x00040000;

        let msg: Vec<u16> = std::ffi::OsStr::new("爱眼卫士已在运行，请勿重复启动。")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let title: Vec<u16> = std::ffi::OsStr::new("爱眼卫士")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            MessageBoxW(0, msg.as_ptr(), title.as_ptr(), MB_OK | MB_ICONWARNING | MB_TOPMOST);
        }
        return;
    }

    let loaded_settings = settings::load_settings();
    let timer = AppTimer::new(loaded_settings.work_interval_minutes);

    idle_detect::set_idle_config(
        loaded_settings.idle_detect_enabled,
        loaded_settings.idle_threshold_minutes as i64 * 60,
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            settings: Mutex::new(loaded_settings.clone()),
            timer: Mutex::new(timer),
            lock_manager: Mutex::new(screen_lock::LockScreenManager::new()),
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            start_rest_now,
            postpone_rest,
            set_always_on_top,
            open_settings_window,
            unlock_screen,
            quit_app,
            get_autostart,
            set_autostart,
            confirm_dialog,
            show_context_menu,
        ])
        .setup(|app| {
            if let Err(e) = keyboard_hook::install_hook() {
                eprintln!("Failed to install keyboard hook: {}", e);
            }
            idle_detect::install_idle_hooks().ok();

            tray::create_tray(app.handle())?;

            screen_lock::set_app_handle(app.handle().clone());

            let app_handle2 = app.handle().clone();
            app.listen("unlock_request", move |_| {
                let state = app_handle2.state::<AppState>();
                let settings = state.settings.lock().unwrap().clone();
                let mut lock_mgr = state.lock_manager.lock().unwrap();
                lock_mgr.unlock_all();
                drop(lock_mgr);
                keyboard_hook::set_locked(false, false);
                JUST_UNLOCKED.store(true, Ordering::Release);
                let mut timer = state.timer.lock().unwrap();
                timer.reset_work(settings.work_interval_minutes);
                drop(timer);
                if let Some(window) = app_handle2.get_webview_window("countdown") {
                    let _ = window.show();
                }
                if let Some(window) = app_handle2.get_webview_window("warning") {
                    let _ = window.close();
                }
                let sounds_dir = audio::get_sounds_dir(&app_handle2);
                if let Err(e) = audio::play_midi(&format!("{}\\unlock.mid", sounds_dir)) {
                    eprintln!("Audio error: {}", e);
                }
                THREE_MINUTE_WARNING_SHOWN.store(false, Ordering::Relaxed);
            });

            let app_handle3 = app.handle().clone();
            app.listen("start_rest", move |_| {
                let state = app_handle3.state::<AppState>();
                let settings = state.settings.lock().unwrap().clone();
                if let Some(window) = app_handle3.get_webview_window("countdown") {
                    let _ = window.hide();
                }
                if let Some(window) = app_handle3.get_webview_window("warning") {
                    let _ = window.close();
                }
                let rest_secs = if let Ok(mut timer) = state.timer.lock() {
                    timer.reset_rest(settings.rest_duration_minutes);
                    timer.remaining_secs
                } else {
                    settings.rest_duration_minutes * 60
                };
                let initial_can_unlock = match settings.force_mode {
                    ForceMode::None => true,
                    ForceMode::Soft => false,
                    ForceMode::Hard => false,
                };
                screen_lock::set_lock_state(rest_secs, initial_can_unlock);
                keyboard_hook::set_locked(true, false);
                if let Ok(mut lock_mgr) = state.lock_manager.lock() {
                    let _ = lock_mgr.lock_all_screens(&app_handle3);
                }
                let sounds_dir = audio::get_sounds_dir(&app_handle3);
                if let Err(e) = audio::play_midi(&format!("{}\\break.mid", sounds_dir)) {
                    eprintln!("Audio error: {}", e);
                }
                THREE_MINUTE_WARNING_SHOWN.store(false, Ordering::Relaxed);
            });

            let app_handle4 = app.handle().clone();
            app.listen("unlock", move |_| {
                let state = app_handle4.state::<AppState>();
                let settings = state.settings.lock().unwrap().clone();
                let mut lock_mgr = state.lock_manager.lock().unwrap();
                lock_mgr.unlock_all();
                drop(lock_mgr);
                keyboard_hook::set_locked(false, false);
                JUST_UNLOCKED.store(true, Ordering::Release);
                let mut timer = state.timer.lock().unwrap();
                timer.reset_work(settings.work_interval_minutes);
                drop(timer);
                if let Some(window) = app_handle4.get_webview_window("countdown") {
                    let _ = window.show();
                }
                if let Some(window) = app_handle4.get_webview_window("warning") {
                    let _ = window.close();
                }
                let sounds_dir = audio::get_sounds_dir(&app_handle4);
                if let Err(e) = audio::play_midi(&format!("{}\\unlock.mid", sounds_dir)) {
                    eprintln!("Audio error: {}", e);
                }
                THREE_MINUTE_WARNING_SHOWN.store(false, Ordering::Relaxed);
            });

            if app.get_webview_window("countdown").is_none() {
                let _countdown = tauri::WebviewWindowBuilder::new(
                    app,
                    "countdown",
                    tauri::WebviewUrl::App("index.html".into()),
                )
                .inner_size(180.0, 70.0)
                .decorations(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .title("EyeGuard")
                .build()?;

                if let Some(monitor) = app.primary_monitor().ok().flatten() {
                    let size = monitor.size();
                    let scale = monitor.scale_factor();
                    let logical_width = size.width as f64 / scale;
                    let x = (logical_width - 400.0).max(0.0);
                    let _ = _countdown.set_position(tauri::LogicalPosition::new(x, 20.0));
                    let _ = _countdown.set_size(tauri::LogicalSize::new(90.0, 45.0));
                }
            }

            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                let mut was_idle = false;
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(1));

                    let state = app_handle.state::<AppState>();
                    let settings = match state.settings.lock() {
                        Ok(s) => s.clone(),
                        Err(_) => continue,
                    };

                    let mut should_unlock = false;
                    let mut should_start_rest = false;

                    if let Ok(mut timer) = state.timer.lock() {
                        if timer.state == AppStateEnum::Working {
                            let is_idle = idle_detect::is_idle();
                            if is_idle && !was_idle {
                                timer.paused = true;
                                was_idle = true;
                            } else if !is_idle && was_idle {
                                timer.reset_work(settings.work_interval_minutes);
                                was_idle = false;
                                THREE_MINUTE_WARNING_SHOWN.store(false, Ordering::Relaxed);
                            }

                            if settings.pause_on_fullscreen {
                                let is_fullscreen = fullscreen_detect::is_foreground_fullscreen();
                                if is_fullscreen && !timer.paused {
                                    timer.paused = true;
                                } else if !is_fullscreen && timer.paused && !is_idle {
                                    timer.paused = false;
                                    timer.last_tick = std::time::Instant::now();
                                }
                            }

                            if let Some(remaining) = timer.tick() {
                                let payload = serde_json::json!({
                                    "remaining_secs": remaining,
                                    "total_secs": timer.total_secs,
                                    "postpone_count": timer.postpone_count,
                                    "max_postpone": settings.max_postpone_count,
                                });
                                let _ = app_handle.emit("tick", payload);

                                if remaining <= 180 && remaining > 170 {
                                    if !THREE_MINUTE_WARNING_SHOWN.load(Ordering::Relaxed) {
                                        THREE_MINUTE_WARNING_SHOWN.store(true, Ordering::Relaxed);
                                        let just_unlocked = JUST_UNLOCKED.swap(false, Ordering::AcqRel);
                                        if !just_unlocked {
                                            let sounds_dir = audio::get_sounds_dir(&app_handle);
                                            let pre_path = format!("{}\\breakpre.mid", sounds_dir);
                                            if let Err(e) = audio::play_midi(&pre_path) {
                                                eprintln!("Audio error: {}", e);
                                            }
                                            show_warning_window(&app_handle);
                                        }
                                    }
                                }

                                if remaining == 0 {
                                    should_start_rest = true;
                                }
                            }
                        } else if timer.state == AppStateEnum::Resting {
                            if let Some(remaining) = timer.tick() {
                                let elapsed = timer.total_secs.saturating_sub(remaining);
                                let can_unlock = match settings.force_mode {
                                    ForceMode::None => true,
                                    ForceMode::Soft => elapsed >= 60,
                                    ForceMode::Hard => false,
                                };

                                screen_lock::set_lock_state(remaining, can_unlock);

                                if remaining == 0 {
                                    should_unlock = true;
                                }
                            }
                        }
                    }

                    if should_start_rest {
                        let _ = app_handle.emit("start_rest", ());
                    }
                    if should_unlock {
                        let _ = app_handle.emit("unlock", ());
                    }
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "countdown" {
                    api.prevent_close();
                }
            }
        })
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "ctx_immediate_rest" => {
                    let state = app.state::<AppState>();
                    let settings = state.settings.lock().unwrap().clone();
                    let timer = state.timer.lock().unwrap();
                    let remaining = timer.remaining_secs;
                    drop(timer);
                    if remaining > 180 {
                        let msg = "确定立即休息吗？";
                        let msg_wide: Vec<u16> = std::ffi::OsStr::new(msg)
                            .encode_wide()
                            .chain(std::iter::once(0))
                            .collect();
                        let title_wide: Vec<u16> = std::ffi::OsStr::new("爱眼卫士")
                            .encode_wide()
                            .chain(std::iter::once(0))
                            .collect();
                        extern "system" {
                            fn MessageBoxW(hwnd: isize, lptext: *const u16, lpcaption: *const u16, utype: u32) -> i32;
                        }
                        const MB_YESNO: u32 = 0x00000004;
                        const MB_ICONQUESTION: u32 = 0x00000020;
                        const MB_TOPMOST: u32 = 0x00040000;
                        const IDYES: i32 = 6;
                        let result = unsafe {
                            MessageBoxW(0, msg_wide.as_ptr(), title_wide.as_ptr(), MB_YESNO | MB_ICONQUESTION | MB_TOPMOST)
                        };
                        if result != IDYES {
                            return;
                        }
                    }
                    let _ = app.emit("start_rest", ());
                }
                "ctx_postpone_3" => {
                    let msg = "确定推迟休息 3 分钟吗？";
                    let msg_wide: Vec<u16> = std::ffi::OsStr::new(msg)
                        .encode_wide()
                        .chain(std::iter::once(0))
                        .collect();
                    let title_wide: Vec<u16> = std::ffi::OsStr::new("爱眼卫士")
                        .encode_wide()
                        .chain(std::iter::once(0))
                        .collect();
                    extern "system" {
                        fn MessageBoxW(hwnd: isize, lptext: *const u16, lpcaption: *const u16, utype: u32) -> i32;
                    }
                    const MB_YESNO: u32 = 0x00000004;
                    const MB_ICONQUESTION: u32 = 0x00000020;
                    const MB_TOPMOST: u32 = 0x00040000;
                    const IDYES: i32 = 6;
                    let result = unsafe {
                        MessageBoxW(0, msg_wide.as_ptr(), title_wide.as_ptr(), MB_YESNO | MB_ICONQUESTION | MB_TOPMOST)
                    };
                    if result != IDYES {
                        return;
                    }
                    let state = app.state::<AppState>();
                    let settings = state.settings.lock().unwrap().clone();
                    let max = settings.max_postpone_count;
                    if let Ok(mut timer) = state.timer.lock() {
                        let _ = timer.postpone(3, max);
                    };
                    THREE_MINUTE_WARNING_SHOWN.store(false, Ordering::Relaxed);
                }
                "ctx_postpone_5" => {
                    let msg = "确定推迟休息 5 分钟吗？";
                    let msg_wide: Vec<u16> = std::ffi::OsStr::new(msg)
                        .encode_wide()
                        .chain(std::iter::once(0))
                        .collect();
                    let title_wide: Vec<u16> = std::ffi::OsStr::new("爱眼卫士")
                        .encode_wide()
                        .chain(std::iter::once(0))
                        .collect();
                    extern "system" {
                        fn MessageBoxW(hwnd: isize, lptext: *const u16, lpcaption: *const u16, utype: u32) -> i32;
                    }
                    const MB_YESNO: u32 = 0x00000004;
                    const MB_ICONQUESTION: u32 = 0x00000020;
                    const MB_TOPMOST: u32 = 0x00040000;
                    const IDYES: i32 = 6;
                    let result = unsafe {
                        MessageBoxW(0, msg_wide.as_ptr(), title_wide.as_ptr(), MB_YESNO | MB_ICONQUESTION | MB_TOPMOST)
                    };
                    if result != IDYES {
                        return;
                    }
                    let state = app.state::<AppState>();
                    let settings = state.settings.lock().unwrap().clone();
                    let max = settings.max_postpone_count;
                    if let Ok(mut timer) = state.timer.lock() {
                        let _ = timer.postpone(5, max);
                    };
                    THREE_MINUTE_WARNING_SHOWN.store(false, Ordering::Relaxed);
                }
                "ctx_postpone_10" => {
                    let msg = "确定推迟休息 10 分钟吗？";
                    let msg_wide: Vec<u16> = std::ffi::OsStr::new(msg)
                        .encode_wide()
                        .chain(std::iter::once(0))
                        .collect();
                    let title_wide: Vec<u16> = std::ffi::OsStr::new("爱眼卫士")
                        .encode_wide()
                        .chain(std::iter::once(0))
                        .collect();
                    extern "system" {
                        fn MessageBoxW(hwnd: isize, lptext: *const u16, lpcaption: *const u16, utype: u32) -> i32;
                    }
                    const MB_YESNO: u32 = 0x00000004;
                    const MB_ICONQUESTION: u32 = 0x00000020;
                    const MB_TOPMOST: u32 = 0x00040000;
                    const IDYES: i32 = 6;
                    let result = unsafe {
                        MessageBoxW(0, msg_wide.as_ptr(), title_wide.as_ptr(), MB_YESNO | MB_ICONQUESTION | MB_TOPMOST)
                    };
                    if result != IDYES {
                        return;
                    }
                    let state = app.state::<AppState>();
                    let settings = state.settings.lock().unwrap().clone();
                    let max = settings.max_postpone_count;
                    if let Ok(mut timer) = state.timer.lock() {
                        let _ = timer.postpone(10, max);
                    };
                    THREE_MINUTE_WARNING_SHOWN.store(false, Ordering::Relaxed);
                }
                "ctx_top" => {
                    if let Some(window) = app.get_webview_window("countdown") {
                        let is_top = window.is_always_on_top().unwrap_or(false);
                        let _ = window.set_always_on_top(!is_top);
                    }
                }
                "ctx_open_settings" => {
                    if let Some(win) = app.get_webview_window("settings") {
                        let _ = win.show();
                        let _ = win.set_focus();
                    } else {
                        let _ = tauri::WebviewWindowBuilder::new(
                            app,
                            "settings",
                            tauri::WebviewUrl::App("settings.html".into()),
                        )
                        .title("爱眼卫士 设置")
                        .inner_size(533.0, 460.0)
                        .center()
                        .decorations(true)
                        .resizable(false)
                        .build();
                    }
                }
                "ctx_quit" => {
                    let msg = "确定退出爱眼卫士吗？";
                    let msg_wide: Vec<u16> = std::ffi::OsStr::new(msg)
                        .encode_wide()
                        .chain(std::iter::once(0))
                        .collect();
                    let title_wide: Vec<u16> = std::ffi::OsStr::new("爱眼卫士")
                        .encode_wide()
                        .chain(std::iter::once(0))
                        .collect();
                    extern "system" {
                        fn MessageBoxW(hwnd: isize, lptext: *const u16, lpcaption: *const u16, utype: u32) -> i32;
                    }
                    const MB_YESNO: u32 = 0x00000004;
                    const MB_ICONQUESTION: u32 = 0x00000020;
                    const MB_TOPMOST: u32 = 0x00040000;
                    const IDYES: i32 = 6;
                    let result = unsafe {
                        MessageBoxW(0, msg_wide.as_ptr(), title_wide.as_ptr(), MB_YESNO | MB_ICONQUESTION | MB_TOPMOST)
                    };
                    if result == IDYES {
                        idle_detect::uninstall_idle_hooks();
                        keyboard_hook::uninstall_hook();
                        app.exit(0);
                    }
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}