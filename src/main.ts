import { invoke } from "@tauri-apps/api/core";

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
