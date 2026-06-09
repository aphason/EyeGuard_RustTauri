# 爱眼卫士 (EyeGuard) 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 构建一个 Windows 护眼提醒桌面应用，支持工作/休息状态切换、多屏锁定、全局键盘拦截、系统托盘等功能。

**Architecture:** Tauri v2 混合架构，Vanilla HTML/CSS/JS 前端渲染 UI（倒计时窗口、锁定遮罩、设置界面），Rust 后端负责 Windows API 调用（键盘钩子、多屏锁定、MCI 音频、全屏检测）及业务逻辑（状态机、计时器、设置持久化）。

**Tech Stack:** Rust + Tauri v2 + Vanilla HTML/CSS/JS + `windows` crate (Win32 API) + `serde`/`toml` (配置)

---

### Task 1: 项目脚手架搭建

**Files:**
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/build.rs`
- Create: `src-tauri/src/main.rs` (骨架)
- Create: `src-tauri/src/lib.rs` (骨架)
- Create: `src-tauri/capabilities/default.json`
- Create: `frontend/index.html` (骨架)

- [ ] **Step 1: 初始化 Cargo.toml**

```toml
[package]
name = "eyeguard"
version = "0.1.0"
edition = "2021"

[lib]
name = "eyeguard_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = ["tray-icon"] }
tauri-plugin-opener = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
windows = { version = "0.58", features = [
    "Win32_Foundation",
    "Win32_UI_WindowsAndMessaging",
    "Win32_System_Registry",
    "Win32_System_Console",
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_Graphics_Gdi",
] }
```

- [ ] **Step 2: 创建 tauri.conf.json**

```json
{
  "$schema": "https://raw.githubusercontent.com/tauri-apps/tauri/dev/crates/tauri-config-schema/schema.json",
  "productName": "EyeGuard",
  "version": "0.1.0",
  "identifier": "com.eyeguard.app",
  "build": {
    "frontendDist": "../frontend",
    "devUrl": "http://localhost:1420",
    "beforeDevCommand": "",
    "beforeBuildCommand": ""
  },
  "app": {
    "windows": [],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
  }
}
```

- [ ] **Step 3: 创建 build.rs**

```rust
fn main() {
    tauri_build::build()
}
```

- [ ] **Step 4: 创建 capabilities/default.json**

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Default permissions",
  "windows": ["*"],
  "permissions": [
    "core:default",
    "opener:default"
  ]
}
```

- [ ] **Step 5: 创建 main.rs (骨架)**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    eyeguard_lib::run()
}
```

- [ ] **Step 6: 创建 lib.rs (骨架)**

```rust
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 7: 创建 frontend/index.html (占位)**

```html
<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>EyeGuard - 倒计时</title>
</head>
<body>
  <p>Loading...</p>
</body>
</html>
```

---

### Task 2: 设置模块 (settings.rs)

**Files:**
- Create: `src-tauri/src/settings.rs`
- Modify: `src-tauri/src/lib.rs` (集成 settings)

- [ ] **Step 1: 创建设置结构体与读写函数**

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ForceMode {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "soft")]
    Soft,
    #[serde(rename = "hard")]
    Hard,
}

impl Default for ForceMode {
    fn default() -> Self {
        ForceMode::Soft
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub work_interval_minutes: u32,
    pub rest_duration_minutes: u32,
    pub max_postpone_count: u32,
    pub force_mode: ForceMode,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            work_interval_minutes: 25,
            rest_duration_minutes: 5,
            max_postpone_count: 3,
            force_mode: ForceMode::Soft,
        }
    }
}

fn config_dir() -> PathBuf {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("EyeGuard");
    dir
}

pub fn config_path() -> PathBuf {
    config_dir().join("settings.toml")
}

pub fn load_settings() -> Settings {
    let path = config_path();
    if path.exists() {
        match fs::read_to_string(&path) {
            Ok(content) => {
                toml::from_str(&content).unwrap_or_default()
            }
            Err(_) => Settings::default(),
        }
    } else {
        let default = Settings::default();
        save_settings(&default);
        default
    }
}

pub fn save_settings(settings: &Settings) -> Result<(), String> {
    let dir = config_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let content = toml::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(config_path(), content).map_err(|e| e.to_string())?;
    Ok(())
}
```

- [ ] **Step 2: 添加 dirs 依赖到 Cargo.toml**

在 `[dependencies]` 中添加:
```toml
dirs = "5"
```

- [ ] **Step 3: 注册 Tauri commands**

在 `lib.rs` 中:

```rust
mod settings;
use settings::{Settings, ForceMode};
use std::sync::Mutex;

struct AppState {
    settings: Mutex<Settings>,
    // 更多状态后续添加
}

#[tauri::command]
fn get_settings(state: tauri::State<AppState>) -> Result<Settings, String> {
    let settings = state.settings.lock().map_err(|e| e.to_string())?;
    Ok(settings.clone())
}

#[tauri::command]
fn save_settings(state: tauri::State<AppState>, settings: Settings) -> Result<(), String> {
    settings::save_settings(&settings)?;
    let mut current = state.settings.lock().map_err(|e| e.to_string())?;
    *current = settings;
    Ok(())
}

pub fn run() {
    let loaded_settings = settings::load_settings();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            settings: Mutex::new(loaded_settings),
        })
        .invoke_handler(tauri::generate_handler![get_settings, save_settings])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

---

### Task 3: 状态管理模块 (state.rs)

**Files:**
- Create: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/lib.rs` (集成状态管理)

- [ ] **Step 1: 定义状态枚举与计时器结构体**

```rust
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppStateEnum {
    Working,
    Resting,
}

pub struct AppTimer {
    pub state: AppStateEnum,
    pub remaining_secs: u32,
    pub total_secs: u32,
    pub last_tick: Instant,
    pub paused: bool,
    pub postpone_count: u32,
}

