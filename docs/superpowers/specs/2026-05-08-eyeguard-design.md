# 爱眼卫士 (EyeGuard) 设计文档

> 版本: 1.0
> 日期: 2026-05-08
> 技术栈: Rust + Tauri v2 + Vanilla HTML/CSS/JS

---

## 1. 概述

爱眼卫士是一款 Windows 桌面护眼提醒软件，采用"工作-休息"交替模式运行。工作时在桌面右上角显示倒计时窗口，到设定时间后锁定屏幕强制休息，保护用户视力。

### 核心约束

- 纯 Windows 桌面应用 (exe)
- 基于 Rust + Tauri v2 框架
- 原生调用 Windows API 实现底层功能
- 深色系 UI 主题 (深海蓝配色)

---

## 2. 系统架构

### 架构模式: Tauri v2 混合架构

```
┌───────────────────────────────────────────────────────────────┐
│                    Tauri v2 Application                       │
│                                                               │
│  ┌───────────────────┐  ┌────────────────────────────────┐   │
│  │   Frontend (Web)    │  │    Rust Backend                │   │
│  │   Vanilla JS       │  │   (Windows Native)             │   │
│  │                    │  │                                │   │
│  │ • 倒计时窗口 UI    │◄─┤ Tauri Commands + Events      │   │
│  │ • 锁定遮罩 UI     │  │                                │   │
│  │ • 设置界面         │  │ • keyboard_hook - 全局键盘钩子  │   │
│  │ • 系统托盘交互     │  │ • screen_lock - 屏幕锁定管理   │   │
│  └───────────────────┘  │ • fullscreen_detect - 全屏检测  │   │
│                          │ • audio - MCI 音频播放 (.mid)   │   │
│                          │ • settings - 设置持久化 (TOML)  │   │
│                          │ • autostart - 开机自启管理     │   │
│                          └────────────────────────────────┘   │
└───────────────────────────────────────────────────────────────┘
```

### 技术选型

| 组件 | 技术 | 说明 |
|------|------|------|
| 应用框架 | Tauri v2 | 窗口管理、系统托盘、进程间通信 |
| 前端 UI | Vanilla HTML/CSS/JS | 倒计时窗口、锁定屏幕、设置界面 |
| 后端语言 | Rust | 所有业务逻辑 + Windows API 调用 |
| Win API 绑定 | `windows-sys` 或 `winapi` crate | 键盘钩子、全屏检测、MCI 播放 |
| 设置持久化 | `confy` 或 `toml` + `serde` | 读写配置文件 |
| 窗口管理 | `tauri::window` API | 多窗口创建、全屏遮罩 |

### 核心模块

| 模块文件 | 职责 |
|----------|------|
| `src/main.rs` | 应用入口、Tauri 初始化、系统托盘、状态管理 |
| `src/keyboard_hook.rs` | 全局低层键盘钩子 (WH_KEYBOARD_LL) |
| `src/screen_lock.rs` | 多显示器全屏遮罩窗口管理 |
| `src/fullscreen_detect.rs` | 前台窗口全屏状态检测 |
| `src/audio.rs` | Windows MCI API 播放 MIDI |
| `src/settings.rs` | 设置结构体定义、读写 TOML 文件 |
| `src/autostart.rs` | 注册表方式管理开机自启 |
| `src/state.rs` | 应用全局状态管理 (计时器、状态机) |
| `src/tray.rs` | 系统托盘图标及菜单 |

---

## 3. 状态机

### 应用三种状态

```
WORKING ────→ RESTING ────→ WORKING
    ↑                        │
    └────────────────────────┘
          (循环交替)
```

### WORKING 状态

- 桌面右上角显示 180×70 倒计时窗口
- 格式 MM:SS，每秒刷新
- 进度条从左到右逐渐消退
- 全屏检测：检测到前台窗口全屏 → 暂停倒计时 → 恢复后续计
- 右键菜单：立即休息、推迟3/5/10分钟、总在最前/取消、设置、退出
- 倒计时 ≤ 3分钟：弹出提示 + 播放 breakpre.mid
- 倒计时归零：进入 RESTING 状态

**推迟逻辑：**
- 每次推迟在原剩余时间上加 3/5/10 分钟
- 每次点击消耗一次推迟次数
- 总次数不能超过设置中的上限 (1-6次)

### RESTING 状态

- 隐藏工作时倒计时窗口
- 在所有显示器上创建黑色全屏遮罩窗口
- 屏幕正中央显示休息倒计时 (MM:SS)
- 拦截 Win 键、Alt+Tab、Alt+F4
- 播放 break.mid 一遍
- 休息结束 → 自动解锁 → 播放 unlock.mid → 回到 WORKING

**解锁策略：**

| 模式 | 行为 |
|------|------|
| 非强制 | 解锁图标始终显示，点击立即解锁 |
| 一般强制 | 休息 1 分钟后右下角显示解锁图标 |
| 完全强制 | 不显示解锁图标，不可解锁 |

---

## 4. UI 设计

### 4.1 配色方案 (深海蓝)

| 角色 | 色值 | 用途 |
|------|------|------|
| 主背景色 | `#0a1628` | 窗口背景 |
| 强调色 | `#4fc3f7` | 倒计时数字、按钮、高亮 |
| 辅色 | `#1a3a6a` | 进度条、渐变底色 |
| 深色辅助 | `#0f3460` | 边框、分割线 |
| 文字色 | `#e0e0e0` | 主要文字 |
| 次级文字 | `#78909c` | 辅助文字 |
| 遮罩背景 | `#000000` | 锁定屏幕背景 |

