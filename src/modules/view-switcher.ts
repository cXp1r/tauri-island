import { invoke } from "@tauri-apps/api/core";
import type { ViewMode } from "../types";
import {
  capsule,
  currentViewContainer,
  viewHolder,
  iconPlay, iconPause,
  mpIconPlay, mpIconPause,
  vinylDisc,
  viewSwitcher, viewDots, viewElements,
} from "../dom";
import {
  isMusicPlaying,
  lyricMode,
  aiEnabled,
  emailConfigure,
  currentView, setCurrentView,
  setUserChosenView,
  isPlaying,
  setIsExpandAnimating,
  setSkipResizeSync,
} from "../state";
import { logi, logw } from "../logger";
// ---------------------------------------------------------------------------
// 可用视图列表（search 不参与循环切换和底部 dots）
// ---------------------------------------------------------------------------

export function getAvailableViews(): ViewMode[] {
  const views: ViewMode[] = ["time"];
  if (isMusicPlaying && lyricMode !== "off") {
    views.push("lyric");
  }
  if (aiEnabled) {
    views.push("agent");
  }
  views.push("sadb");
  if (emailConfigure) {
    views.push("email");
  }
  
  return views;
}

// ---------------------------------------------------------------------------
// 底部视图切换器 UI
// ---------------------------------------------------------------------------

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
    dot.title = v === "time" ? "时间视图" : v === "lyric" ? "歌词视图" : v === "agent" ? "Agent" : v === "sadb" ? "ADB" : "邮箱";
    dot.addEventListener("click", (e) => {
      e.stopPropagation();
      setUserChosenView(v);
      setView(v, true);
    });
    viewDots.appendChild(dot);
  });
}

// ---------------------------------------------------------------------------
// 切换脉冲动画
// ---------------------------------------------------------------------------

function playSwitchPulse() {
  capsule.classList.remove("switch-pulse");
  void capsule.offsetWidth;
  capsule.classList.add("switch-pulse");
  window.setTimeout(() => {
    capsule.classList.remove("switch-pulse");
  }, 360);
}

// ---------------------------------------------------------------------------
// 循环切换到下一视图（双击触发）
// ---------------------------------------------------------------------------

export function switchToNextView() {
  const views = getAvailableViews();
  logi("ViewSwitcher", "switchToNextView views:", views, "isMusicPlaying:", isMusicPlaying, "lyricMode:", lyricMode, "aiEnabled:", aiEnabled);
  if (views.length < 2) return;

  const currentIndex = views.indexOf(currentView);
  const nextIndex = currentIndex >= 0 ? (currentIndex + 1) % views.length : 0;
  const nextView = views[nextIndex];

  playSwitchPulse();
  setUserChosenView(nextView);
  setView(nextView, true);
}

// ---------------------------------------------------------------------------
// DOM 搬运：将活跃视图移入 #current-view，其余回暗仓
// ---------------------------------------------------------------------------

function mountView(mode: ViewMode) {
  // 把当前容器里的子元素搬回暗仓
  while (currentViewContainer.firstChild) {
    viewHolder.appendChild(currentViewContainer.firstChild);
  }
  // 把目标视图元素搬入容器
  const el = viewElements[mode];
  if (el) {
    currentViewContainer.appendChild(el);
    // 从 display:none 容器移出后强制重算样式
    el.style.display = "flex";
  }
}

// ---------------------------------------------------------------------------
// 无动画切换（初始化 / 强制跳转）
// ---------------------------------------------------------------------------

export function showOnlyView(mode: ViewMode) {
  // 取消所有视图上残留的动画
  (Object.keys(viewElements) as ViewMode[]).forEach((v) => {
    const el = viewElements[v];
    el.getAnimations().forEach((a) => a.cancel());
    el.style.opacity = "";
    el.style.transform = "";
  });
  mountView(mode);
}

// ---------------------------------------------------------------------------
// 带动画切换
// ---------------------------------------------------------------------------

