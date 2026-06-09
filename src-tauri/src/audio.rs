use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr;
use tauri::Manager;

#[link(name = "winmm")]
extern "system" {
    fn mciSendStringW(
        lpstrCommand: *const u16,
        lpstrReturnString: *mut u16,
        uReturnLength: u32,
        hwndCallback: *mut std::ffi::c_void,
    ) -> u32;
    
    fn mciGetErrorStringW(
        fdwError: u32,
        lpszErrorText: *mut u16,
        cchErrorText: u32,
    ) -> u32;
}

fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn wide_to_string(wide: &[u16]) -> String {
    wide.iter()
        .take_while(|&c| *c != 0)
        .map(|&c| c)
        .collect::<Vec<u16>>()
        .iter()
        .map(|c| char::from_u32(*c as u32).unwrap_or('?'))
        .collect()
}

fn get_mci_error(error_code: u32) -> String {
    let mut buffer = [0u16; 256];
    unsafe {
        mciGetErrorStringW(error_code, buffer.as_mut_ptr(), 256);
    }
    wide_to_string(&buffer)
}

pub fn play_midi(file_path: &str) -> Result<(), String> {
    let path = Path::new(file_path);
    if !path.exists() {
        return Err(format!("Audio file not found: {}", file_path));
    }

    unsafe {
        let close_all = to_wide("close all");
        mciSendStringW(close_all.as_ptr(), ptr::null_mut(), 0, ptr::null_mut());
    }

    let alias = "midisound";
    
    let open_cmd = format!("open \"{}\" alias {}", file_path, alias);
    let open_wide = to_wide(&open_cmd);

    let result = unsafe {
        mciSendStringW(open_wide.as_ptr(), ptr::null_mut(), 0, ptr::null_mut())
    };

    if result != 0 {
        let error_msg = get_mci_error(result);
        return Err(format!("MCI open failed ({}): {}", result, error_msg));
    }

    let play_cmd = format!("play {}", alias);
    let play_wide = to_wide(&play_cmd);

    let result = unsafe {
        mciSendStringW(play_wide.as_ptr(), ptr::null_mut(), 0, ptr::null_mut())
    };

    if result != 0 {
        let error_msg = get_mci_error(result);
        let close_cmd = format!("close {}", alias);
        let close_wide = to_wide(&close_cmd);
        unsafe {
            mciSendStringW(close_wide.as_ptr(), ptr::null_mut(), 0, ptr::null_mut());
        }
        return Err(format!("MCI play failed ({}): {}", result, error_msg));
    }

    Ok(())
}

pub fn get_sounds_dir(app_handle: &tauri::AppHandle) -> String {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    
    let exe_sounds_dir = exe_dir.join("sounds");
    if exe_sounds_dir.exists() {
        return exe_sounds_dir.to_string_lossy().to_string();
    }

    let up_sounds_dir = exe_dir.join("_up_").join("sounds");
    if up_sounds_dir.exists() {
        return up_sounds_dir.to_string_lossy().to_string();
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let cwd_sounds_dir = cwd.join("sounds");
    if cwd_sounds_dir.exists() {
        return cwd_sounds_dir.to_string_lossy().to_string();
    }
    
    let resource_dir = app_handle
        .path()
        .resource_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    let resource_sounds_dir = resource_dir.join("sounds");
    if resource_sounds_dir.exists() {
        return resource_sounds_dir.to_string_lossy().to_string();
    }
    
    let resource_up_sounds_dir = resource_dir.join("_up_").join("sounds");
    if resource_up_sounds_dir.exists() {
        return resource_up_sounds_dir.to_string_lossy().to_string();
    }
    
    exe_sounds_dir.to_string_lossy().to_string()
}