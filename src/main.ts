import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface BreakSettings {
  intervalMin: number;
  durationSec: number;
  enabled: boolean;
}

interface Hotkeys {
  breakNow: string | null;
  togglePause: string | null;
  openSettings: string | null;
}

interface GlobalSettings {
  paused: boolean;
  autostart: boolean;
  overlayMode: string;
  theme: string;
  progressPlugin: string | null;
}

interface Settings {
  version: number;
  break: BreakSettings;
  hotkeys: Hotkeys;
  global: GlobalSettings;
}

interface ScheduleTick {
  remainingSec: number | null;
  status: "running" | "paused" | "disabled" | "inSession";
}

function el<T extends HTMLElement>(id: string): T {
  const node = document.getElementById(id);
  if (!node) throw new Error(`missing element #${id}`);
  return node as T;
}

// ---- tabs ----

for (const tab of document.querySelectorAll<HTMLButtonElement>(".tab")) {
  tab.addEventListener("click", () => {
    for (const t of document.querySelectorAll(".tab")) t.classList.remove("active");
    tab.classList.add("active");
    for (const panel of document.querySelectorAll(".tab-panel")) panel.classList.add("hidden");
    el(`tab-${tab.dataset.tab}`).classList.remove("hidden");
  });
}

// ---- settings form ----

let current: Settings | null = null;

function fillForm(s: Settings): void {
  el<HTMLInputElement>("break-enabled").checked = s.break.enabled;
  el<HTMLInputElement>("interval-min").value = String(s.break.intervalMin);
  el<HTMLInputElement>("duration-sec").value = String(s.break.durationSec);
  el<HTMLSelectElement>("overlay-mode").value = s.global.overlayMode;
  el<HTMLInputElement>("autostart").checked = s.global.autostart;
}

function collectForm(): Settings {
  if (!current) throw new Error("settings not loaded");
  return {
    ...current,
    break: {
      enabled: el<HTMLInputElement>("break-enabled").checked,
      intervalMin: Number(el<HTMLInputElement>("interval-min").value),
      durationSec: Number(el<HTMLInputElement>("duration-sec").value),
    },
    global: {
      ...current.global,
      overlayMode: el<HTMLSelectElement>("overlay-mode").value,
      autostart: el<HTMLInputElement>("autostart").checked,
    },
  };
}

async function load(): Promise<void> {
  current = await invoke<Settings>("get_settings");
  fillForm(current);
}

async function save(): Promise<void> {
  const status = el("save-status");
  try {
    const settings = collectForm();
    await invoke("save_settings", { settings });
    current = settings;
    status.textContent = "已保存";
  } catch (e) {
    status.textContent = `保存失败：${e}`;
  }
  setTimeout(() => (status.textContent = ""), 3000);
}

el("save-btn").addEventListener("click", save);
load().catch((e) => {
  el("save-status").textContent = `加载失败：${e}`;
});

// ---- next break countdown ----

function fmtRemain(sec: number): string {
  const total = Math.ceil(sec);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  if (h > 0) return `${h} 小时 ${m} 分`;
  if (m > 0) return `${m} 分 ${s} 秒`;
  return `${s} 秒`;
}

listen<ScheduleTick>("schedule://tick", (e) => {
  const { remainingSec, status } = e.payload;
  el("next-break").textContent =
    status === "running" && remainingSec != null
      ? `距离下次休息：${fmtRemain(remainingSec)}`
      : status === "paused"
        ? "提醒已暂停"
        : status === "disabled"
          ? "提醒已禁用"
          : "休息中…";
}).catch(console.error);
