use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::io::Write;

static HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);
pub static IS_LOCKED: AtomicBool = AtomicBool::new(false);

const LLKHF_ALTDOWN: u32 = 0x20;
const WM_KEYDOWN: u32 = 0x0100;
const WM_SYSKEYDOWN: u32 = 0x0104;
const WM_KEYUP: u32 = 0x0101;
const WM_SYSKEYUP: u32 = 0x0105;
const VK_CONTROL: u32 = 0x11;
const VK_SHIFT: u32 = 0x10;
const VK_ESCAPE: u32 = 0x1B;
const VK_LWIN: u32 = 0x5B;
const VK_RWIN: u32 = 0x5C;
const VK_MENU: u32 = 0x12;
const VK_TAB: u32 = 0x09;
const VK_F4: u32 = 0x73;

#[repr(C)]
struct KBDLLHOOKSTRUCT {
    vkCode: u32,
    scanCode: u32,
    flags: u32,
    time: u32,
    dwExtraInfo: usize,
}

fn log_to_file(msg: &str) {
    let exe_path = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let exe_dir = exe_path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let log_path = exe_dir.join("eyeguard_log.txt");
    
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0))
            .as_secs();
        let _ = file.write_all(format!("[{}] {}\n", timestamp, msg).as_bytes());
    }
}

unsafe extern "system" fn keyboard_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 {
        let blocked = IS_LOCKED.load(Ordering::SeqCst);
        if blocked {
            let kbd = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
            let vk = kbd.vkCode;
            let flags = kbd.flags;
            let is_key_down = wparam.0 == WM_KEYDOWN as usize || wparam.0 == WM_SYSKEYDOWN as usize;
            let is_key_up = wparam.0 == WM_KEYUP as usize || wparam.0 == WM_SYSKEYUP as usize;

            if is_key_down || is_key_up {
                if vk == VK_LWIN || vk == VK_RWIN {
                    log_to_file(&format!("BLOCKED Win: vk={}, flags={}", vk, flags));
                    return LRESULT(1);
                }

                if vk == VK_MENU {
                    log_to_file(&format!("BLOCKED Alt: vk={}, flags={}", vk, flags));
                    return LRESULT(1);
                }

                let alt_down = (flags & LLKHF_ALTDOWN) != 0;

                if alt_down && vk == VK_TAB {
                    log_to_file(&format!("BLOCKED Alt+Tab: vk={}, flags={}", vk, flags));
                    return LRESULT(1);
                }

                if alt_down && vk == VK_F4 {
                    log_to_file(&format!("BLOCKED Alt+F4: vk={}, flags={}", vk, flags));
                    return LRESULT(1);
                }

                if alt_down && vk == VK_ESCAPE {
                    log_to_file(&format!("BLOCKED Alt+Esc: vk={}, flags={}", vk, flags));
                    return LRESULT(1);
                }

                let ctrl_down = GetAsyncKeyState(VK_CONTROL as i32) < 0;

                if vk == VK_ESCAPE {
                    let shift_down = GetAsyncKeyState(VK_SHIFT as i32) < 0;
                    if ctrl_down || shift_down {
                        log_to_file(&format!("BLOCKED Ctrl/Shift+Esc: vk={}, flags={}", vk, flags));
                        return LRESULT(1);
                    }
                }
            }
        }
    }

    CallNextHookEx(HHOOK::default(), code, wparam, lparam)
}

pub fn install_hook() -> Result<(), String> {
    if HOOK_INSTALLED.load(Ordering::SeqCst) {
        log_to_file("Hook already installed");
        return Ok(());
    }

    let (tx, rx) = std::sync::mpsc::channel();

    thread::spawn(move || {
        unsafe {
            let handle = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(keyboard_proc),
                HINSTANCE::default(),
                0,
            );

            match handle {
                Ok(h) => {
                    log_to_file(&format!("Keyboard hook installed successfully, handle: {:?}", h.0));
                    HOOK_INSTALLED.store(true, Ordering::SeqCst);
                    tx.send(true).ok();
                    
                    let mut msg = MSG::default();
                    loop {
                        let ret = GetMessageW(&mut msg, HWND::default(), 0, 0);
                        if ret.0 <= 0 {
                            break;
                        }
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
                Err(e) => {
                    log_to_file(&format!("Failed to install keyboard hook: {}", e));
                    tx.send(false).ok();
                }
            }
        }
    });

    match rx.recv_timeout(std::time::Duration::from_secs(2)) {
        Ok(true) => Ok(()),
        Ok(false) => Err("Failed to install keyboard hook".to_string()),
        Err(_) => Err("Timeout waiting for hook installation".to_string()),
    }
}

pub fn uninstall_hook() {
    HOOK_INSTALLED.store(false, Ordering::SeqCst);
}

pub fn set_locked(locked: bool, block_input: bool) {
    IS_LOCKED.store(locked, Ordering::SeqCst);
    log_to_file(&format!("IS_LOCKED set to: {}", locked));
    
    if block_input {
        unsafe {
            let _ = BlockInput(true);
            log_to_file("BlockInput called: true");
        }
    }
}

pub fn set_block_input(block: bool) {
    unsafe {
        let _ = BlockInput(block);
        log_to_file(&format!("BlockInput called: {}", block));
    }
}