import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { capsule } from "../dom";
import { skipResizeSync, setLyricMode } from "../state";
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

  // 监听胶囊尺寸变化，动态调整窗口高度（agent 展开/收起由 set_agent_expanded 处理）

  const capsuleObserver = new ResizeObserver((entries) => {

    if (skipResizeSync) return;

    for (const entry of entries) {

      const h = entry.contentRect.height;

      // 胶囊高度 + padding-top(5px) + 底部余量(5px)

      const windowH = h + 10;

      void invoke("sync_window_height", { height: windowH });

    }

  });

  capsuleObserver.observe(capsule);

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
