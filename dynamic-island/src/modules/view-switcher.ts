import { invoke } from "@tauri-apps/api/core";
import type { ViewMode } from "../types";
import {
  capsule,
  iconPlay, iconPause,
  mpIconPlay, mpIconPause,
  vinylDisc,
  viewSwitcher, viewDots, viewElements,
} from "../dom";
import {
  isMusicPlaying,
  lyricMode,
  aiEnabled,
  currentView, setCurrentView,
  setUserChosenView,
  isPlaying,
  setIsExpandAnimating,
  setSkipResizeSync,
} from "../state";

export function getAvailableViews(): ViewMode[] {
  const views: ViewMode[] = ["time"];
  if (isMusicPlaying && lyricMode !== "off") {
    views.push("lyric");
  }
  if (aiEnabled) {
    views.push("agent");
  }
  console.log("getAvailableViews called, aiEnabled:", aiEnabled, "views:", views);
  return views;
}

export function updateSwitcherUI() {
  const views = getAvailableViews();

  if (views.length > 1) {
    viewSwitcher.classList.add("has-views");
  } else {
    viewSwitcher.classList.remove("has-views");
  }

  viewDots.innerHTML = "";
  views.forEach((v) => {
    const dot = document.createElement("div");
    dot.className = "view-dot" + (v === currentView ? " active" : "");
    dot.title = v === "time" ? "时间视图" : "歌词视图";
    dot.addEventListener("click", (e) => {
      e.stopPropagation();
      setUserChosenView(v);
      setView(v, true);
    });
    viewDots.appendChild(dot);
  });
}

function playSwitchPulse() {
  capsule.classList.remove("switch-pulse");
  void capsule.offsetWidth;
  capsule.classList.add("switch-pulse");
  window.setTimeout(() => {
    capsule.classList.remove("switch-pulse");
  }, 360);
}

export function switchToNextView() {
  const views = getAvailableViews();
  console.log("Available views:", views, "aiEnabled:", aiEnabled);
  if (views.length < 2) return;

  const currentIndex = views.indexOf(currentView);
  const nextIndex = currentIndex >= 0 ? (currentIndex + 1) % views.length : 0;
  const nextView = views[nextIndex];

  console.log("Switching from", currentView, "to", nextView);
  playSwitchPulse();
  setUserChosenView(nextView);
  setView(nextView, true);
}

export function showOnlyView(mode: ViewMode) {
  (Object.keys(viewElements) as ViewMode[]).forEach((v) => {
    const el = viewElements[v];
    el.getAnimations().forEach((a) => a.cancel());
    el.style.display = v === mode ? "flex" : "none";
    el.style.opacity = "";
    el.style.transform = "";
  });
}

function animateViewSwitch(from: ViewMode, to: ViewMode) {
  if (from === to) {
    showOnlyView(to);
    return;
  }

  const fromEl = viewElements[from];
  const toEl = viewElements[to];

  (Object.keys(viewElements) as ViewMode[]).forEach((v) => {
    if (v !== from && v !== to) {
      const el = viewElements[v];
      el.getAnimations().forEach((a) => a.cancel());
      el.style.display = "none";
    }
  });

  toEl.getAnimations().forEach((a) => a.cancel());
  toEl.style.display = "flex";
  const inAnim = toEl.animate(
    [
      { opacity: 0, transform: "translateY(8px) scale(0.985)" },
      { opacity: 1, transform: "translateY(0) scale(1)" },
    ],
    {
      duration: 230,
      easing: "cubic-bezier(0.2, 0.8, 0.2, 1)",
      fill: "forwards",
    },
  );
  inAnim.onfinish = () => {
    if (currentView === to) {
      toEl.style.opacity = "";
      toEl.style.transform = "";
    }
  };

  if (fromEl.style.display !== "none") {
    fromEl.getAnimations().forEach((a) => a.cancel());
    const outAnim = fromEl.animate(
      [
        { opacity: 1, transform: "translateY(0) scale(1)" },
        { opacity: 0, transform: "translateY(-8px) scale(0.985)" },
      ],
      {
        duration: 160,
        easing: "cubic-bezier(0.4, 0, 1, 1)",
        fill: "forwards",
      },
    );

    outAnim.onfinish = () => {
      if (currentView === to) {
        fromEl.style.display = "none";
        fromEl.style.opacity = "";
        fromEl.style.transform = "";
      }
    };
  } else {
    fromEl.style.display = "none";
  }
}

export function setView(mode: ViewMode, animated = true) {
  const previous = currentView;
  setCurrentView(mode);

  // 如果从 agent 展开态切走，收起并恢复窗口大小
  if (previous === "agent" && mode !== "agent" && capsule.classList.contains("agent-expanded")) {
    capsule.classList.remove("agent-expanded");
    window.setTimeout(() => {
      void invoke("set_agent_expanded", { expanded: false });
    }, 380);
  }

  // 如果从 lyric 展开态切走，收起（使用 skipResizeSync 避免过渡中 ResizeObserver 干扰）
  if (previous === "lyric" && mode !== "lyric" && capsule.classList.contains("music-expanded")) {
    setSkipResizeSync(true);
    setIsExpandAnimating(false);
    capsule.classList.remove("music-expanded");
    void invoke("set_music_expanded", { expanded: false, width: 380, height: 420 });
    window.setTimeout(() => { setSkipResizeSync(false); }, 500);
  }

  if (animated) {
    animateViewSwitch(previous, mode);
  } else {
    showOnlyView(mode);
  }

  updateCapsuleSize();
  void syncCurrentView(mode);
  updateSwitcherUI();

  // 七彩边框已禁用
}

export function syncCurrentView(mode: ViewMode) {
  invoke("set_current_view", { view: mode }).catch((e) => {
    console.warn("sync current view failed:", e);
  });
}

export function updateCapsuleSize() {
  console.log("updateCapsuleSize called, currentView:", currentView, "expanded:", capsule.classList.contains("expanded"));

  // Agent 视图不使用 expanded 类，使用独立的 agent-expanded 类
  if (currentView === "agent") {
    capsule.classList.remove("expanded", "lyric-collapsed");
    // agent-expanded 由单击事件控制，这里不处理
    return;
  }

  // 其他视图的展开态
  if (capsule.classList.contains("expanded")) {
    capsule.classList.remove("lyric-collapsed", "agent-expanded", "music-expanded");
    return;
  }

  // 收起态
  capsule.classList.remove("agent-expanded", "music-expanded");

  if (currentView === "lyric" && isMusicPlaying) {
    capsule.classList.add("lyric-collapsed");
  } else {
    capsule.classList.remove("lyric-collapsed");
  }
}

export function updatePlayIcon() {
  iconPlay.style.display = isPlaying ? "none" : "block";
  iconPause.style.display = isPlaying ? "block" : "none";

  // 面板播放图标同步
  mpIconPlay.style.display = isPlaying ? "none" : "block";
  mpIconPause.style.display = isPlaying ? "block" : "none";

  if (isPlaying) {
    vinylDisc.classList.remove("paused");
  } else {
    vinylDisc.classList.add("paused");
  }
}

export function initViewSwitcher() {
  // 视图切换器无需注册事件，所有函数由其他模块按需调用
}
