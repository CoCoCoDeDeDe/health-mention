# 架构设计

> 基于 `docs/01-requirements.md`，技术栈：**Tauri 2（Rust 后端）+ WebView2（前端）**。
> 修订（2026-07-27）：单一「休息」提醒（无类型区分、无合并逻辑）；新增全局快捷键模块。

## 总体结构

单进程 Tauri 应用，两个窗口：

| 窗口 | 说明 |
|---|---|
| `main` | 设置 + 记录统计（两个 tab），平时隐藏，托盘菜单或快捷键打开 |
| `overlay` | 休息进度提示窗，无边框、置顶；小浮窗/全屏两种模式，平时隐藏 |

前端：Vite + TypeScript，不引入 UI 框架（界面规模小，保持 bundle 极小；日后复杂了可再加 Preact）。

## 后端模块（src-tauri/src）

| 模块 | 职责 |
|---|---|
| `scheduler.rs` | 休息计时（单计时器）、到期触发、暂停/恢复 |
| `session.rs` | 休息会话状态机（running → done/stopped），写记录 |
| `tray.rs` | 托盘图标与菜单：设置、立即休息、暂停/恢复、查看记录、退出 |
| `hotkey.rs` | 全局快捷键注册/注销，触发行为与托盘菜单一致 |
| `storage.rs` | settings.json 读写、logs/*.jsonl 追加 |
| `stats.rs` | 由 logs 现算：今日完成次数、连续坚持天数 |
| `plugin.rs` | 扫描/读取用户自定义组件与主题 |
| `commands.rs` | 所有 `#[tauri::command]` 入口 |

官方插件：`tauri-plugin-single-instance`、`tauri-plugin-autostart`、`tauri-plugin-global-shortcut`（托盘能力为 Tauri 2 内置）。

## 跟随系统自启动

- 基于 `tauri-plugin-autostart`，设置页开关，默认关闭
- 状态持久化于 `settings.json` 的 `global.autostart`，切换时即时调用插件 enable/disable，无需重启

## 调度器设计

- 单 tick 循环（1s）：检查休息提醒的 `next_due`，暂停即冻结
- 到期 → 开启**休息会话**：`planned_sec = duration_sec`
- 会话结束：倒计时归零 → `done`；用户提前结束 → `stopped`；写一条记录后重置计时器
- 手动休息：托盘菜单或快捷键「立即休息」→ 立即开启会话（`trigger = manual`）；会话进行中重复触发则忽略
- 倒计时渲染由 overlay 前端本地进行，后端只发开始/结束事件并权威记录结果

## 全局快捷键

- 基于 `tauri-plugin-global-shortcut` 注册系统级快捷键
- 三个可配置动作：`breakNow`（立即休息）、`togglePause`（暂停/恢复）、`openSettings`（打开设置）
- `settings.json` 中每个动作存 accelerator 字符串（如 `"Ctrl+Alt+B"`）或 `null`，**null 即关闭**
- 设置页提供按键录制输入框（捕获 keydown 生成 accelerator），保存时后端先注销旧快捷键再注册新；注册失败（按键被占用）返回错误提示用户
- 设置变更即时生效，无需重启

## 事件与命令

- 事件：`break://start {plannedSec, trigger}`、`break://end {result}`、`settings://changed`
- 命令：`get_settings` / `save_settings`（含快捷键变更）、`list_logs(month)` / `get_stats`、`trigger_break()` / `end_break()`、`set_paused(bool)`、`list_progress_plugins` / `read_progress_plugin(name)`、`list_themes` / `read_theme(name)`

## 自定义组件契约（核心扩展点）

**进度指示组件**：目录 `<app_data>/plugins/progress/<组件名>/`

```
component.js   ES module，必需
meta.json      { name, version, author }，可选
style.css      可选
```

`component.js` 契约：

```js
export default {
  // ctx: { durationSec: number, mode: 'float' | 'fullscreen' }
  mount(el, ctx) {
    return {
      // state: { progress: 0..1, remainingSec: number }
      update(state) {},
      unmount() {}
    };
  }
};
```

加载流程：后端扫描目录 → 前端经 blob URL 动态 `import()` → 校验导出结构 → `mount` 全过程 try/catch，**任何失败回退内置默认进度条**。

**主题（设置窗口风格自定义）**：`<app_data>/themes/<主题名>.css`，纯 CSS 变量覆盖（`--bg`、`--fg`、`--accent`、圆角等），设置页下拉选择，即选即用。这比"整个窗口换组件"轻得多，首期足够。

## 存储

目录：`<app_data>`（Tauri path API，Windows 下为 `%APPDATA%/health-mention`）

```
settings.json        { version: 1, break: {...}, hotkeys: {...},
                       global: { paused, autostart, overlayMode, theme, progressPlugin } }
logs/YYYY-MM.jsonl   每行一条休息记录
themes/、plugins/progress/   用户自定义内容
```

记录格式：

```json
{"ts": "2026-07-27T14:40:00+08:00", "trigger": "scheduled",
 "plannedSec": 60, "actualSec": 45, "result": "stopped"}
```

统计口径：今日完成次数 = 今日 `result=done` 记录数；连续天数 = 从今天向前连续存在 done 记录的天数。

## 工程工作流

- WSL：编辑代码、git 管理
- Windows 11：安装 Rust 工具链 + Node.js（WebView2 系统自带），`pnpm tauri dev` 调试，`pnpm tauri build` 出单 exe（nsis 安装包或绿色 exe）

## 里程碑

- M1 脚手架：Tauri 工程、托盘、设置页读写 settings.json
- M2 调度器 + overlay 默认进度条 + 休息记录落盘
- M3 记录列表 + 简单统计 + 手动休息 + 全局快捷键
- M4 自定义进度组件加载 + 主题选择
- M5 打包、自启、打磨
