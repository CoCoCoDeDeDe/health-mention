import { invoke } from "@tauri-apps/api/core";

interface BreakState {
  plannedSec: number;
  elapsedSec: number;
}

const bar = document.getElementById("bar") as HTMLDivElement;
const timeText = document.getElementById("time") as HTMLSpanElement;

document.getElementById("end-btn")!.addEventListener("click", () => {
  invoke("end_break").catch(console.error);
});

const state = await invoke<BreakState | null>("get_break_state");

if (!state) {
  timeText.textContent = "没有进行中的休息";
} else {
  // 以后端状态推算本地截止时刻，前端独立倒计时渲染
  const endAt = Date.now() + (state.plannedSec - state.elapsedSec) * 1000;

  const update = () => {
    const remainingMs = Math.max(0, endAt - Date.now());
    const remainingSec = Math.ceil(remainingMs / 1000);
    const progress = Math.min(1, 1 - remainingMs / 1000 / state.plannedSec);
    bar.style.width = `${(progress * 100).toFixed(1)}%`;
    timeText.textContent = remainingSec > 0 ? `剩余 ${remainingSec} 秒` : "休息结束";
    if (remainingMs <= 0) clearInterval(timer);
  };

  const timer = setInterval(update, 200);
  update();
}