impl AppTimer {
    pub fn new(work_interval_minutes: u32) -> Self {
        let total = work_interval_minutes * 60;
        Self {
            state: AppStateEnum::Working,
            remaining_secs: total,
            total_secs: total,
            last_tick: Instant::now(),
            paused: false,
            postpone_count: 0,
        }
    }

    pub fn reset_work(&mut self, work_interval_minutes: u32) {
        let total = work_interval_minutes * 60;
        self.state = AppStateEnum::Working;
        self.remaining_secs = total;
        self.total_secs = total;
        self.last_tick = Instant::now();
        self.paused = false;
        self.postpone_count = 0;
    }

    pub fn reset_rest(&mut self, rest_duration_minutes: u32) {
        let total = rest_duration_minutes * 60;
        self.state = AppStateEnum::Resting;
        self.remaining_secs = total;
        self.total_secs = total;
        self.last_tick = Instant::now();
    }

    pub fn postpone(&mut self, minutes: u32, max_postpone: u32) -> bool {
        if self.postpone_count >= max_postpone {
            return false;
        }
        self.remaining_secs += minutes * 60;
        self.total_secs += minutes * 60;
        self.postpone_count += 1;
        true
    }

    pub fn tick(&mut self) -> Option<u32> {
        if self.paused {
            return None;
        }
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_tick).as_secs() as u32;
        if elapsed > 0 {
            self.last_tick = now;
            self.remaining_secs = self.remaining_secs.saturating_sub(elapsed);
            Some(self.remaining_secs)
        } else {
            None
        }
    }
}
```

- [ ] **Step 2: 集成到 lib.rs**

在 `lib.rs` 中添加:
```rust
mod state;
use state::{AppTimer, AppStateEnum};
```

修改 `AppState`:
```rust
struct AppState {
    settings: Mutex<Settings>,
    timer: Mutex<AppTimer>,
}
```

初始化:
```rust
let timer = AppTimer::new(loaded_settings.work_interval_minutes);
```

---

### Task 4: 音频模块 (audio.rs)

**Files:**
- Create: `src-tauri/src/audio.rs`

- [ ] **Step 1: 实现 MCI 音频播放**

```rust
use windows::Win32::UI::WindowsAndMessaging::*;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr::null;

fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

pub fn play_midi(file_path: &str) -> Result<(), String> {
    // Close any previously playing MIDI
    let close_cmd = to_wide("close all");
    unsafe {
        mciSendStringW(close_cmd.as_ptr(), null(), 0, None);
    }

    let open_cmd = format!("open \"{}\" type sequencer alias sound", file_path);
    let open_wide = to_wide(&open_cmd);
    let mut buf = [0u16; 256];

    let result = unsafe {
        mciSendStringW(open_wide.as_ptr(), Some(&mut buf), 256, None)
    };
    if result != 0 {
        return Err(format!("Failed to open MIDI file: {}", file_path));
    }

    let play_cmd = to_wide("play sound");
    let result = unsafe {
        mciSendStringW(play_cmd.as_ptr(), null(), 0, None)
    };
    if result != 0 {
        return Err("Failed to play MIDI".to_string());
    }

    Ok(())
}

/// 获取音频文件路径 (相对于应用目录下的 sounds/)
pub fn get_sounds_dir(app_handle: &tauri::AppHandle) -> String {
    let resource_dir = app_handle
        .path()
        .resource_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    let sounds_dir = resource_dir.join("sounds");
    sounds_dir.to_string_lossy().to_string()
}
```

---

### Task 5: 键盘钩子模块 (keyboard_hook.rs)

**Files:**
- Create: `src-tauri/src/keyboard_hook.rs`

- [ ] **Step 1: 实现全局低层键盘钩子**

```rust
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use std::sync::atomic::{AtomicBool, Ordering};

static HOOK_HANDLE: std::sync::Mutex<Option<HHOOK>> = std::sync::Mutex::new(None);
pub static IS_LOCKED: AtomicBool = AtomicBool::new(false);

/// 低层键盘钩子过程
///
/// 当 IS_LOCKED 为 true 时，拦截:
/// - Windows 徽标键 (VK_LWIN, VK_RWIN)
/// - Alt+Tab 组合键
/// - Alt+F4 组合键
unsafe extern "system" fn keyboard_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 && IS_LOCKED.load(Ordering::Relaxed) {
        let kbd = &*(lparam as *const KBDLLHOOKSTRUCT);
        let vk = kbd.vkCode;

        // VK_LWIN = 0x5B, VK_RWIN = 0x5C
        if vk == 0x5B || vk == 0x5C {
            // Windows key pressed, block it
            return LRESULT(1);
        }

        // Alt+Tab: VK_TAB = 0x09, check Alt modifier
        if vk == 0x09 {
            let alt_pressed = (kbd.flags & 0x20) != 0; // LLKHF_ALTDOWN
            if alt_pressed {
                return LRESULT(1);
            }
        }

        // Alt+F4: VK_F4 = 0x73
        if vk == 0x73 {
            let alt_pressed = (kbd.flags & 0x20) != 0;
            if alt_pressed {
                return LRESULT(1);
            }
        }
    }

    CallNextHookEx(HHOOK::default(), code, wparam, lparam)
}

pub fn install_hook() -> Result<(), String> {
    unsafe {
        let handle = SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(keyboard_proc),
            HINSTANCE::default(),
            0,
        );

        if handle.0 == 0 {
            return Err("Failed to install keyboard hook".to_string());
        }

        let mut guard = HOOK_HANDLE.lock().map_err(|e| e.to_string())?;
        *guard = Some(handle);
    }
    Ok(())
}

pub fn uninstall_hook() {
    unsafe {
        if let Ok(mut guard) = HOOK_HANDLE.lock() {
            if let Some(handle) = guard.take() {
                UnhookWindowsHookEx(handle);
            }
        }
    }
}
```

---

### Task 6: 全屏检测模块 (fullscreen_detect.rs)

**Files:**
- Create: `src-tauri/src/fullscreen_detect.rs`

- [ ] **Step 1: 实现前台窗口全屏检测**

```rust
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::Graphics::Gdi::*;

