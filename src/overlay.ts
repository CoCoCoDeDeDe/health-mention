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

if (!state) {
  // 无会话（如会话已结束窗口残留），直接关闭
  await win.close();
} else {
  if (state.mode === "fullscreen") {
    document.body.classList.add("fullscreen");
  } else {
    grip.classList.remove("hidden");

    // 左键按住进度条拖拽移动
    track.addEventListener("mousedown", (e) => {
      if (e.button === 0) win.startDragging().catch(console.error);
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

  // 双击 = 提前结束
  document.body.addEventListener("dblclick", () => {
    invoke("end_break").catch(console.error);
  });

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
