use std::sync::{Mutex, OnceLock, atomic::{AtomicBool, AtomicU32, Ordering}};
use std::os::windows::ffi::OsStrExt;
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::core::PCWSTR;

const LOCK_CLASS: &str = "EyeGuardLock";

struct SafeHwnd(HWND);
unsafe impl Send for SafeHwnd {}

static LOCK_CAN_UNLOCK: AtomicBool = AtomicBool::new(false);
static LOCK_REMAINING_SECS: AtomicU32 = AtomicU32::new(0);

pub fn set_lock_state(remaining: u32, can_unlock: bool) {
    LOCK_REMAINING_SECS.store(remaining, Ordering::Release);
    LOCK_CAN_UNLOCK.store(can_unlock, Ordering::Release);
    
    if can_unlock {
        crate::keyboard_hook::set_block_input(false);
    }
}

static APP_HANDLE_FOR_LOCK: OnceLock<tauri::AppHandle> = OnceLock::new();

pub fn set_app_handle(handle: tauri::AppHandle) {
    let _ = APP_HANDLE_FOR_LOCK.set(handle);
}

static LOCK_WINDOWS: Mutex<Vec<SafeHwnd>> = Mutex::new(Vec::new());
static SHOULD_LOCK: AtomicBool = AtomicBool::new(false);
static SHOULD_UNLOCK: AtomicBool = AtomicBool::new(false);
static MESSAGE_THREAD_STARTED: OnceLock<()> = OnceLock::new();
static UNLOCK_HOVER: AtomicBool = AtomicBool::new(false);

fn start_message_thread() {
    if MESSAGE_THREAD_STARTED.set(()).is_err() {
        return;
    }
    
    std::thread::spawn(|| {
        unsafe {
            let hinstance = GetModuleHandleW(None).unwrap_or_default();
            let cn: Vec<u16> = std::ffi::OsStr::new(LOCK_CLASS)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let wc = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(lock_window_proc),
                hInstance: HINSTANCE(hinstance.0),
                lpszClassName: PCWSTR(cn.as_ptr()),
                hbrBackground: HBRUSH(GetStockObject(BLACK_BRUSH).0),
                ..Default::default()
            };
            RegisterClassW(&wc);

            let mut msg = MSG::default();
            loop {
                if SHOULD_LOCK.load(Ordering::Acquire) {
                    SHOULD_LOCK.store(false, Ordering::Release);
                    create_lock_windows_internal();
                }
                
                if SHOULD_UNLOCK.load(Ordering::Acquire) {
                    SHOULD_UNLOCK.store(false, Ordering::Release);
                    destroy_lock_windows_internal();
                }

                while PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_REMOVE).0 != 0 {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
    });
}

unsafe fn create_lock_windows_internal() {
    let cn: Vec<u16> = std::ffi::OsStr::new(LOCK_CLASS)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut monitors: Vec<RECT> = Vec::new();
    let _ = EnumDisplayMonitors(
        HDC::default(),
        None,
        Some(monitor_enum_proc),
        LPARAM(&mut monitors as *mut _ as isize),
    );

    if monitors.is_empty() {
        monitors.push(RECT {
            left: GetSystemMetrics(SM_XVIRTUALSCREEN),
            top: GetSystemMetrics(SM_YVIRTUALSCREEN),
            right: GetSystemMetrics(SM_XVIRTUALSCREEN) + GetSystemMetrics(SM_CXVIRTUALSCREEN),
            bottom: GetSystemMetrics(SM_YVIRTUALSCREEN) + GetSystemMetrics(SM_CYVIRTUALSCREEN),
        });
    }

    let hinstance = GetModuleHandleW(None).unwrap_or_default();
    let mut windows = LOCK_WINDOWS.lock().unwrap();
    windows.clear();

    for r in monitors.iter() {
        let x = r.left;
        let y = r.top;
        let w = r.right - r.left;
        let h = r.bottom - r.top;

        let create_result = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            PCWSTR(cn.as_ptr()),
            PCWSTR::null(),
            WS_POPUP | WS_VISIBLE,
            x, y, w, h,
            HWND::default(),
            HMENU::default(),
            hinstance,
            None,
        );

        if let Ok(hwnd) = create_result {
            if !hwnd.is_invalid() {
                let _ = SetWindowPos(hwnd, HWND_TOPMOST, x, y, w, h, SWP_SHOWWINDOW);
                let _ = ShowWindow(hwnd, SW_SHOW);
                SetTimer(hwnd, 1, 1000, None);
                let _ = SetForegroundWindow(hwnd);
                windows.push(SafeHwnd(hwnd));
                let _ = InvalidateRect(hwnd, None, false);
                let _ = UpdateWindow(hwnd);
            }
        }
    }
}

unsafe fn destroy_lock_windows_internal() {
    let mut windows = LOCK_WINDOWS.lock().unwrap();
    for SafeHwnd(hwnd) in windows.drain(..) {
        KillTimer(hwnd, 1).ok();
        DestroyWindow(hwnd).ok();
    }
}

pub struct LockScreenManager;

unsafe impl Send for LockScreenManager {}

impl LockScreenManager {
    pub fn new() -> Self {
        start_message_thread();
        Self
    }

    pub fn lock_all_screens(&mut self, _app: &tauri::AppHandle) -> Result<(), String> {
        SHOULD_LOCK.store(true, Ordering::Release);
        Ok(())
    }