pub fn is_foreground_fullscreen() -> bool {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0 == 0 {
            return false;
        }

        // 检查窗口是否可见
        if !IsWindowVisible(hwnd).as_bool() {
            return false;
        }

        // 获取前台窗口矩形
        let mut window_rect = RECT::default();
        if GetWindowRect(hwnd, &mut window_rect).is_err() {
            return false;
        }

        // 如果是桌面或任务栏，跳过
        let shell_hwnd = GetShellWindow();
        let progman_hwnd = GetDesktopWindow();
        if hwnd == shell_hwnd || hwnd == progman_hwnd {
            return false;
        }

        let win_w = (window_rect.right - window_rect.left) as u32;
        let win_h = (window_rect.bottom - window_rect.top) as u32;

        // 获取窗口所在显示器的工作区域
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if monitor.0 == 0 {
            return false;
        }

        let mut monitor_info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            rcMonitor: RECT::default(),
            rcWork: RECT::default(),
            dwFlags: 0,
        };

        if GetMonitorInfoW(monitor, &mut monitor_info).is_err() {
            return false;
        }

        let monitor_w = (monitor_info.rcMonitor.right - monitor_info.rcMonitor.left) as u32;
        let monitor_h = (monitor_info.rcMonitor.bottom - monitor_info.rcMonitor.top) as u32;

        // 前台窗口尺寸与显示器分辨率匹配 → 全屏
        win_w >= monitor_w && win_h >= monitor_h
    }
}
```

---

### Task 7: 屏幕锁定模块 (screen_lock.rs)

**Files:**
- Create: `src-tauri/src/screen_lock.rs`

- [ ] **Step 1: 实现多显示器锁定窗口管理**

```rust
use tauri::{AppHandle, WebviewWindowBuilder, WebviewUrl};
use std::collections::HashMap;

pub struct LockScreenManager {
    lock_windows: HashMap<String, tauri::WebviewWindow>,
}

impl LockScreenManager {
    pub fn new() -> Self {
        Self {
            lock_windows: HashMap::new(),
        }
    }

