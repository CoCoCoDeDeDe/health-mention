import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

interface BreakSettings {
  intervalMin: number;
  durationSec: number;
  enabled: boolean;
}

interface Hotkeys {
  breakNow: string | null;
  toggleEnabled: string | null;
  endBreak: string | null;
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

interface LogRecord {
  ts: string;
  trigger: string;
  plannedSec: number;
  actualSec: number;
  result: string;
}

interface Stats {
  todayDone: number;
  streakDays: number;
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
    if (tab.dataset.tab === "logs") loadLogs().catch(console.error);
  });
}

// ---- settings form ----

const HOTKEY_INPUTS = {
  breakNow: "hk-break-now",
  toggleEnabled: "hk-toggle-enabled",
  endBreak: "hk-end-break",
  openSettings: "hk-open-settings",
} as const;

let current: Settings | null = null;

function fillForm(s: Settings): void {
  el<HTMLInputElement>("break-enabled").checked = s.break.enabled;
  el<HTMLInputElement>("interval-min").value = String(s.break.intervalMin);
  el<HTMLInputElement>("duration-sec").value = String(s.break.durationSec);
  el<HTMLSelectElement>("overlay-mode").value = s.global.overlayMode;
  el<HTMLInputElement>("autostart").checked = s.global.autostart;
  for (const [key, id] of Object.entries(HOTKEY_INPUTS)) {
    el<HTMLInputElement>(id).value = s.hotkeys[key as keyof Hotkeys] ?? "";
  }
}

function hotkeyValue(id: string): string | null {
  const v = el<HTMLInputElement>(id).value.trim();
  return v === "" ? null : v;
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
    hotkeys: {
      breakNow: hotkeyValue(HOTKEY_INPUTS.breakNow),
      toggleEnabled: hotkeyValue(HOTKEY_INPUTS.toggleEnabled),
      endBreak: hotkeyValue(HOTKEY_INPUTS.endBreak),
      openSettings: hotkeyValue(HOTKEY_INPUTS.openSettings),
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
  setDirty(false);
}

async function save(): Promise<void> {
  const status = el("save-status");
  try {
    const settings = collectForm();
    await invoke("save_settings", { settings });
    current = settings;
    setDirty(false);
    status.textContent = "已保存";
  } catch (e) {
    status.textContent = `保存失败：${e}`;
  }
  setTimeout(() => (status.textContent = ""), 4000);
}

el("save-btn").addEventListener("click", save);
load().catch((e) => {
  el("save-status").textContent = `加载失败：${e}`;
});

// ---- dirty state (unsaved changes indicator) ----

function setDirty(v: boolean): void {
  const btn = el<HTMLButtonElement>("save-btn");
  btn.textContent = v ? "保存 ●" : "保存";
  btn.classList.toggle("dirty", v);
}

el("tab-settings").addEventListener("input", () => setDirty(true));
el("tab-settings").addEventListener("change", () => setDirty(true));

// ---- hotkey capture ----

function keyToAccelerator(e: KeyboardEvent): string | null {
  // 只按修饰键时不生成，等待主键
  if (["Control", "Alt", "Shift", "Meta"].includes(e.key)) return null;
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push("Super");
  let k = e.key;
  if (k === " ") k = "Space";
  else if (k.startsWith("Arrow")) k = k.slice(5); // ArrowUp -> Up
  else if (k.length === 1) k = k.toUpperCase();
  parts.push(k);
  return parts.join("+");
}

for (const id of Object.values(HOTKEY_INPUTS)) {
  const input = el<HTMLInputElement>(id);
  input.addEventListener("keydown", (e) => {
    e.preventDefault();
    const acc = keyToAccelerator(e);
    if (acc) {
      input.value = acc;
      // 程序化赋值不触发 input 事件，手动标脏
      setDirty(true);
    }
  });
}

for (const btn of document.querySelectorAll<HTMLButtonElement>(".hk-clear")) {
  btn.addEventListener("click", () => {
    el<HTMLInputElement>(btn.dataset.hk!).value = "";
    setDirty(true);
  });
}

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

async function pollSchedule(): Promise<void> {
  try {
    const { remainingSec, status } = await invoke<ScheduleTick>("get_schedule_state");
    el("next-break").textContent =
      status === "running" && remainingSec != null
        ? `距离下次休息：${fmtRemain(remainingSec)}`
        : status === "paused"
          ? "提醒已暂停"
          : status === "disabled"
            ? "提醒已禁用"
            : "休息中…";
  } catch (e) {
    el("next-break").textContent = `状态获取失败：${e}`;
  }
}

setInterval(pollSchedule, 1000);
pollSchedule();

// ---- persist main window rect ----

const mainWin = getCurrentWindow();
setInterval(() => {
  Promise.all([mainWin.outerPosition(), mainWin.innerSize(), mainWin.scaleFactor()])
    .then(([pos, size, scale]) =>
      invoke("save_overlay_rect", {
        label: "main",
        rect: {
          x: pos.x / scale,
          y: pos.y / scale,
          w: size.width / scale,
          h: size.height / scale,
        },
      })
    )
    .catch((e) => console.error("persist main rect failed:", e));
}, 2000);

// ---- logs & stats ----

function currentDate(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

async function loadLogs(): Promise<void> {
  const date = el<HTMLInputElement>("log-date").value || null;
  const [logs, stats] = await Promise.all([
    invoke<LogRecord[]>("list_logs", { date }),
    invoke<Stats>("get_stats"),
  ]);
  el("stats-line").textContent = `今日完成 ${stats.todayDone} 次 · 连续坚持 ${stats.streakDays} 天`;

  const list = el("log-list");
  list.innerHTML = "";
  if (logs.length === 0) {
    list.innerHTML = '<p class="hint">暂无记录</p>';
    return;
  }
  for (const r of logs) {
    const d = new Date(r.ts);
    const time = `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
    const trigger = r.trigger === "manual" ? "手动" : "定时";
    const result = r.result === "done" ? "完成" : "提前结束";
    const row = document.createElement("div");
    row.className = "log-row";
    row.textContent = `${time} · ${trigger} · ${result} · ${Math.round(r.actualSec)}/${Math.round(r.plannedSec)}s`;
    list.appendChild(row);
  }
}

el<HTMLInputElement>("log-date").value = currentDate();
el("log-date").addEventListener("change", () => loadLogs().catch(console.error));

// ---- clear logs (two-step confirm) ----

const clearBtn = el<HTMLButtonElement>("clear-logs-btn");
let clearArmed = false;
clearBtn.addEventListener("click", () => {
  if (!clearArmed) {
    clearArmed = true;
    clearBtn.textContent = "确认清空？再点一次";
    setTimeout(() => {
      clearArmed = false;
      clearBtn.textContent = "清空历史";
    }, 3000);
    return;
  }
  clearArmed = false;
  clearBtn.textContent = "清空历史";
  invoke("clear_logs")
    .then(() => loadLogs())
    .catch((e) => console.error(e));
});
