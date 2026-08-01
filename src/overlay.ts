import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";

interface BreakState {
  plannedSec: number;
  elapsedSec: number;
  mode: string;
  trigger: string;
  progressPlugin: string | null;
}

interface PluginFiles {
  componentJs: string;
  styleCss: string | null;
}

interface ComponentCtx {
  durationSec: number;
  mode: string;
}

interface ComponentState {
  progress: number;
  remainingSec: number;
  phase: "countdown" | "overtime";
  elapsedSec: number;
}

interface ComponentHandle {
  update(state: ComponentState): void;
  unmount?(): void;
}

const win = getCurrentWindow();
const track = document.getElementById("bar-track") as HTMLDivElement;
const bar = document.getElementById("bar") as HTMLDivElement;
const grip = document.getElementById("grip") as HTMLDivElement;
const otText = document.getElementById("ot-text") as HTMLSpanElement;
const customRoot = document.getElementById("custom-root") as HTMLDivElement;

const state = await invoke<BreakState | null>("get_break_state");

if (!state) {
  // 无会话（如会话已结束窗口残留），直接关闭
  await win.close();
} else {
  // 尝试加载自定义进度组件；任何失败回退内置进度条
  let custom: ComponentHandle | null = null;
  if (state.progressPlugin) {
    custom = await loadCustomComponent(state.progressPlugin, {
      durationSec: state.plannedSec,
      mode: state.mode,
    }).catch((e) => {
      console.error("custom progress component failed, fallback to default:", e);
      return null;
    });
  }

  if (custom) {
    track.classList.add("hidden"); // 自定义组件接管渲染区域
  }

  const render = (
    progress: number,
    remainingSec: number,
    phase: "countdown" | "overtime",
    elapsedSec: number
  ) => {
    if (custom) {
      custom.update({ progress: Math.min(1, progress), remainingSec, phase, elapsedSec });
    } else {
      bar.style.width = `${(Math.min(1, progress) * 100).toFixed(1)}%`;
    }
  };

  if (state.mode === "fullscreen") {
    document.body.classList.add("fullscreen");
  } else {
    grip.classList.remove("hidden");
    setupDragAndResize(custom ? customRoot : track);
    setupRectPersistence();
  }

  // 双击 = 提前结束（拖拽已加位移阈值，不与双击冲突）
  document.body.addEventListener("dblclick", () => {
    invoke("end_break").catch(console.error);
  });

  if (state.trigger === "manual") {
    runManual(state, render, custom === null);
  } else {
    runScheduled(state, render);
  }

  // 首帧绘制完成后通知后端显示窗口（窗口先隐藏建出，避免白闪）
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      invoke("overlay_ready", { label: win.label }).catch(() => {});
    });
  });
}

/** 加载并挂载自定义组件；结构不符或抛错时向上抛（调用方回退） */
async function loadCustomComponent(
  name: string,
  ctx: ComponentCtx
): Promise<ComponentHandle | null> {
  const files = await invoke<PluginFiles>("read_progress_plugin", { name });
  if (files.styleCss) {
    const style = document.createElement("style");
    style.textContent = files.styleCss;
    document.head.appendChild(style);
  }
  const blob = new Blob([files.componentJs], { type: "text/javascript" });
  const url = URL.createObjectURL(blob);
  let mod: { default?: { mount?: unknown } };
  try {
    mod = await import(/* @vite-ignore */ url);
  } finally {
    URL.revokeObjectURL(url);
  }
  const def = mod.default;
  if (!def || typeof def.mount !== "function") {
    throw new Error("component.js 缺少 default export 的 mount(el, ctx)");
  }
  const handle = (def.mount as (el: HTMLElement, ctx: ComponentCtx) => ComponentHandle)(
    customRoot,
    ctx
  );
  if (!handle || typeof handle.update !== "function") {
    throw new Error("mount 返回值缺少 update(state)");
  }
  return handle;
}

/** 定时休息：倒计时跑完由后端自动结束 */
function runScheduled(
  state: BreakState,
  render: (p: number, r: number, phase: "countdown" | "overtime", e: number) => void
): void {
  const endAt = Date.now() + (state.plannedSec - state.elapsedSec) * 1000;
  const update = () => {
    const remainingMs = Math.max(0, endAt - Date.now());
    const remainingSec = remainingMs / 1000;
    render(1 - remainingSec / state.plannedSec, remainingSec, "countdown", state.plannedSec - remainingSec);
    if (remainingMs <= 0) clearInterval(timer);
  };
  const timer = setInterval(update, 200);
  update();
}