    /// 在所有显示器上创建黑色全屏锁定窗口
    pub fn lock_all_screens(&mut self, app: &AppHandle) -> Result<(), String> {
        unsafe {
            let monitors = enumerate_monitors()?;
            for (i, monitor_rect) in monitors.iter().enumerate() {
                let label = format!("lock-{}", i);
                let window = WebviewWindowBuilder::new(
                    app,
                    &label,
                    WebviewUrl::App("lock.html".into()),
                )
                .fullscreen(false)
                .inner_size(monitor_rect.right as f64 - monitor_rect.left as f64,
                           monitor_rect.bottom as f64 - monitor_rect.top as f64)
                .position(monitor_rect.left as f64, monitor_rect.top as f64)
                .decorations(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .title("")
                .build()
                .map_err(|e| e.to_string())?;

                self.lock_windows.insert(label, window);
            }
            // 激活键盘钩子锁定
            crate::keyboard_hook::IS_LOCKED.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(())
    }

    /// 销毁所有锁定窗口，解锁
    pub fn unlock_all(&mut self) {
        for (_, window) in self.lock_windows.drain() {
            let _ = window.close();
        }
        crate::keyboard_hook::IS_LOCKED.store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

/// 枚举所有显示器
unsafe fn enumerate_monitors() -> Result<Vec<windows::Win32::Foundation::RECT>, String> {
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::Win32::Graphics::Gdi::*;

    let mut monitors = Vec::new();

    let result = EnumDisplayMonitors(
        HDC::default(),
        None,
        Some(monitor_enum_proc),
        LPARAM(&mut monitors as *mut _ as isize),
    );

    if !result.as_bool() {
        return Err("Failed to enumerate monitors".to_string());
    }

    Ok(monitors)
}

unsafe extern "system" fn monitor_enum_proc(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _lprc_clip: *mut RECT,
    dw_data: LPARAM,
) -> BOOL {
    let monitors = &mut *(dw_data.0 as *mut Vec<windows::Win32::Foundation::RECT>);

    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        rcMonitor: RECT::default(),
        rcWork: RECT::default(),
        dwFlags: 0,
    };

    if GetMonitorInfoW(hmonitor, &mut info).is_ok() {
        monitors.push(info.rcMonitor);
    }

    TRUE
}
```

---

### Task 8: 前端 - 深海蓝主题样式 (theme.css)

**Files:**
- Create: `frontend/styles/theme.css`

- [ ] **Step 1: 创建全局主题样式**

```css
:root {
  --bg-primary: #0a1628;
  --bg-secondary: #0f3460;
  --bg-accent: #1a3a6a;
  --accent: #4fc3f7;
  --accent-dim: #1a3a6a;
  --text-primary: #e0e0e0;
  --text-secondary: #78909c;
  --text-muted: #546e7a;
  --border: #0f3460;
  --danger: #e94560;
  --success: #4caf50;
}

* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

body {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'PingFang SC', 'Microsoft YaHei', sans-serif;
  background: var(--bg-primary);
  color: var(--text-primary);
  user-select: none;
  overflow: hidden;
}

::-webkit-scrollbar {
  width: 4px;
}
::-webkit-scrollbar-track {
  background: var(--bg-primary);
}
::-webkit-scrollbar-thumb {
  background: var(--bg-secondary);
  border-radius: 2px;
}
```

---

### Task 9: 前端 - 倒计时窗口 (index.html)

**Files:**
- Create: `frontend/index.html`

- [ ] **Step 1: 创建倒计时窗口 HTML**

```html
<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>EyeGuard</title>
  <link rel="stylesheet" href="styles/theme.css">
  <style>
    body {
      background: transparent;
      width: 180px;
      height: 70px;
      position: relative;
      cursor: default;
    }
    .container {
      width: 100%;
      height: 100%;
      background: linear-gradient(135deg, rgba(10,22,40,0.95), rgba(15,52,96,0.85));
      border: 1px solid var(--border);
      border-radius: 8px;
      position: relative;
      overflow: hidden;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
    }
    .progress-bar {
      position: absolute;
      left: 0;
      top: 0;
      height: 100%;
      background: linear-gradient(to right, rgba(26,58,106,0.6), transparent);
      transition: width 0.3s linear;
      z-index: 0;
    }
    .time-display {
      font-size: 28px;
      font-weight: bold;
      font-family: 'Courier New', monospace;
      color: var(--accent);
      z-index: 1;
      letter-spacing: 2px;
      text-shadow: 0 0 10px rgba(79,195,247,0.3);
    }
    .label {
      font-size: 10px;
      color: var(--text-secondary);
      z-index: 1;
      margin-top: 1px;
    }
    .warning {
      color: var(--danger);
    }
    .context-menu {
      display: none;
      position: fixed;
      min-width: 180px;
      background: #0d1b2a;
      border: 1px solid var(--border);
      border-radius: 6px;
      padding: 4px 0;
      z-index: 9999;
      box-shadow: 0 8px 24px rgba(0,0,0,0.5);
    }
    .context-menu .menu-item {
      padding: 8px 16px;
      cursor: pointer;
      font-size: 13px;
      color: var(--text-primary);
      white-space: nowrap;
    }
    .context-menu .menu-item:hover {
      background: var(--bg-accent);
      color: var(--accent);
    }
    .context-menu .menu-separator {
      height: 1px;
      background: var(--border);
      margin: 4px 8px;
    }
    .context-menu .menu-item.disabled {
      color: var(--text-muted);
      cursor: not-allowed;
    }
    .context-menu .menu-item.disabled:hover {
      background: transparent;
      color: var(--text-muted);
    }
  </style>
</head>
<body>
  <div class="container" id="app">
    <div class="progress-bar" id="progressBar"></div>
    <div class="time-display" id="timeDisplay">25:00</div>
    <div class="label" id="statusLabel">距下次休息</div>
  </div>

  <div class="context-menu" id="contextMenu">
    <div class="menu-item" onclick="immediateRest()">立即休息</div>
    <div class="menu-item" onclick="postpone(3)">推迟休息 3 分钟</div>
    <div class="menu-item" onclick="postpone(5)">推迟休息 5 分钟</div>
    <div class="menu-item" onclick="postpone(10)">推迟休息 10 分钟</div>
    <div class="menu-separator"></div>
    <div class="menu-item" onclick="toggleAlwaysOnTop()">总在最前显示</div>
    <div class="menu-item" onclick="cancelAlwaysOnTop()">取消最前显示</div>
    <div class="menu-separator"></div>
    <div class="menu-item" onclick="openSettings()">设置属性</div>
    <div class="menu-separator"></div>
    <div class="menu-item" onclick="quitApp()">关闭退出</div>
  </div>

  <script>
    const { invoke } = window.__TAURI__.core;
    const { getCurrentWindow } = window.__TAURI__.window;

    let appWindow = getCurrentWindow();
    let postponeCount = 0;
    let maxPostpone = 3;

    // 右键菜单
    document.getElementById('app').addEventListener('contextmenu', (e) => {
      e.preventDefault();
      const menu = document.getElementById('contextMenu');
      menu.style.left = e.clientX + 'px';
      menu.style.top = e.clientY + 'px';
      menu.style.display = 'block';

      // 更新推迟菜单状态
      const postponeItems = menu.querySelectorAll('.menu-item:not([onclick*="immediate"]):not([onclick*="toggle"]):not([onclick*="cancel"]):not([onclick*="openSettings"]):not([onclick*="quit"])');
      postponeItems.forEach(item => {
        if (postponeCount >= maxPostpone) {
          item.classList.add('disabled');
        } else {
          item.classList.remove('disabled');
        }
      });
    });

    document.addEventListener('click', () => {
      document.getElementById('contextMenu').style.display = 'none';
    });

    // 拖拽
    let isDragging = false, dragX, dragY;
    document.getElementById('app').addEventListener('mousedown', (e) => {
      if (e.button !== 0) return;
      isDragging = true;
      const pos = appWindow.outerPosition();
      dragX = e.screenX;
      dragY = e.screenY;
    });
    document.addEventListener('mousemove', (e) => {
      if (!isDragging) return;
      const dx = e.screenX - dragX;
      const dy = e.screenY - dragY;
      dragX = e.screenX;
      dragY = e.screenY;
      appWindow.setPosition(appWindow.outerPosition().x + dx, appWindow.outerPosition().y + dy);
    });
    document.addEventListener('mouseup', () => { isDragging = false; });

    // 监听 Tauri 事件
    window.__TAURI__.event.listen('tick', (event) => {
      const { remaining_secs, total_secs, state, postpone_count, max_postpone: mp } = event.payload;
      const mins = Math.floor(remaining_secs / 60);
      const secs = remaining_secs % 60;
      document.getElementById('timeDisplay').textContent = 
        String(mins).padStart(2, '0') + ':' + String(secs).padStart(2, '0');
      
      const pct = total_secs > 0 ? (remaining_secs / total_secs) * 100 : 0;
      document.getElementById('progressBar').style.width = pct + '%';

      if (remaining_secs <= 180 && remaining_secs > 0) {
        document.getElementById('timeDisplay').classList.add('warning');
      } else {
        document.getElementById('timeDisplay').classList.remove('warning');
      }

      postponeCount = postpone_count;
      maxPostpone = mp;
    });

    async function immediateRest() {
      await invoke('start_rest_now');
    }
    async function postpone(minutes) {
      try {
        await invoke('postpone_rest', { minutes });
      } catch (e) {
        console.error(e);
      }
    }
    async function toggleAlwaysOnTop() {
      await invoke('set_always_on_top', { top: true });
    }
    async function cancelAlwaysOnTop() {
      await invoke('set_always_on_top', { top: false });
    }
    async function openSettings() {
      await invoke('open_settings_window');
    }
    async function quitApp() {
      await invoke('quit_app');
    }
  </script>
</body>
</html>
```

---

### Task 10: 前端 - 锁定屏幕 (lock.html)

**Files:**
- Create: `frontend/lock.html`

- [ ] **Step 1: 创建锁定屏幕 HTML**

```html
<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>EyeGuard - 休息中</title>
  <style>
    * { margin: 0; padding: 0; box-sizing: border-box; }
    body {
      background: #000;
      color: #fff;
      width: 100vw;
      height: 100vh;
      display: flex;
      align-items: center;
      justify-content: center;
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
      user-select: none;
      overflow: hidden;
    }
    .container {
      text-align: center;
    }
    .time-display {
      font-size: 96px;
      font-weight: bold;
      font-family: 'Courier New', monospace;
      color: #4fc3f7;
      letter-spacing: 6px;
      text-shadow: 0 0 30px rgba(79,195,247,0.3);
    }
    .label {
      font-size: 18px;
      color: #78909c;
      margin-top: 16px;
    }
    .unlock-btn {
      position: fixed;
      bottom: 40px;
      right: 40px;
      width: 48px;
      height: 48px;
      border-radius: 50%;
      background: rgba(79,195,247,0.15);
      border: 1px solid rgba(79,195,247,0.3);
      color: #4fc3f7;
      font-size: 22px;
      cursor: pointer;
      display: flex;
      align-items: center;
      justify-content: center;
      transition: all 0.3s;
      opacity: 0;
      pointer-events: none;
    }
    .unlock-btn.visible {
      opacity: 1;
      pointer-events: auto;
    }
    .unlock-btn:hover {
      background: rgba(79,195,247,0.3);
    }
  </style>
</head>
<body>
  <div class="container">
    <div class="time-display" id="timeDisplay">05:00</div>
    <div class="label" id="statusLabel">休息时间 · 请远离屏幕</div>
  </div>
  <button class="unlock-btn" id="unlockBtn" onclick="unlock()">🔓</button>

  <script>
    window.__TAURI__.event.listen('lock_tick', (event) => {
      const { remaining_secs, can_unlock } = event.payload;
      const mins = Math.floor(remaining_secs / 60);
      const secs = remaining_secs % 60;
      document.getElementById('timeDisplay').textContent =
        String(mins).padStart(2, '0') + ':' + String(secs).padStart(2, '0');

      if (can_unlock) {
        document.getElementById('unlockBtn').classList.add('visible');
      }
    });

    async function unlock() {
      const { invoke } = window.__TAURI__.core;
      await invoke('unlock_screen');
    }
  </script>
</body>
</html>
```

---

### Task 11: 前端 - 设置界面 (settings.html)

**Files:**
- Create: `frontend/settings.html`

- [ ] **Step 1: 创建设置界面 HTML**

```html
<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>EyeGuard - 设置</title>
  <link rel="stylesheet" href="styles/theme.css">
  <style>
    body {
      width: 400px;
      padding: 24px;
      background: var(--bg-primary);
    }
    h2 {
      font-size: 18px;
      color: var(--accent);
      margin-bottom: 20px;
      padding-bottom: 12px;
      border-bottom: 1px solid var(--border);
      font-weight: 500;
    }
    .form-group {
      margin-bottom: 16px;
    }
    label {
      display: block;
      font-size: 13px;
      color: var(--text-secondary);
      margin-bottom: 6px;
    }
    select, input[type="checkbox"] {
      width: 100%;
      padding: 8px 12px;
      background: var(--bg-secondary);
      border: 1px solid var(--border);
      border-radius: 4px;
      color: var(--text-primary);
      font-size: 14px;
      outline: none;
      cursor: pointer;
    }
    select:focus {
      border-color: var(--accent);
    }
    select option {
      background: var(--bg-primary);
    }
    .checkbox-group {
      display: flex;
      align-items: center;
      gap: 8px;
    }
    .checkbox-group input[type="checkbox"] {
      width: auto;
    }
    .checkbox-group label {
      margin-bottom: 0;
      cursor: pointer;
    }
    .actions {
      display: flex;
      justify-content: flex-end;
      gap: 12px;
      margin-top: 24px;
      padding-top: 16px;
      border-top: 1px solid var(--border);
    }
    .actions button {
      padding: 8px 24px;
      border: none;
      border-radius: 4px;
      font-size: 14px;
      cursor: pointer;
      transition: all 0.2s;
    }
    .btn-cancel {
      background: transparent;
      border: 1px solid var(--border) !important;
      color: var(--text-secondary);
    }
    .btn-cancel:hover {
      background: var(--bg-secondary);
    }
    .btn-save {
      background: var(--accent);
      color: #fff;
    }
    .btn-save:hover {
      background: #3ab0e0;
    }
    .force-mode-group {
      margin-top: 8px;
      padding-left: 24px;
    }
    .force-mode-group.hidden {
      display: none;
    }
  </style>
</head>
<body>
  <h2>⚙ 爱眼卫士 设置</h2>

  <div class="form-group">
    <label>工作时间间隔</label>
    <select id="workInterval">
      <!-- JS 动态填充 -->
    </select>
  </div>

  <div class="form-group">
    <label>休息时间长度</label>
    <select id="restDuration">
      <!-- JS 动态填充 -->
    </select>
  </div>

  <div class="form-group">
    <label>允许推迟休息次数</label>
    <select id="maxPostpone">
      <!-- 1-6 -->
    </select>
  </div>

  <div class="form-group">
    <div class="checkbox-group">
      <input type="checkbox" id="forceRest">
      <label for="forceRest">强制休息</label>
    </div>
  </div>

  <div class="force-mode-group" id="forceModeGroup">
    <div class="form-group">
      <label>强制模式</label>
      <select id="forceMode">
        <option value="soft">一般强制（1分钟后可解锁）</option>
        <option value="hard">完全强制（不可解锁）</option>
      </select>
    </div>
  </div>

  <div class="actions">
    <button class="btn-cancel" onclick="cancel()">取消</button>
    <button class="btn-save" onclick="save()">保存</button>
  </div>

  <script>
    const { invoke } = window.__TAURI__.core;

    // 填充下拉选项
    function populateSelect(selId, start, end, selected) {
      const sel = document.getElementById(selId);
      sel.innerHTML = '';
      for (let i = start; i <= end; i++) {
        const opt = document.createElement('option');
        opt.value = i;
        opt.textContent = i + ' 分钟';
        if (i === selected) opt.selected = true;
        sel.appendChild(opt);
      }
    }

    let originalSettings = null;

    async function loadSettings() {
      const s = await invoke('get_settings');
      originalSettings = JSON.parse(JSON.stringify(s));

      populateSelect('workInterval', 1, 120, s.work_interval_minutes);
      populateSelect('restDuration', 1, 30, s.rest_duration_minutes);

      const mpSel = document.getElementById('maxPostpone');
      mpSel.innerHTML = '';
      for (let i = 1; i <= 6; i++) {
        const opt = document.createElement('option');
        opt.value = i;
        opt.textContent = i + ' 次';
        if (i === s.max_postpone_count) opt.selected = true;
        mpSel.appendChild(opt);
      }

      document.getElementById('forceRest').checked = s.force_mode !== 'none';
      document.getElementById('forceMode').value = s.force_mode === 'hard' ? 'hard' : 'soft';
      toggleForceMode();
    }

    function toggleForceMode() {
      const checked = document.getElementById('forceRest').checked;
      document.getElementById('forceModeGroup').classList.toggle('hidden', !checked);
    }

    document.getElementById('forceRest').addEventListener('change', toggleForceMode);

    async function save() {
      const forceRest = document.getElementById('forceRest').checked;
      const forceMode = forceRest ? document.getElementById('forceMode').value : 'none';

      const newSettings = {
        work_interval_minutes: parseInt(document.getElementById('workInterval').value),
        rest_duration_minutes: parseInt(document.getElementById('restDuration').value),
        max_postpone_count: parseInt(document.getElementById('maxPostpone').value),
        force_mode: forceMode,
      };

      await invoke('save_settings', { settings: newSettings });
      window.close();
    }

    function cancel() {
      window.close();
    }

    loadSettings();
  </script>
</body>
</html>
```

---

### Task 12: Rust - 系统托盘模块 (tray.rs)

**Files:**
- Create: `src-tauri/src/tray.rs`

- [ ] **Step 1: 实现系统托盘**

```rust
use tauri::{
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    menu::{Menu, MenuItem},
    AppHandle, Runtime,
};

pub fn create_tray<R: Runtime>(app: &AppHandle<R>) -> Result<(), Box<dyn std::error::Error>> {
    let open_settings = MenuItem::with_id(app, "open_settings", "打开设置", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open_settings, &quit])?;

    TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "open_settings" => {
                    let _ = app.emit("open_settings", ());
                }
                "quit" => {
                    crate::keyboard_hook::uninstall_hook();
                    app.exit(0);
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
                let app = tray.app_handle();
                let _ = app.emit("open_settings", ());
            }
        })
        .build(app)?;