### 4.2 工作时倒计时窗口

- **尺寸**: 180 × 70 px，固定不可变
- **位置**: 桌面右上角 (可拖拽)
- **样式**: 无边框、无标题栏、无系统按钮
- **置顶**: 可切换始终置顶
- **背景**: 半透明，内嵌进度条

**进度条**: 背景填充渐变，由右向左消退，表示剩余时间比例。

**右键菜单**: `立即休息 | 推迟3分 | 推迟5分 | 推迟10分 | 总在最前/取消最前 | 设置属性 | 关闭退出`

### 4.3 休息锁定屏幕

- 全屏纯黑色覆盖所有显示器
- 居中大号字体显示倒计时 MM:SS
- 右下角显示解锁图标 (根据模式决定可见性)
- 输入拦截: Win 键、Alt+Tab、Alt+F4 被全局钩子过滤

### 4.4 设置界面

模态窗口，不保存则恢复原值：

| 设置项 | 类型 | 说明 |
|--------|------|------|
| 工作时间间隔 | 下拉选择 (1-120分钟) | 默认 25 分钟 |
| 休息时间长度 | 下拉选择 (1-30分钟) | 默认 5 分钟 |
| 允许推迟次数 | 下拉选择 (1-6) | 默认 3 次 |
| 强制休息 | 勾选框 | 启用后可选模式 |
| 强制模式 | 下拉选择 (一般/完全) | 勾选强制后可选 |

### 4.5 系统托盘

- 图标: 应用自定义 icon
- 左键: 打开设置窗口
- 右键菜单: 打开设置 / 退出

---

## 5. 数据流

### 计时器流程

```
Rust Timer (1s interval)
    → 计算剩余秒数
    → 更新 AppState
    → 通过 Tauri Event "tick" 发送到前端
    → 前端更新 DOM 显示 MM:SS + 进度条宽度
    → 检查是否触发 3min 预警/归零
```

### 设置保存流程

```
前端设置表单修改 → 点击"保存"
    → Tauri Command "save_settings"
    → Rust 校验 → 写入 TOML 文件
    → 更新 AppState → 实时生效

点击"取消" → 关闭窗口 → 恢复原值 (前端保留备份)
```

---

## 6. 关键 Windows API 调用

| 功能 | API | 说明 |
|------|-----|------|
| 全局键盘钩子 | `SetWindowsHookEx(WH_KEYBOARD_LL, ...)` | 拦截 Win/Alt+Tab/Alt+F4 |
| 全屏检测 | `GetForegroundWindow()` + `GetWindowRect()` + `GetMonitorInfo()` | 检测前台窗口是否覆盖全屏 |
| MCI 音频 | `mciSendString("play break.mid", ...)` | 播放 MIDI 文件 |
| 多屏枚举 | `EnumDisplayMonitors()` | 获取所有显示器信息 |
| 开机自启 | RegSetValueEx(HKCU\...\Run) | 注册表方式设置开机自启 |

---

## 7. 配置持久化

格式: TOML，存储路径: `%APPDATA%/EyeGuard/settings.toml`

```toml
work_interval_minutes = 25
rest_duration_minutes = 5
max_postpone_count = 3
force_mode = "soft"  # "none" | "soft" | "hard"
```

---

## 8. 边界场景处理

| 场景 | 处理方式 |
|------|----------|
| 用户关机重启 | 所有状态重置，重新开始工作计时 |
| 全屏时倒计时暂停 | 每 1 秒检测前台窗口全屏状态，全屏时暂停 Timer |
| 推迟次数耗尽 | 右键推迟菜单项置灰/隐藏 |
| 修改设置正在休息 | 休息中的时长生效当前休息，下次休息用新设置 |
| 多显示器扩展屏 | EnumDisplayMonitors 获取所有显示器，每个创建遮罩窗口 |
| 托盘图标退出 | 清理键盘钩子、销毁所有窗口、退出进程 |

---

## 9. 项目结构

```
EyeGuard/
├── src/
│   ├── main.rs              # 应用入口
│   ├── state.rs             # 全局状态管理
│   ├── keyboard_hook.rs     # 全局键盘钩子
│   ├── screen_lock.rs       # 屏幕锁定管理
│   ├── fullscreen_detect.rs # 全屏检测
│   ├── audio.rs             # MIDI 音频播放
│   ├── settings.rs          # 设置持久化
│   ├── autostart.rs         # 开机自启
│   └── tray.rs              # 系统托盘
├── src-tauri/
│   └── tauri.conf.json      # Tauri 配置
├── frontend/
│   ├── index.html           # 倒计时窗口
│   ├── lock.html            # 锁定屏幕
│   ├── settings.html        # 设置界面
│   └── styles/
│       └── theme.css        # 深海蓝主题样式
├── sounds/                  # 音频文件 (构建后复制)
│   ├── break.mid
│   ├── breakpre.mid
│   └── unlock.mid
├── icons/                   # 应用图标
├── docs/
│   └── superpowers/
│       └── specs/
│           └── 2026-05-08-eyeguard-design.md
└── Cargo.toml
```
