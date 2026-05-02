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



// html 高度实时同步到后端窗口
let lastSyncedHtmlH = 0;
const RESIZE_LOG_DELAY_MS = 120;
let resizeLogTimer: number | null = null;
let resizeLogFrom = 0;
let resizeLogExtreme = 0;
let resizeLogDirection: "up" | "down" | null = null;

function syncHtmlHeight() {
  if (skipResizeSync) return;
  const h = document.documentElement.offsetHeight;
  if (h <= 0 || h === lastSyncedHtmlH) return;
  trackResizeLog(lastSyncedHtmlH, h);
  lastSyncedHtmlH = h;
  void invoke("sync_window_height", { height: h });
}

function trackResizeLog(from: number, to: number) {
  const direction = to > from ? "up" : "down";
  if (resizeLogDirection && resizeLogDirection !== direction) {
    flushResizeLog();
  }

  if (!resizeLogDirection) {
    resizeLogDirection = direction;
    resizeLogFrom = from;
    resizeLogExtreme = to;
  } else if (direction === "up") {
    resizeLogExtreme = Math.max(resizeLogExtreme, to);
  } else {
    resizeLogExtreme = Math.min(resizeLogExtreme, to);
  }

  if (resizeLogTimer !== null) {
    clearTimeout(resizeLogTimer);
  }
  resizeLogTimer = window.setTimeout(flushResizeLog, RESIZE_LOG_DELAY_MS);
}

function flushResizeLog() {
  if (!resizeLogDirection) return;
  if (resizeLogTimer !== null) {
    clearTimeout(resizeLogTimer);
    resizeLogTimer = null;
  }
  const marker = resizeLogDirection === "up" ? "↑ max" : "↓ min";
  console.log(`[ResizeObserver] html height ${marker}:`, resizeLogFrom, "→", resizeLogExtreme);
  resizeLogDirection = null;
}



export function initResizeObserver() {

  // html 高度变化 → 实时同步窗口高度（不改宽度，避免偏左回弹）
  const htmlObserver = new ResizeObserver(() => syncHtmlHeight());
  htmlObserver.observe(document.documentElement);

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