    pub fn unlock_all(&mut self) {
        SHOULD_UNLOCK.store(true, Ordering::Release);
    }
}

unsafe extern "system" fn lock_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            let rect = ps.rcPaint;
            let brush = CreateSolidBrush(COLORREF(0));
            FillRect(hdc, &rect, brush);
            let _ = DeleteObject(HGDIOBJ(brush.0));

            let remaining = LOCK_REMAINING_SECS.load(Ordering::Acquire);
            let mins = remaining / 60;
            let secs = remaining % 60;
            let text = format!("{:02}:{:02}", mins, secs);
            let mut tw: Vec<u16> = std::ffi::OsStr::new(&text)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            let mut cr = RECT::default();
            let _ = GetClientRect(hwnd, &mut cr);

            let font_name: Vec<u16> = std::ffi::OsStr::new("Microsoft YaHei")
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let hfont = CreateFontW(128, 0, 0, 0, 700, 0, 0, 0, 0, 0, 0, 0, 0, PCWSTR(font_name.as_ptr()));
            let old_font = SelectObject(hdc, HGDIOBJ(hfont.0));
            SetBkMode(hdc, TRANSPARENT);
            SetTextColor(hdc, COLORREF(0x00F7C34F));
            DrawTextW(hdc, &mut tw, &mut cr, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
            SelectObject(hdc, old_font);
            let _ = DeleteObject(HGDIOBJ(hfont.0));

            let can_unlock = LOCK_CAN_UNLOCK.load(Ordering::Acquire);
            if can_unlock {
                let hint = "点击解锁";
                let mut hw: Vec<u16> = std::ffi::OsStr::new(hint)
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect();
                let hfont2 = CreateFontW(32, 0, 0, 0, 400, 0, 0, 0, 0, 0, 0, 0, 0, PCWSTR(font_name.as_ptr()));
                let old2 = SelectObject(hdc, HGDIOBJ(hfont2.0));
                let is_hover = UNLOCK_HOVER.load(Ordering::Acquire);
                if is_hover {
                    SetTextColor(hdc, COLORREF(0x00FFFFFF));
                } else {
                    SetTextColor(hdc, COLORREF(0x00F7C34F));
                }
                let mut br = RECT { left: cr.right - 220, top: cr.bottom - 70, right: cr.right - 10, bottom: cr.bottom - 10 };
                DrawTextW(hdc, &mut hw, &mut br, DT_RIGHT | DT_BOTTOM | DT_SINGLELINE);
                SelectObject(hdc, old2);
                let _ = DeleteObject(HGDIOBJ(hfont2.0));
            }

            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_TIMER => {
            let _ = InvalidateRect(hwnd, None, false);
            LRESULT(0)
        }
        WM_SYSCOMMAND => {
            let cmd = wparam.0 as u32 & 0xFFF0;
            if cmd == SC_CLOSE || cmd == SC_KEYMENU || cmd == SC_TASKLIST {
                return LRESULT(0);
            }
            LRESULT(0)
        }
        WM_ACTIVATEAPP => {
            if wparam.0 == 0 {
                let _ = SetForegroundWindow(hwnd);
                let _ = BringWindowToTop(hwnd);
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let can_unlock = LOCK_CAN_UNLOCK.load(Ordering::Acquire);
            if can_unlock {
                let x = (lparam.0 & 0xFFFF) as i32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i32;
                let mut cr = RECT::default();
                let _ = GetClientRect(hwnd, &mut cr);
                let in_unlock_area = x >= cr.right - 220 && y >= cr.bottom - 70;
                let was_hover = UNLOCK_HOVER.load(Ordering::Acquire);
                if in_unlock_area != was_hover {
                    UNLOCK_HOVER.store(in_unlock_area, Ordering::Release);
                    let _ = InvalidateRect(hwnd, None, false);
                }
                if in_unlock_area {
                    if let Ok(cursor) = LoadCursorW(HINSTANCE::default(), IDC_HAND) {
                        SetCursor(cursor);
                    }
                    return LRESULT(1);
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let can_unlock = LOCK_CAN_UNLOCK.load(Ordering::Acquire);
            if can_unlock {
                let x = (lparam.0 & 0xFFFF) as i32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i32;
                let mut cr = RECT::default();
                let _ = GetClientRect(hwnd, &mut cr);
                if x >= cr.right - 220 && y >= cr.bottom - 70 {
                    if let Some(app) = APP_HANDLE_FOR_LOCK.get() {
                        use tauri::Emitter;
                        let _ = app.emit("unlock_request", ());
                    }
                }
            }
            LRESULT(0)
        }
        WM_NCHITTEST => LRESULT(HTCLIENT as isize),
        WM_SETCURSOR => {
            if let Ok(cursor) = LoadCursorW(HINSTANCE::default(), IDC_ARROW) {
                SetCursor(cursor);
            }
            LRESULT(1)
        },
        WM_CLOSE => {
            let can_unlock = LOCK_CAN_UNLOCK.load(Ordering::Acquire);
            if can_unlock {
                DestroyWindow(hwnd).ok();
            }
            LRESULT(0)
        }
        WM_DESTROY => LRESULT(0),
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe extern "system" fn monitor_enum_proc(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _lprc: *mut RECT,
    dw: LPARAM,
) -> BOOL {
    let monitors = &mut *(dw.0 as *mut Vec<RECT>);
    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    if GetMonitorInfoW(hmonitor, &mut info).0 != 0 {
        monitors.push(info.rcMonitor);
    }
    TRUE
}