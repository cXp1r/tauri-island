import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { capsule } from "../dom";
import { setLyricMode, skipResizeSync } from "../state";
import { applyIndicatorColor } from "./minimize-drag";

// 根据窗口大小档位更新 CSS 变量

export function updateAgentCSSSize(size: string) {

  let w: number, h: number;

  switch (size) {

    case "small": w = 380; h = 400; break;

    case "large": w = 620; h = 640; break;

    default: w = 520; h = 540; break; // medium

  }

  document.documentElement.style.setProperty("--agent-w", `${w}px`);

  document.documentElement.style.setProperty("--agent-h", `${h}px`);

}



// body 高度实时同步到后端窗口
let lastSyncedBodyH = 0;

function syncBodyHeight() {
  if (skipResizeSync) return;
  const h = document.body.offsetHeight;
  if (h <= 0 || h === lastSyncedBodyH) return;
  console.log("[ResizeObserver] body height changed:", lastSyncedBodyH, "→", h);
  lastSyncedBodyH = h;
  void invoke("sync_window_height", { height: h });
}



export function initResizeObserver() {

  // body 高度变化 → 实时同步窗口高度（不改宽度，避免偏左回弹）
  const bodyObserver = new ResizeObserver(() => syncBodyHeight());
  bodyObserver.observe(document.body);

  invoke<{ lyric_mode: string; indicator_color: string; agent_window_size: string }>("get_settings").then((s) => {

    setLyricMode(s.lyric_mode || "lyric");

    if (s.indicator_color) {

      applyIndicatorColor(s.indicator_color);

    }

    if (s.agent_window_size) {

      updateAgentCSSSize(s.agent_window_size);

    }

  });



  listen<string>("agent-window-size-changed", async (event) => {

    // 更新 CSS 变量

    updateAgentCSSSize(event.payload);

    // 如果当前 AI 窗口已展开，应用新的窗口大小

    if (capsule.classList.contains("agent-expanded")) {

      await invoke("set_agent_expanded", { expanded: false });

      await invoke("set_agent_expanded", { expanded: true });

    }

  });

}