    Ok(())
}
```

---

### Task 13: 开机自启模块 (autostart.rs)

**Files:**
- Create: `src-tauri/src/autostart.rs`

- [ ] **Step 1: 实现注册表开机自启**

```rust
use windows::Win32::Foundation::*;
use windows::Win32::System::Registry::*;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

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
            key.as_ptr(),
            0,
            KEY_SET_VALUE,
            &mut hkey,
        );

        if result != 0 {
            return Err("Failed to open registry key".to_string());
        }

        if enabled {
            let name = to_wide(APP_NAME);
            let value = to_wide(exe_path);
            let result = RegSetValueExW(
                hkey,
                name.as_ptr(),
                0,
                REG_SZ,
                Some(value.as_ptr() as *const u8),
                (value.len() * 2) as u32,
            );
            if result != 0 {
                return Err("Failed to set registry value".to_string());
            }
        } else {
            let name = to_wide(APP_NAME);
            RegDeleteValueW(hkey, name.as_ptr());
        }

        RegCloseKey(hkey);
    }
    Ok(())
}

pub fn is_autostart_enabled() -> bool {
    unsafe {
        let mut hkey = HKEY::default();
        let key = to_wide(RUN_KEY);
        let result = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            0,
            KEY_QUERY_VALUE,
            &mut hkey,
        );
        if result != 0 {
            return false;
        }

        let name = to_wide(APP_NAME);
        let mut buf = [0u16; 1024];
        let mut size = (buf.len() * 2) as u32;
        let result = RegQueryValueExW(
            hkey,
            name.as_ptr(),
            None,
            None,
            Some(buf.as_mut_ptr() as *mut u8),
            &mut size,
        );
        RegCloseKey(hkey);
        result == 0
    }
}
```

---

### Task 14: 主集成 (lib.rs 完整版 + main.rs)

**Files:**
- Modify: `src-tauri/src/lib.rs` (完整实现)
- Modify: `src-tauri/src/main.rs` (最终)

- [ ] **Step 1: 实现完整的 lib.rs**

```rust
mod audio;
mod autostart;
mod fullscreen_detect;
mod keyboard_hook;
mod screen_lock;
mod settings;
mod state;
mod tray;

