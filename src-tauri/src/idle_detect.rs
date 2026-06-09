use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use std::sync::Mutex;

struct HookHandles {
    keyboard: Option<HHOOK>,
    mouse: Option<HHOOK>,
}
unsafe impl Send for HookHandles {}

static HOOKS: Mutex<HookHandles> = Mutex::new(HookHandles { keyboard: None, mouse: None });
static LAST_ACTIVITY: AtomicI64 = AtomicI64::new(0);
static IDLE_ENABLED: AtomicBool = AtomicBool::new(false);
static IDLE_THRESHOLD_SECS: AtomicI64 = AtomicI64::new(300);

pub fn set_idle_config(enabled: bool, threshold_secs: i64) {
    IDLE_ENABLED.store(enabled, Ordering::Relaxed);
    IDLE_THRESHOLD_SECS.store(threshold_secs, Ordering::Relaxed);
}

pub fn get_idle_secs() -> i64 {
    let last = LAST_ACTIVITY.load(Ordering::Relaxed);
    if last == 0 {
        return 0;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    now - last
}

pub fn is_idle() -> bool {
    if !IDLE_ENABLED.load(Ordering::Relaxed) {
        return false;
    }
    let idle_secs = get_idle_secs();
    idle_secs >= IDLE_THRESHOLD_SECS.load(Ordering::Relaxed)
}

fn record_activity() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    LAST_ACTIVITY.store(now, Ordering::Relaxed);
}

unsafe extern "system" fn keyboard_idle_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 {
        record_activity();
    }
    CallNextHookEx(HHOOK::default(), code, wparam, lparam)
}

unsafe extern "system" fn mouse_idle_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 {
        record_activity();
    }
    CallNextHookEx(HHOOK::default(), code, wparam, lparam)
}

pub fn install_idle_hooks() -> Result<(), String> {
    unsafe {
        let kb_hook = SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(keyboard_idle_proc),
            HINSTANCE::default(),
            0,
        ).map_err(|e| format!("Failed to install keyboard idle hook: {}", e))?;

        let mouse_hook = SetWindowsHookExW(
            WH_MOUSE_LL,
            Some(mouse_idle_proc),
            HINSTANCE::default(),
            0,
        ).map_err(|e| format!("Failed to install mouse idle hook: {}", e))?;

        let mut guard = HOOKS.lock().map_err(|e| e.to_string())?;
        guard.keyboard = Some(kb_hook);
        guard.mouse = Some(mouse_hook);

        record_activity();
    }
    Ok(())
}

pub fn uninstall_idle_hooks() {
    unsafe {
        if let Ok(mut guard) = HOOKS.lock() {
            if let Some(h) = guard.keyboard.take() {
                let _ = UnhookWindowsHookEx(h);
            }
            if let Some(h) = guard.mouse.take() {
                let _ = UnhookWindowsHookEx(h);
            }
        }
    }
}