/** 手动休息：先倒计时跑完计划时长，再转为正计时（不自动退出） */
function runManual(
  state: BreakState,
  render: (p: number, r: number, phase: "countdown" | "overtime", e: number) => void,
  useDefault: boolean
): void {
  const startStamp = Date.now() - state.elapsedSec * 1000;
  const plannedMs = state.plannedSec * 1000;
  const update = () => {
    const elapsedMs = Date.now() - startStamp;
    const elapsed = elapsedMs / 1000;
    if (elapsedMs < plannedMs) {
      if (useDefault) otText.classList.add("hidden");
      render(elapsedMs / plannedMs, (plannedMs - elapsedMs) / 1000, "countdown", elapsed);
      return;
    }
    render(elapsed / denominator(elapsed), 0, "overtime", elapsed);
    if (useDefault) {
      otText.classList.remove("hidden");
      otText.textContent = fmtElapsed(elapsed);
    }
  };
  const timer = setInterval(update, 250);
  update();
  void timer;
}

function denominator(sec: number): number {
  if (sec < 3600) return 3600; // 分母 1 小时
  if (sec < 86400) return 86400; // 分母 1 天
  return 604800; // 分母 7 天
}

function fmtElapsed(sec: number): string {
  const t = Math.floor(sec);
  const d = Math.floor(t / 86400);
  const h = Math.floor((t % 86400) / 3600);
  const m = Math.floor((t % 3600) / 60);
  const s = t % 60;
  if (d > 0) return `已休息 ${d} 天 ${h} 小时`;
  if (h > 0) return `已休息 ${h} 小时 ${m} 分`;
  return `已休息 ${m} 分 ${s} 秒`;
}

function setupDragAndResize(surface: HTMLElement): void {
  // 按住移动超过阈值才拖拽，避免吃掉双击
  surface.addEventListener("mousedown", (e) => {
    if (e.button !== 0) return;
    const startX = e.screenX;
    const startY = e.screenY;
    let dragging = false;
    const onMove = (ev: MouseEvent) => {
      if (!dragging && Math.hypot(ev.screenX - startX, ev.screenY - startY) > 4) {
        dragging = true;
        win.startDragging().catch(console.error);
        cleanup();
      }
    };
    const onUp = () => cleanup();
    const cleanup = () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  });

  // 右下角手柄调整宽高
  grip.addEventListener("mousedown", (e) => {
    e.preventDefault();
    e.stopPropagation();
    const startX = e.screenX;
    const startY = e.screenY;
    let startW = 0;
    let startH = 0;
    Promise.all([win.innerSize(), win.scaleFactor()])
      .then(([size, scale]) => {
        startW = size.width / scale;
        startH = size.height / scale;
      })
      .catch(console.error);
    const onMove = (ev: MouseEvent) => {
      if (!startW) return;
      const w = Math.max(60, startW + (ev.screenX - startX));
      // Windows 最小跟踪高度约 39，再小系统会夹紧
      const h = Math.max(39, startH + (ev.screenY - startY));
      win.setSize(new LogicalSize(w, h)).catch(console.error);
    };
    const onUp = () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      // 松开手柄立即记录，不依赖 onResized 事件
      reportRect();
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  });
}

/** 立即上报本窗口位置与宽高 */
function reportRect(): void {
  Promise.all([win.outerPosition(), win.innerSize(), win.scaleFactor()])
    .then(([pos, size, scale]) =>
      invoke("save_overlay_rect", {
        label: win.label,
        rect: {
          x: pos.x / scale,
          y: pos.y / scale,
          w: size.width / scale,
          h: size.height / scale,
        },
      })
    )
    .catch((e) => console.error("reportRect failed:", e));
}

/** 记忆本窗口位置与宽高：移动/缩放事件上报（300ms 防抖），另有 2s 轮询兜底 */
function setupRectPersistence(): void {
  let timer: ReturnType<typeof setTimeout> | null = null;
  const debounced = () => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(reportRect, 300);
  };
  win.onMoved(debounced).catch(() => {});
  win.onResized(debounced).catch(() => {});
  setInterval(reportRect, 2000);
}