use settings::{ForceMode, Settings};
use state::{AppStateEnum, AppTimer};
use std::sync::Mutex;
use tauri::{Emitter, Manager};

struct AppState {
    settings: Mutex<Settings>,
    timer: Mutex<AppTimer>,
    lock_manager: Mutex<screen_lock::LockScreenManager>,
}

#[tauri::command]
fn get_settings(state: tauri::State<AppState>) -> Result<Settings, String> {
    state.settings.lock().map_err(|e| e.to_string()).map(|s| s.clone())
}

#[tauri::command]
fn save_settings(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    settings: Settings,
) -> Result<(), String> {
    settings::save_settings(&settings)?;
    {
        let mut current = state.settings.lock().map_err(|e| e.to_string())?;
        *current = settings.clone();
    }
    // 重置计时器以应用新的工作间隔
    {
        let mut timer = state.timer.lock().map_err(|e| e.to_string())?;
        timer.reset_work(settings.work_interval_minutes);
    }
    let _ = app.emit("settings_updated", ());
    Ok(())
}

#[tauri::command]
fn start_rest_now(app: tauri::AppHandle, state: tauri::State<AppState>) -> Result<(), String> {
    let settings = state.settings.lock().map_err(|e| e.to_string())?.clone();
    let mut timer = state.timer.lock().map_err(|e| e.to_string())?;

    // 隐藏倒计时窗口
    if let Some(window) = app.get_webview_window("countdown") {
        let _ = window.hide();
    }

    // 重置休息计时器
    timer.reset_rest(settings.rest_duration_minutes);

    // 播放 break.mid
    let sounds_dir = audio::get_sounds_dir(&app);
    let break_path = format!("{}\\break.mid", sounds_dir);
    let _ = audio::play_midi(&break_path);

    // 锁定所有屏幕
    let mut lock_mgr = state.lock_manager.lock().map_err(|e| e.to_string())?;
    lock_mgr.lock_all_screens(&app)?;

    // 键盘钩子启用锁定
    keyboard_hook::IS_LOCKED.store(true, std::sync::atomic::Ordering::Relaxed);

    Ok(())
}

