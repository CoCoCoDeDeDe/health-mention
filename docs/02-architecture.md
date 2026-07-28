# 架构设计

> 基于 `docs/01-requirements.md`，技术栈：**Tauri 2（Rust 后端）+ WebView2（前端）**。
> 修订（2026-07-27）：单一「休息」提醒（无类型区分、无合并逻辑）；新增全局快捷键模块。
> 修订（2026-07-27）：overlay 改为按显示器动态创建多窗口；纯进度条交互；记录按日查询。

## 总体结构

单进程 Tauri 应用：

| 窗口 | 说明 |
|---|---|
| `main` | 设置 + 记录统计（两个 tab），可调整大小，平时隐藏；托盘双击/菜单/快捷键打开 |
| `overlay-N` | 休息进度提示窗，**会话开始时按显示器数量动态创建**（每屏一个），无边框、置顶，会话结束统一销毁 |

前端：Vite + TypeScript，不引入 UI 框架（界面规模小，保持 bundle 极小；日后复杂了可再加 Preact）。

## 后端模块（src-tauri/src）

| 模块 | 职责 |
|---|---|
| `scheduler.rs` | 休息计时（单计时器）、到期触发、暂停/恢复 |
| `session.rs` | 休息会话状态机（running → done/stopped），写记录，多 overlay 窗口创建/销毁 |
| `tray.rs` | 托盘图标与菜单：设置、立即休息、暂停/恢复、退出；双击打开设置 |
| `hotkey.rs` | 全局快捷键注册/注销，触发行为与托盘菜单一致 |
| `storage.rs` | settings.json 读写、logs/*.jsonl 追加 |
| `stats.rs` | 按日读取记录；由 logs 现算：今日完成次数、连续坚持天数 |
| `plugin.rs` | 扫描/读取用户自定义组件与主题（M4） |
| `main.rs` | 应用装配与所有 `#[tauri::command]` 入口 |

官方插件：`tauri-plugin-single-instance`、`tauri-plugin-autostart`、`tauri-plugin-global-shortcut`（托盘能力为 Tauri 2 内置）。

## 跟随系统自启动

- 基于 `tauri-plugin-autostart`，设置页开关，默认关闭
- 状态持久化于 `settings.json` 的 `global.autostart`，切换时即时调用插件 enable/disable，无需重启

## 调度器设计

- 单 tick 循环（1s）：检查休息提醒的 `next_due`，暂停即冻结
- 到期 → 开启**休息会话**：`planned_sec = duration_sec`
- 会话结束：倒计时归零 → `done`；用户提前结束 → `stopped`；写一条记录后重置计时器
- 手动休息：托盘菜单或快捷键「立即休息」→ 立即开启会话（`trigger = manual`）；会话进行中重复触发则忽略
- 倒计时渲染由各 overlay 前端本地进行（`get_break_state` 拉取 `{plannedSec, elapsedSec, mode}` 推算截止时刻），后端权威记录结果
- 下一次休息倒计时：tick 线程每秒更新托盘 tooltip；设置页每秒调用 `get_schedule_state` 拉取 `{remainingSec, status}` 实时显示；status ∈ running / paused / disabled / inSession

## 休息窗口（overlay）

- 会话开始时枚举显示器（`available_monitors`），**每个显示器创建一个 `overlay-N` 窗口**：浮窗模式初始 320×44、各屏居中；全屏模式各屏全屏；会话结束统一 `destroy`
- 窗口内容**只有一根进度条**：
  - 浮窗：左键拖拽移动（位移超 4px 才 `startDragging`，不吞双击）、右下角手柄调整宽高（`setSize`，最小 120×28）
  - **双击进度条 = 提前结束**；Alt+F4 关闭任一 overlay 也视为提前结束
  - 各窗口移动/调整各自独立，不同步
- 建窗 `focused(false)` + `focusable(false)`：不夺取输入焦点
- 位置/宽高记忆：各窗口前端每 2s 上报 rect（`save_overlay_rect`，仅变化时写 `overlay-rects.json`），overlay 建窗与 main 窗口启动时优先使用
- 预览：设置页「预览休息浮窗」开关（`set/get_overlay_preview`），无会话时 overlay 显示静态半条用于调整；会话开始时自动关闭预览并重建窗口
- 注：建窗只用 `focused(false)`；`focusable(false)` 在 Windows 上会导致窗口白屏卡死，勿用
- capabilities 用 `overlay-*` 匹配动态窗口标签，附加 `start-dragging` / `set-size` / `close` 权限

## 全局快捷键

- 基于 `tauri-plugin-global-shortcut` 注册系统级快捷键
- 四个可配置动作：`breakNow`（立即休息）、`toggleEnabled`（开关启用提醒）、`endBreak`（手动结束休息）、`openSettings`（打开设置）
- `settings.json` 中每个动作存 accelerator 字符串（如 `"Ctrl+Alt+B"`）或 `null`，**null 即关闭**
- 设置页提供按键录制输入框（捕获 keydown 生成 accelerator），保存时后端先注销旧快捷键再注册新；注册失败（按键被占用）返回错误提示用户
- 设置变更即时生效，无需重启；快捷键切换暂停时同步托盘勾选状态

## 事件与命令

- 事件：`break://start {plannedSec, trigger}`、`break://end {result}`（保留；前端状态以主动查询为准）
- 命令：`get_settings` / `save_settings`（含快捷键变更）、`list_logs(date)` / `get_stats`、`trigger_break()` / `end_break()`、`get_break_state()` / `get_schedule_state()`、`list_progress_plugins` / `read_progress_plugin(name)`（M4）、`list_themes` / `read_theme(name)`（M4）

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

统计口径：今日完成次数 = 今日 `result=done` 记录数；连续天数 = 从今天向前连续存在 done 记录的天数（当天尚无 done 时从昨天往前数）。

## 工程工作流

- WSL：编辑代码、git 管理
- Windows 11：安装 Rust 工具链 + Node.js（WebView2 系统自带），`npm run tauri dev` 调试，`npm run tauri build` 出单 exe

## 里程碑

- M1 脚手架：Tauri 工程、托盘、设置页读写 settings.json
- M2 调度器 + overlay 默认进度条 + 休息记录落盘
- M3 记录列表（按日）+ 简单统计 + 全局快捷键 + 多显示器 overlay + 纯进度条浮窗
- M4 自定义进度组件加载 + 主题选择
- M5 打包、自启、打磨
