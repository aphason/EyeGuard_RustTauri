use std::os::windows::ffi::OsStrExt;
use tauri::{
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    menu::{Menu, MenuItem},
    AppHandle, Manager,
};

pub fn create_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let open_settings = MenuItem::with_id(app, "open_settings", "设置属性", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出应用", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open_settings, &quit])?;

    TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "open_settings" => open_or_create_settings(app),
                "quit" => {
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
                        crate::idle_detect::uninstall_idle_hooks();
                        crate::keyboard_hook::uninstall_hook();
                        app.exit(0);
                    }
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event {
                open_or_create_settings(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

fn open_or_create_settings(app: &AppHandle) {
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
        .inner_size(533.0, 480.0)
        .center()
        .decorations(true)
        .resizable(false)
        .build();
    }
}