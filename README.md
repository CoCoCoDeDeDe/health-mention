# health-mention

轻量 Windows 桌面休息提醒工具。技术栈：Tauri 2（Rust）+ WebView2，设计见 `docs/`。

## 开发

分工：WSL 里写代码与 git 管理；Windows 11 侧构建运行（GUI、托盘必须在 Windows 验证）。

### Windows 侧首次准备

1. 安装 [Node.js LTS](https://nodejs.org/)
2. 安装 [Visual Studio C++ 生成工具](https://visualstudio.microsoft.com/visual-cpp-build-tools/)（勾选「使用 C++ 的桌面开发」工作负载）
3. 安装 [rustup](https://rustup.rs/)（默认 x86_64-pc-windows-msvc 工具链）
4. WebView2：Windows 11 系统自带，无需安装

### Windows 侧拉取与运行（PowerShell）

```powershell
git clone -b 202607271440-init git@github-cococo:CoCoCoDeDeDe/health-mention.git C:\Users\Dell\health-mention-dev
cd C:\Users\Dell\health-mention-dev
npm install
npm run tauri dev    # 调试运行（首次 cargo 编译较久）
npm run tauri build  # 打包出 exe
```

### 日常迭代

WSL 提交并 push → Windows `git pull` → `npm run tauri dev`。

前端纯 UI 可在 WSL 跑 `npm run dev`，Windows 浏览器打开 http://localhost:1420 预览（WSL2 localhost 自动转发；浏览器 mock 待加）。

## 里程碑

- [x] M1 脚手架：Tauri 工程、托盘、设置页读写 settings.json、自启动开关
- [x] M2 调度器 + overlay 默认进度条 + 休息记录落盘
- [ ] M3 记录列表 + 简单统计 + 手动休息 + 全局快捷键
- [ ] M4 自定义进度组件加载 + 主题选择
- [ ] M5 打包、打磨