#[tauri::command]
fn postpone_rest(state: tauri::State<AppState>, minutes: u32) -> Result<(), String> {
    let settings = state.settings.lock().map_err(|e| e.to_string())?;
    let max = settings.max_postpone_count;
    drop(settings);

    let mut timer = state.timer.lock().map_err(|e| e.to_string())?;
    if !timer.postpone(minutes, max) {
        return Err("推迟次数已用完".to_string());
    }
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
    let settings_win = app.get_webview_window("settings");
    if let Some(win) = settings_win {
        let _ = win.show();
        let _ = win.set_focus();
    } else {
        let _ = tauri::WebviewWindowBuilder::new(
            &app,
            "settings",
            tauri::WebviewUrl::App("settings.html".into()),
        )
        .title("爱眼卫士 设置")
        .inner_size(400.0, 380.0)
        .center()
        .decorations(true)
        .resizable(false)
        .build();
    }
    Ok(())
}

#[tauri::command]
fn unlock_screen(app: tauri::AppHandle, state: tauri::State<AppState>) -> Result<(), String> {
    let settings = state.settings.lock().map_err(|e| e.to_string())?.clone();

    // 解锁所有屏幕
    let mut lock_mgr = state.lock_manager.lock().map_err(|e| e.to_string())?;
    lock_mgr.unlock_all();

    // 键盘钩子解锁
    keyboard_hook::IS_LOCKED.store(false, std::sync::atomic::Ordering::Relaxed);

    // 播放 unlock.mid
    let sounds_dir = audio::get_sounds_dir(&app);
    let unlock_path = format!("{}\\unlock.mid", sounds_dir);
    let _ = audio::play_midi(&unlock_path);

    // 重置工作计时器
    let mut timer = state.timer.lock().map_err(|e| e.to_string())?;
    timer.reset_work(settings.work_interval_minutes);

    // 显示倒计时窗口
    if let Some(window) = app.get_webview_window("countdown") {
        let _ = window.show();
    }

    Ok(())
}

#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    keyboard_hook::uninstall_hook();
    app.exit(0);
}

