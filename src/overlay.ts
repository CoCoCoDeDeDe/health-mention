import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";

interface BreakState {
  plannedSec: number;
  elapsedSec: number;
  mode: string;
}

const win = getCurrentWindow();
const track = document.getElementById("bar-track") as HTMLDivElement;
const bar = document.getElementById("bar") as HTMLDivElement;
const grip = document.getElementById("grip") as HTMLDivElement;

const state = await invoke<BreakState | null>("get_break_state");
const preview = state ? false : await invoke<boolean>("get_overlay_preview").catch(() => false);

if (!state && !preview) {
  // 无会话且非预览（如会话已结束窗口残留），直接关闭
  await win.close();
} else {
  const mode = state?.mode ?? "float";

  if (mode === "fullscreen") {
    document.body.classList.add("fullscreen");
  } else {
    grip.classList.remove("hidden");
    setupDragAndResize();
    persistRectLoop();
  }

  if (state) {
    // 双击 = 提前结束（拖拽已加位移阈值，不与双击冲突）
    document.body.addEventListener("dblclick", () => {
      invoke("end_break").catch(console.error);
    });
    startCountdown(state);
  } else {
    // 预览：静态半条
    bar.style.width = "50%";
  }
}

function startCountdown(state: BreakState): void {
  // 以后端状态推算本地截止时刻，前端独立倒计时渲染
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
      const w = Math.max(120, startW + (ev.screenX - startX));
      const h = Math.max(28, startH + (ev.screenY - startY));
      win.setSize(new LogicalSize(w, h)).catch(console.error);
    };
    const onUp = () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  });
}

/** 每 2 秒上报本窗口位置与宽高，后端按标签记忆（仅变化时写盘） */
function persistRectLoop(): void {
  setInterval(() => {
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
      .catch(() => {});
  }, 2000);
}
