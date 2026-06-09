use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use windows::Win32::Foundation::WIN32_ERROR;
use windows::Win32::System::Registry::*;
use windows::core::PCWSTR;

fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APP_NAME: &str = "EyeGuard";

pub fn set_autostart(enabled: bool, exe_path: &str) -> Result<(), String> {
    unsafe {
        let mut hkey = HKEY::default();
        let key = to_wide(RUN_KEY);
        let result = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            0,
            KEY_SET_VALUE,
            &mut hkey,
        );

        if result != WIN32_ERROR(0) {
            return Err("Failed to open registry key".to_string());
        }

        if enabled {
            let name = to_wide(APP_NAME);
            let value = to_wide(exe_path);
            let data = std::slice::from_raw_parts(value.as_ptr() as *const u8, value.len() * 2);
            let result = RegSetValueExW(
                hkey,
                PCWSTR(name.as_ptr()),
                0,
                REG_SZ,
                Some(data),
            );
            if result != WIN32_ERROR(0) {
                let _ = RegCloseKey(hkey);
                return Err("Failed to set registry value".to_string());
            }
        } else {
            let name = to_wide(APP_NAME);
            let _ = RegDeleteValueW(hkey, PCWSTR(name.as_ptr()));
        }

        let _ = RegCloseKey(hkey);
    }
    Ok(())
}

pub fn is_autostart_enabled() -> bool {
    unsafe {
        let mut hkey = HKEY::default();
        let key = to_wide(RUN_KEY);
        let result = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            0,
            KEY_QUERY_VALUE,
            &mut hkey,
        );
        if result != WIN32_ERROR(0) {
            return false;
        }

        let name = to_wide(APP_NAME);
        let mut buf = [0u16; 1024];
        let mut size = (buf.len() * 2) as u32;
        let result = RegQueryValueExW(
            hkey,
            PCWSTR(name.as_ptr()),
            None,
            None,
            Some(buf.as_mut_ptr() as *mut u8),
            Some(&mut size as *mut u32),
        );
        let _ = RegCloseKey(hkey);
        result == WIN32_ERROR(0)
    }
}
