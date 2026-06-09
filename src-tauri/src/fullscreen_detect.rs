use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::Graphics::Gdi::*;

fn get_window_class_name(hwnd: HWND) -> Option<String> {
    unsafe {
        let mut buf = [0u16; 256];
        let len = GetClassNameW(hwnd, &mut buf);
        if len > 0 {
            let class = String::from_utf16_lossy(&buf[..len as usize]);
            Some(class)
        } else {
            None
        }
    }
}

fn is_system_window(hwnd: HWND) -> bool {
    if let Some(class) = get_window_class_name(hwnd) {
        if class == "Shell_TrayWnd"
            || class == "Shell_SecondaryTrayWnd"
            || class == "Progman"
            || class == "WorkerW"
            || class == "TaskManagerWindow"
            || class == "Windows.UI.Core.CoreWindow"
            || class.contains("StartMenu")
            || class.contains("Search")
        {
            return true;
        }
    }
    false
}

pub fn is_foreground_fullscreen() -> bool {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return false;
        }

        if !IsWindowVisible(hwnd).as_bool() {
            return false;
        }

        if is_system_window(hwnd) {
            return false;
        }

        let shell_hwnd = GetShellWindow();
        let progman_hwnd = GetDesktopWindow();
        if hwnd == shell_hwnd || hwnd == progman_hwnd {
            return false;
        }

        let mut window_rect = RECT::default();
        if GetWindowRect(hwnd, &mut window_rect).is_err() {
            return false;
        }

        let win_w = (window_rect.right - window_rect.left) as u32;
        let win_h = (window_rect.bottom - window_rect.top) as u32;

        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if monitor.0.is_null() {
            return false;
        }

        let mut monitor_info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            rcMonitor: RECT::default(),
            rcWork: RECT::default(),
            dwFlags: 0,
        };

        if !GetMonitorInfoW(monitor, &mut monitor_info).as_bool() {
            return false;
        }

        let monitor_w = (monitor_info.rcMonitor.right - monitor_info.rcMonitor.left) as u32;
        let monitor_h = (monitor_info.rcMonitor.bottom - monitor_info.rcMonitor.top) as u32;

        if !(win_w >= monitor_w && win_h >= monitor_h) {
            return false;
        }

        let style = GetWindowLongA(hwnd, GWL_STYLE) as u32;
        if (style & WS_CAPTION.0) == WS_CAPTION.0
            || (style & WS_THICKFRAME.0) == WS_THICKFRAME.0
        {
            return false;
        }

        true
    }
}