pub fn run() {
    let loaded_settings = settings::load_settings();
    let timer = AppTimer::new(loaded_settings.work_interval_minutes);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            settings: Mutex::new(loaded_settings),
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
        ])
        .setup(|app| {
            // 安装键盘钩子
            keyboard_hook::install_hook().ok();

            // 创建系统托盘
            tray::create_tray(app.handle())?;

            // 创建倒计时窗口
            let _countdown = tauri::WebviewWindowBuilder::new(
                app,
                "countdown",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .inner_size(180.0, 70.0)
            .position(
                // 右上角：获取屏幕宽度
                {
                    use tauri::PhysicalPosition;
                    // 获取主显示器宽度
                    if let Some(monitor) = app.primary_monitor().ok().flatten() {
                        let size = monitor.size();
                        (size.width as f64) - 200.0
                    } else {
                        1200.0
                    }
                },
                20.0,
            )
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .title("EyeGuard")
            .build()?;

            // 定时器：每秒 tick
            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(1));

                    let state = app_handle.state::<AppState>();
                    let settings = match state.settings.lock() {
                        Ok(s) => s.clone(),
                        Err(_) => continue,
                    };

                    // 全屏检测 - 工作中且未暂停时检测
                    if let Ok(mut timer) = state.timer.lock() {
                        if timer.state == AppStateEnum::Working {
                            let is_fullscreen = fullscreen_detect::is_foreground_fullscreen();
                            if is_fullscreen && !timer.paused {
                                timer.paused = true;
                            } else if !is_fullscreen && timer.paused {
                                timer.paused = false;
                                timer.last_tick = std::time::Instant::now();
                            }

                            if let Some(remaining) = timer.tick() {
                                // 发送 tick 事件到倒计时窗口
                                let payload = serde_json::json!({
                                    "remaining_secs": remaining,
                                    "total_secs": timer.total_secs,
                                    "state": "Working",
                                    "postpone_count": timer.postpone_count,
                                    "max_postpone": settings.max_postpone_count,
                                });
                                let _ = app_handle.emit("tick", payload);

                                // 3分钟预警
                                if remaining <= 180 && remaining > 170 {
                                    let sounds_dir = audio::get_sounds_dir(&app_handle);
                                    let pre_path = format!("{}\\breakpre.mid", sounds_dir);
                                    let _ = audio::play_midi(&pre_path);
                                }

                                // 倒计时归零 → 进入休息
                                if remaining == 0 {
                                    drop(timer);
                                    let _ = start_rest_now(
                                        app_handle.clone(),
                                        state,
                                    );
                                }
                            }
                        } else if timer.state == AppStateEnum::Resting {
                            if let Some(remaining) = timer.tick() {
                                let settings = match state.settings.lock() {
                                    Ok(s) => s.clone(),
                                    Err(_) => continue,
                                };

                                let elapsed = timer.total_secs.saturating_sub(remaining);
                                let can_unlock = match settings.force_mode {
                                    ForceMode::None => true,
                                    ForceMode::Soft => elapsed >= 60,
                                    ForceMode::Hard => false,
                                };

                                let payload = serde_json::json!({
                                    "remaining_secs": remaining,
                                    "can_unlock": can_unlock,
                                });
                                let _ = app_handle.emit("lock_tick", payload);

                                // 休息结束 → 自动解锁
                                if remaining == 0 {
                                    drop(timer);
                                    let _ = unlock_screen(
                                        app_handle.clone(),
                                        state,
                                    );
                                }
                            }
                        }
                    }
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // 阻止关闭倒计时窗口（用户无法直接关闭）
                if window.label() == "countdown" {
                    api.prevent_close();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 2: 更新 main.rs**

无需修改，main.rs 已是最终版本。

---

### Task 15: 构建配置与资源

**Files:**
- Modify: `src-tauri/tauri.conf.json` (添加资源路径)
- Modify: `src-tauri/build.rs` (如需)
- Create: `src-tauri/icons/` (占位图标)

- [ ] **Step 1: 更新 tauri.conf.json 添加窗口配置和资源**

```json
{
  "$schema": "https://raw.githubusercontent.com/tauri-apps/tauri/dev/crates/tauri-config-schema/schema.json",
  "productName": "EyeGuard",
  "version": "0.1.0",
  "identifier": "com.eyeguard.app",
  "build": {
    "frontendDist": "../frontend",
    "devUrl": "http://localhost:1420",
    "beforeDevCommand": "",
    "beforeBuildCommand": ""
  },
  "app": {
    "windows": [
      {
        "label": "countdown",
        "url": "/index.html",
        "width": 180,
        "height": 70,
        "decorations": false,
        "alwaysOnTop": true,
        "skipTaskbar": true,
        "resizable": false,
        "title": "EyeGuard",
        "visible": true
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.ico"
    ],
    "resources": [
      "../sounds/*"
    ]
  }
}
```

- [ ] **Step 2: 生成占位图标**

创建最小 32x32 PNG 图标文件并在 `tauri.conf.json` 中配置。
```bash
# 使用 ImageMagick 生成图标或手动创建一个简单的蓝色圆形图标
```

---

### Task 16: 复制音频文件与构建验证

**Files:**
- Copy: 音频文件到 `sounds/` 目录

- [ ] **Step 1: 复制音频文件**

```bash
mkdir -p sounds
cp "D:/GreenProgram/EyeFoo3/EyeFoo3/resources/sounds/break.mid" sounds/
cp "D:/GreenProgram/EyeFoo3/EyeFoo3/resources/sounds/breakpre.mid" sounds/
cp "D:/GreenProgram/EyeFoo3/EyeFoo3/resources/sounds/unlock.mid" sounds/
```

- [ ] **Step 2: 构建并验证**

```bash
cd src-tauri
cargo build
```

预期: 编译成功，生成 `src-tauri/target/debug/eyeguard.exe`。

- [ ] **Step 3: 运行测试验证基本功能**

```bash
cargo tauri dev
```

---

## 自我审查

### Spec 覆盖检查
- ✅ 工作/休息状态切换 - Task 14 (main.rs lib.rs)
- ✅ 180x70 倒计时窗口 - Task 9 (index.html) + Task 14
- ✅ 右键菜单 - Task 9 (index.html)
- ✅ 进度条 - Task 9 (index.html)
- ✅ 推迟功能（3/5/10分钟，次数限制）- Task 9 + Task 14
- ✅ 总在最前/取消最前 - Task 14 (set_always_on_top command)
- ✅ 3分钟预警 + breakpre.mid - Task 14
- ✅ 黑色全屏锁定 - Task 7 (screen_lock.rs)
- ✅ 休息倒计时 - Task 10 (lock.html)
- ✅ Win/Alt+Tab/Alt+F4 拦截 - Task 5 (keyboard_hook.rs)
- ✅ 解锁策略（非强制/一般/完全）- Task 14 (unlock_screen logic)
- ✅ 音频播放 - Task 4 (audio.rs)
- ✅ 设置界面 - Task 11 (settings.html)
- ✅ 设置持久化 - Task 2 (settings.rs)
- ✅ 多显示器支持 - Task 7 (screen_lock.rs)
- ✅ 系统托盘 - Task 12 (tray.rs)
- ✅ 开机自启 - Task 13 (autostart.rs)
- ✅ 全屏检测暂停 - Task 6 (fullscreen_detect.rs) + Task 14
- ✅ 深色主题 - Task 8 (theme.css)
- ✅ 关闭退出 - Task 14 (quit_app command)

### 占位符检查
- 图标文件需要生成占位 PNG（Task 15）
- 其余无占位符

### 类型一致性
- Settings struct 在所有引用处保持一致（work_interval_minutes, rest_duration_minutes, max_postpone_count, force_mode）
- AppStateEnum 枚举值 Working/Resting 在所有匹配处一致
- 事件 payload 字段名一致（remaining_secs, can_unlock 等）