function animateViewSwitch(from: ViewMode, to: ViewMode) {
  if (from === to) {
    showOnlyView(to);
    return;
  }

  const fromEl = viewElements[from];
  const toEl = viewElements[to];

  // 先把旧视图淡出
  if (fromEl && fromEl.parentElement === currentViewContainer) {
    fromEl.getAnimations().forEach((a) => a.cancel());
    const outAnim = fromEl.animate(
      [
        { opacity: 1, transform: "translateY(0) scale(1)" },
        { opacity: 0, transform: "translateY(-8px) scale(0.985)" },
      ],
      { duration: 160, easing: "cubic-bezier(0.4, 0, 1, 1)", fill: "forwards" },
    );
    outAnim.onfinish = () => {
      fromEl.style.opacity = "";
      fromEl.style.transform = "";
      // 搬回暗仓
      if (fromEl.parentElement === currentViewContainer) {
        viewHolder.appendChild(fromEl);
      }
    };
  }

  // 新视图挂入容器并淡入
  if (toEl.parentElement !== currentViewContainer) {
    currentViewContainer.appendChild(toEl);
    toEl.style.display = "flex";
  }
  toEl.getAnimations().forEach((a) => a.cancel());
  const inAnim = toEl.animate(
    [
      { opacity: 0, transform: "translateY(8px) scale(0.985)" },
      { opacity: 1, transform: "translateY(0) scale(1)" },
    ],
    { duration: 230, easing: "cubic-bezier(0.2, 0.8, 0.2, 1)", fill: "forwards" },
  );
  inAnim.onfinish = () => {
    if (currentView === to) {
      toEl.style.opacity = "";
      toEl.style.transform = "";
    }
  };
}

// ---------------------------------------------------------------------------
// 胶囊 class 统一管理
// ---------------------------------------------------------------------------
let cl: string[] = ["lyric", "email"];
let cll: string[] = ["lyric-collapsed", "email"]
export function updateCapsuleSize() {
  capsule.classList.value = "";
  const cls = cll[cl.indexOf(currentView)];
  if (cls) {
    capsule.classList.add(cls);
  }
}

// ---------------------------------------------------------------------------
// 统一入口：前端模块调用
// ---------------------------------------------------------------------------

export function setView(mode: ViewMode, animated = true) {
  const previous = currentView;//快照
  setCurrentView(mode);
  // 如果从 agent 展开态切走，收起并恢复窗口大小
  if (previous === "agent" && mode !== "agent" && capsule.classList.contains("agent-expanded")) {
    capsule.classList.remove("agent-expanded");
    window.setTimeout(() => {
      void invoke("set_expanded", { expanded: false, width: 0, height: 0 });
    }, 100);
  }

  // 如果从 lyric 展开态切走，收起
  if (previous === "lyric" && mode !== "lyric" && capsule.classList.contains("music-expanded")) {
    setSkipResizeSync(true);
    setIsExpandAnimating(false);
    capsule.classList.remove("music-expanded");
    void invoke("set_expanded", { expanded: false, width: 0, height: 0 });
    window.setTimeout(() => { setSkipResizeSync(false); }, 500);
  }

  // 如果从 sadb 切走，终止镜像并清理所有 sadb 态
  if (previous === "sadb" && mode !== "sadb") {
    void invoke("sadb_stop_mirroring");
    capsule.style.width = "";
    capsule.style.height = "";
    if (capsule.classList.contains("sadb-expanded")) {
      capsule.classList.remove("sadb-expanded");
      void invoke("set_sadb_expanded", { expanded: false });
    }
    if (capsule.classList.contains("sadb-idle")) {
      capsule.classList.remove("sadb-idle");
      // 后端动画回默认尺寸并 snap 回顶部
      window.setTimeout(() => {
        void invoke("sadb_set_idle", { idle: false });
      }, 200);
    }
  }

  // 如果从搜索切走，清理搜索态 class
  if (previous === "search" && mode !== "search") {
    capsule.classList.remove("search-active", "search-expanded");
  }

  if (previous === "email" && mode !== "email" && capsule.classList.contains("email-expanded")) {
    setSkipResizeSync(true);
    capsule.classList.remove("email-expanded");
    void invoke("set_expanded", { expanded: false, width: 0, height: 0 });
    window.setTimeout(() => { setSkipResizeSync(false); }, 360);
  }

  
  if (animated) {
    animateViewSwitch(previous, mode);
  } else {
    showOnlyView(mode);
  }
  syncCurrentView(mode);
  updateCapsuleSize();
  updateSwitcherUI();
}

// ---------------------------------------------------------------------------
// 后端同步
// ---------------------------------------------------------------------------

export function syncCurrentView(mode: ViewMode) {
  return invoke("set_current_view", { view: mode }).catch((e) => {
    logw("sync current view failed:", e);
  });
}

// ---------------------------------------------------------------------------
// 播放图标同步
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// 初始化
// ---------------------------------------------------------------------------

export function initViewSwitcher() {
  // 视图切换器无需注册事件，所有函数由其他模块按需调用
}
