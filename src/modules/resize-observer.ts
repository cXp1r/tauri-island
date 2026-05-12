import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { capsule, collapsedIndicator } from "../dom";
import { setLyricMode } from "../state";
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


export function initResizeObserver() {
  const el = document.getElementById('island-capsule');
  let timer: number | null = null;
  const syncCapsuleRect = () => {
    const minimized = document.body.classList.contains("minimized");
    const target = minimized ? collapsedIndicator : el;
    const width = target?.offsetWidth || 0;
    const height = target?.offsetHeight || 0;
    void invoke('set_capsule_rect', { height, width });
  };
  const bodyObserver = new ResizeObserver(() => {
    if (timer !== null) {
      clearTimeout(timer);
    }
    syncCapsuleRect();
    timer = window.setTimeout(() => {
      
    }, 1);
  });
  if (el) {
    bodyObserver.observe(el);
  }
  bodyObserver.observe(collapsedIndicator);


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
