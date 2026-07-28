import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";

interface BreakState {
  plannedSec: number;
  elapsedSec: number;
  mode: string;
  trigger: string;
}

const win = getCurrentWindow();
const track = document.getElementById("bar-track") as HTMLDivElement;
const bar = document.getElementById("bar") as HTMLDivElement;
const grip = document.getElementById("grip") as HTMLDivElement;
const otText = document.getElementById("ot-text") as HTMLSpanElement;

const state = await invoke<BreakState | null>("get_break_state");

if (!state) {
  // 无会话（如会话已结束窗口残留），直接关闭
  await win.close();
} else {
  if (state.mode === "fullscreen") {
    document.body.classList.add("fullscreen");
  } else {
    grip.classList.remove("hidden");
    setupDragAndResize();
    setupRectPersistence();
  }

  // 双击 = 提前结束（拖拽已加位移阈值，不与双击冲突）
  document.body.addEventListener("dblclick", () => {
    invoke("end_break").catch(console.error);
  });

  if (state.trigger === "manual") {
    runManual(state);
  } else {
    runScheduled(state);
  }

  // 首帧绘制完成后通知后端显示窗口（窗口先隐藏建出，避免白闪）
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      invoke("overlay_ready", { label: win.label }).catch(() => {});
    });
  });
}

/** 定时休息：倒计时跑完由后端自动结束 */
function runScheduled(state: BreakState): void {
  const endAt = Date.now() + (state.plannedSec - state.elapsedSec) * 1000;
  const update = () => {
    const remainingMs = Math.max(0, endAt - Date.now());
    const progress = Math.min(1, 1 - remainingMs / 1000 / state.plannedSec);
    bar.style.width = `${(progress * 100).toFixed(1)}%`;
    if (remainingMs <= 0) clearInterval(timer);
  };
  const timer = setInterval(update, 200);
  update();
}

/** 手动休息：先倒计时跑完计划时长，再转为正计时（不自动退出），
 *  中央显示累计休息时长；分母随量级切换（1 小时 / 1 天 / 7 天） */
function runManual(state: BreakState): void {
  const startStamp = Date.now() - state.elapsedSec * 1000;
  const plannedMs = state.plannedSec * 1000;
  const update = () => {
    const elapsedMs = Date.now() - startStamp;
    if (elapsedMs < plannedMs) {
      bar.style.width = `${((elapsedMs / plannedMs) * 100).toFixed(1)}%`;
      otText.classList.add("hidden");
      return;
    }
    const elapsed = elapsedMs / 1000;
    otText.classList.remove("hidden");
    otText.textContent = fmtElapsed(elapsed);
    bar.style.width = `${(Math.min(1, elapsed / denominator(elapsed)) * 100).toFixed(1)}%`;
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

function setupDragAndResize(): void {
  // 按住移动超过阈值才拖拽，避免吃掉双击
  track.addEventListener("mousedown", (e) => {
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
      const h = Math.max(12, startH + (ev.screenY - startY));
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
