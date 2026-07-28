// 示例进度组件：emoji + 百分比文字
// 用法：把整个 sample 目录复制到
//   %APPDATA%\com.cococodedede.health-mention\plugins\progress\sample\
// 然后在设置页「进度组件」里选择它。
//
// 契约：default export { mount(el, ctx) -> { update(state), unmount?() } }
//   ctx:   { durationSec: number, mode: 'float' | 'fullscreen' }
//   state: { progress: 0..1, remainingSec, phase: 'countdown' | 'overtime', elapsedSec }
export default {
  mount(el, ctx) {
    el.style.cssText =
      "display:flex;align-items:center;justify-content:center;gap:8px;" +
      "height:100%;background:#1d2b36;color:#cfe3f0;" +
      "font:13px 'Segoe UI','Microsoft YaHei',sans-serif;";
    const dot = document.createElement("span");
    const text = document.createElement("span");
    el.append(dot, text);
    return {
      update(state) {
        dot.textContent = state.phase === "overtime" ? "🌊" : "🌱";
        text.textContent =
          state.phase === "overtime"
            ? `已休息 ${Math.floor(state.elapsedSec / 60)} 分`
            : `${Math.round(state.progress * 100)}%`;
      },
      unmount() {
        el.innerHTML = "";
      },
    };
  },
};
