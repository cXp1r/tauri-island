import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

const capsule = document.getElementById("island-capsule") as HTMLDivElement;
const timeWrapper = document.getElementById("time-wrapper") as HTMLDivElement;
const timeText = document.getElementById("time-text") as HTMLDivElement;
const dateText = document.getElementById("date-text") as HTMLDivElement;
const weatherText = document.getElementById("weather-text") as HTMLDivElement;

const noticeArea = document.getElementById("notice-area") as HTMLDivElement;
const noticeMsg = document.getElementById("notice-msg") as HTMLDivElement;
const urlList = document.getElementById("url-list") as HTMLDivElement;
const lyricArea = document.getElementById("lyric-area") as HTMLDivElement;

const lyricText = document.getElementById("lyric-text") as HTMLDivElement;
const lyricMeta = document.getElementById("lyric-meta") as HTMLDivElement;
const musicIndicator = document.getElementById("music-indicator") as HTMLDivElement;

const agentArea = document.getElementById("agent-area") as HTMLDivElement;
const agentMessages = document.getElementById("agent-messages") as HTMLDivElement;
const agentInput = document.getElementById("agent-input") as HTMLInputElement;
const agentSendBtn = document.getElementById("agent-send-btn") as HTMLButtonElement;
const agentStopBtn = document.getElementById("agent-stop-btn") as HTMLButtonElement;
const agentModelName = document.getElementById("agent-model-name") as HTMLDivElement;
const agentStatusLabel = document.getElementById("agent-status-label") as HTMLDivElement;
const agentClearBtn = document.getElementById("agent-clear-btn") as HTMLButtonElement;
const agentConfirmDialog = document.getElementById("agent-confirm-dialog") as HTMLDivElement;
const agentConfirmCancel = document.getElementById("agent-confirm-cancel") as HTMLButtonElement;
const agentConfirmOk = document.getElementById("agent-confirm-ok") as HTMLButtonElement;

const btnPrev = document.getElementById("btn-prev") as HTMLButtonElement;
const btnPlay = document.getElementById("btn-play") as HTMLButtonElement;
const btnNext = document.getElementById("btn-next") as HTMLButtonElement;
const iconPlay = document.getElementById("icon-play") as HTMLElement;
const iconPause = document.getElementById("icon-pause") as HTMLElement;

const viewSwitcher = document.getElementById("view-switcher") as HTMLDivElement;
const viewDots = document.getElementById("view-dots") as HTMLDivElement;
const privacyIndicators = document.getElementById("privacy-indicators") as HTMLDivElement;
const privacyMic = document.getElementById("privacy-mic") as HTMLDivElement;
const privacyCamera = document.getElementById("privacy-camera") as HTMLDivElement;
const collapsedIndicator = document.getElementById("collapsed-indicator") as HTMLDivElement;

let noticeTimer: number | null = null;
let pendingUrls: string[] = [];
let isShowingUrlList = false;
let isMusicPlaying = false;
let isPlaying = false;
let lyricMode = "lyric"; // "off" | "info" | "lyric"
let privacyPopupTimer: number | null = null;
let privacyPulseCleanupTimer: number | null = null;
let isMinimized = false;

// AI Agent 相关状态
let aiEnabled = false;
let aiGenerating = false;
let currentAssistantMessage: HTMLDivElement | null = null;
let currentThinkingSection: HTMLDivElement | null = null;
let thinkingStartTime = 0;

type ViewMode = "time" | "lyric" | "agent";
let currentView: ViewMode = "time";
let userChosenView: ViewMode = "time";

const viewElements: Record<ViewMode, HTMLElement> = {
  time: timeWrapper,
  lyric: lyricArea,
  agent: agentArea,
};

const WEATHER_REFRESH_MS = 20 * 60 * 1000;
let lastWeatherFetchAt = 0;
let weatherLoading = false;

type PrivacyUsagePayload = {
  microphone: boolean;
  camera: boolean;
};

let lastPrivacyUsage: PrivacyUsagePayload = {
  microphone: false,
  camera: false,
};

type WeatherResult = {
  desc: string;
  temp: number;
  city: string;
};

function getAvailableViews(): ViewMode[] {
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

function updateSwitcherUI() {
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
      userChosenView = v;
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

function switchToNextView() {
  const views = getAvailableViews();
  console.log("Available views:", views, "aiEnabled:", aiEnabled);
  if (views.length < 2) return;

  const currentIndex = views.indexOf(currentView);
  const nextIndex = currentIndex >= 0 ? (currentIndex + 1) % views.length : 0;
  const nextView = views[nextIndex];

  console.log("Switching from", currentView, "to", nextView);
  playSwitchPulse();
  userChosenView = nextView;
  setView(nextView, true);
}

function showOnlyView(mode: ViewMode) {
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

function setView(mode: ViewMode, animated = true) {
  const previous = currentView;
  currentView = mode;

  // 如果从 agent 展开态切走，收起并恢复窗口大小
  if (previous === "agent" && mode !== "agent" && capsule.classList.contains("agent-expanded")) {
    capsule.classList.remove("agent-expanded");
    window.setTimeout(() => {
      void invoke("set_agent_expanded", { expanded: false });
    }, 380);
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

function syncCurrentView(mode: ViewMode) {
  invoke("set_current_view", { view: mode }).catch((e) => {
    console.warn("sync current view failed:", e);
  });
}

function updateCapsuleSize() {
  console.log("updateCapsuleSize called, currentView:", currentView, "expanded:", capsule.classList.contains("expanded"));

  // Agent 视图不使用 expanded 类，使用独立的 agent-expanded 类
  if (currentView === "agent") {
    capsule.classList.remove("expanded", "lyric-collapsed");
    // agent-expanded 由单击事件控制，这里不处理
    return;
  }

  // 其他视图的展开态
  if (capsule.classList.contains("expanded")) {
    capsule.classList.remove("lyric-collapsed", "agent-expanded");
    return;
  }

  // 收起态
  capsule.classList.remove("agent-expanded");

  if (currentView === "lyric" && isMusicPlaying) {
    capsule.classList.add("lyric-collapsed");
  } else {
    capsule.classList.remove("lyric-collapsed");
  }
}


function updatePlayIcon() {
  iconPlay.style.display = isPlaying ? "none" : "block";
  iconPause.style.display = isPlaying ? "block" : "none";

  if (isPlaying) {
    musicIndicator.classList.remove("paused");
  } else {
    musicIndicator.classList.add("paused");
  }
}

function hidePrivacyPopup() {
  if (privacyPopupTimer) {
    clearTimeout(privacyPopupTimer);
    privacyPopupTimer = null;
  }
  if (privacyPulseCleanupTimer) {
    clearTimeout(privacyPulseCleanupTimer);
    privacyPulseCleanupTimer = null;
  }
  capsule.classList.remove("privacy-active", "privacy-pulse");
  privacyIndicators.classList.remove("active", "pulse");
  privacyMic.classList.remove("active");
  privacyCamera.classList.remove("active");
}

// ===== 收起/展开功能 =====
function applyIndicatorColor(color: string) {
  collapsedIndicator.style.background = `linear-gradient(90deg, ${color}dd, ${color}, ${color}dd)`;
  collapsedIndicator.style.boxShadow = `0 0 8px ${color}80`;
}

function minimizeIsland() {
  if (isMinimized) return; // 已经收起了
  isMinimized = true;

  // 添加缩小动画类
  capsule.classList.add("minimizing");

  // 等待动画完成（与 CSS transform 动画时长 300ms 同步）
  setTimeout(() => {
    capsule.classList.remove("minimizing");
    capsule.classList.add("minimized");
    document.body.classList.add("minimized");
    // 通知后端缩小窗口
    void invoke("set_minimized", { minimized: true });
  }, 300);
}

function expandFromMinimized() {
  if (!isMinimized) return; // 已经展开了
  isMinimized = false;

  // 先通知后端恢复窗口尺寸
  void invoke("set_minimized", { minimized: false });

  // 隐藏绿条
  document.body.classList.remove("minimized");

  // 准备展开动画
  capsule.classList.remove("minimized");
  capsule.classList.add("expanding");

  // 动画完成后移除动画类（与 CSS transform 动画时长 300ms 同步）
  setTimeout(() => {
    capsule.classList.remove("expanding");
  }, 300);
}

// ===== 右键菜单 =====
function showContextMenu() {
  // 使用后端显示系统菜单
  void invoke("show_context_menu");
}

// 监听菜单动作
listen<string>("context-menu-action", (event) => {
  const action = event.payload;
  if (action === "minimize") {
    minimizeIsland();
  } else if (action === "settings") {
    // 延迟执行，确保菜单完全关闭后再打开设置窗口
    setTimeout(() => {
      void invoke("open_settings");
    }, 100);
  }
});

function showPrivacyPopup(payload: PrivacyUsagePayload) {
  const { microphone, camera } = payload;
  if (!microphone && !camera) return;

  // AI 大屏展开时不显示隐私检测
  if (capsule.classList.contains("agent-expanded")) return;

  privacyMic.classList.toggle("active", microphone);
  privacyCamera.classList.toggle("active", camera);

  capsule.classList.add("privacy-active");
  capsule.classList.remove("privacy-pulse");
  void capsule.offsetWidth;
  capsule.classList.add("privacy-pulse");
  if (privacyPulseCleanupTimer) {
    clearTimeout(privacyPulseCleanupTimer);
  }
  privacyPulseCleanupTimer = window.setTimeout(() => {
    capsule.classList.remove("privacy-pulse");
    privacyPulseCleanupTimer = null;
  }, 460);

  privacyIndicators.classList.remove("pulse");
  void privacyIndicators.offsetWidth;
  privacyIndicators.classList.add("active", "pulse");

  if (privacyPopupTimer) {
    clearTimeout(privacyPopupTimer);
  }
  privacyPopupTimer = window.setTimeout(() => {
    hidePrivacyPopup();
  }, 3000);
}

function formatDateLabel(now: Date): string {
  const weekdays = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"];
  const mm = `${now.getMonth() + 1}`;
  const dd = `${now.getDate()}`;
  return `${weekdays[now.getDay()]} ${mm}/${dd}`;
}

function updateTimeAndDate() {
  const now = new Date();
  timeText.innerText = now.toLocaleTimeString("zh-CN", { hour12: false });
  dateText.innerText = formatDateLabel(now);
}

// ===== 天气功能（后端统一处理） =====
async function refreshWeather(force = false) {
  const now = Date.now();
  if (weatherLoading) return;
  if (!force && now - lastWeatherFetchAt < WEATHER_REFRESH_MS) return;

  weatherLoading = true;
  if (lastWeatherFetchAt === 0 || force) {
    weatherText.textContent = "获取中...";
  }

  try {
    const result = await invoke<WeatherResult>("get_weather");
    // 格式：城市 天气 温度 或 天气 温度
    if (result.city) {
      weatherText.textContent = `${result.city} ${result.desc} ${result.temp}°C`;
    } else {
      weatherText.textContent = `${result.desc} ${result.temp}°C`;
    }
    lastWeatherFetchAt = Date.now();
  } catch (e) {
    if (lastWeatherFetchAt === 0) {
      weatherText.textContent = "天气暂不可用";
    }
    console.warn("[Weather] 刷新失败:", e);
  } finally {
    weatherLoading = false;
  }
}

function dismissOverlays() {
  noticeArea.classList.remove("active");
  urlList.classList.remove("active");
  urlList.innerHTML = "";
}

function restoreUserView() {
  isShowingUrlList = false;
  if (noticeTimer) {
    clearTimeout(noticeTimer);
    noticeTimer = null;
  }

  dismissOverlays();

  const views = getAvailableViews();
  if (views.includes(userChosenView)) {
    setView(userChosenView, true);
  } else {
    userChosenView = "time";
    setView("time", true);
  }
}

listen<boolean>("set-expand", (event) => {
  if (event.payload) {
    // Agent 展开态或最小化状态时不响应普通展开
    if (capsule.classList.contains("agent-expanded")) return;
    if (isMinimized) return;
    capsule.classList.add("expanded");
    capsule.classList.remove("lyric-collapsed");
    updateSwitcherUI();
  } else {
    // Agent 展开态时不响应普通收起
    if (capsule.classList.contains("agent-expanded")) return;
    capsule.classList.remove("expanded");
    updateCapsuleSize();
    if (isShowingUrlList) {
      restoreUserView();
    } else {
      dismissOverlays();
    }
  }
});

listen<string>("show-notice", (event) => {
  showNotice(event.payload);
});

listen("notice-timeout", () => {
  if (!isShowingUrlList) dismissOverlays();
});

listen("reset-view", () => {
  restoreUserView();
});

listen<PrivacyUsagePayload>("privacy-usage", (event) => {
  const next = event.payload;
  const micStarted = next.microphone && !lastPrivacyUsage.microphone;
  const camStarted = next.camera && !lastPrivacyUsage.camera;

  if (micStarted || camStarted) {
    showPrivacyPopup(next);
  } else if (!next.microphone && !next.camera && (lastPrivacyUsage.microphone || lastPrivacyUsage.camera)) {
    // 麦克风和摄像头都停止使用，主动收起隐私弹窗
    hidePrivacyPopup();
  }

  lastPrivacyUsage = next;
});

listen<string[]>("clipboard-urls", (event) => {
  pendingUrls = event.payload;
});

listen<string>("lyric-mode-changed", (event) => {
  lyricMode = event.payload;
  if (lyricMode === "off" && currentView === "lyric") {
    userChosenView = "time";
    setView("time", true);
  }
  updateSwitcherUI();
});

listen<string>("indicator-color-changed", (event) => {
  applyIndicatorColor(event.payload);
});

listen<string>("agent-window-size-changed", async () => {
  // 如果当前 AI 窗口已展开，应用新的窗口大小
  if (capsule.classList.contains("agent-expanded")) {
    await invoke("set_agent_expanded", { expanded: false });
    await invoke("set_agent_expanded", { expanded: true });
  }
});

listen<boolean>("playback-state", (event) => {
  isPlaying = event.payload;
  updatePlayIcon();
});

listen<{ text: string | null; title: string; artist: string } | null>("lyric-update", (event) => {
  if (event.payload === null) {
    const wasPlaying = isMusicPlaying;
    isMusicPlaying = false;
    isPlaying = false;
    updatePlayIcon();

    if (wasPlaying && currentView === "lyric") {
      userChosenView = "time";
      setView("time", true);
    }

    updateSwitcherUI();
    return;
  }

  const wasPlaying = isMusicPlaying;
  isMusicPlaying = true;
  const { text, title, artist } = event.payload;

  if (lyricMode === "info" || text === null) {
    lyricText.textContent = "";
    lyricMeta.textContent = title;
    lyricMeta.style.fontSize = "13px";
    lyricMeta.style.color = "rgba(255,255,255,0.85)";
  } else {
    lyricMeta.style.fontSize = "";
    lyricMeta.style.color = "";
    lyricMeta.textContent = `${artist} - ${title}`;
    if (lyricText.textContent !== text) {
      lyricText.classList.add("fade");
      window.setTimeout(() => {
        lyricText.textContent = text;
        lyricText.classList.remove("fade");
      }, 140);
    }
  }

  if (!wasPlaying && lyricMode !== "off" && userChosenView === "time") {
    userChosenView = "lyric";
    setView("lyric", true);
  }

  updateSwitcherUI();
});

listen<{ title: string; artist: string }>("media-changed", (event) => {
  isMusicPlaying = true;
  lyricText.textContent = "♪";
  lyricMeta.textContent = `${event.payload.artist} - ${event.payload.title}`;
  lyricMeta.style.fontSize = "";
  lyricMeta.style.color = "";
  updateSwitcherUI();
});

listen<{ title: string; artist: string }>("media-paused", () => {
  isMusicPlaying = true;
});

function showNotice(msg: string) {
  if (isShowingUrlList) return;

  noticeMsg.innerText = msg;
  noticeArea.classList.add("active");
  capsule.classList.add("expanded");
  capsule.classList.remove("lyric-collapsed");

  if (noticeTimer) {
    clearTimeout(noticeTimer);
  }

  noticeTimer = window.setTimeout(() => {
    if (!isShowingUrlList) dismissOverlays();
  }, 3000);
}

noticeArea.addEventListener("click", (e: MouseEvent) => {
  e.stopPropagation();
  if (pendingUrls.length === 0) return;

  if (pendingUrls.length === 1) {
    void invoke("open_link_with_handler", { url: pendingUrls[0] });
    void invoke("dismiss_island");
  } else {
    showUrlList();
  }
});

function showUrlList() {
  if (noticeTimer) {
    clearTimeout(noticeTimer);
    noticeTimer = null;
  }

  isShowingUrlList = true;
  void invoke("set_interacting", { active: true });

  // 隐藏通知覆盖层，显示 URL 列表覆盖层
  noticeArea.classList.remove("active");

  urlList.innerHTML = "";
  pendingUrls.forEach((url) => {
    const item = document.createElement("div");
    item.className = "url-item";
    item.textContent = truncateUrl(url, 50);
    item.title = url;
    item.addEventListener("click", (e) => {
      e.stopPropagation();
      void invoke("open_link_with_handler", { url });
      void invoke("set_interacting", { active: false });
      void invoke("dismiss_island");
    });
    urlList.appendChild(item);
  });

  urlList.classList.add("active");
  capsule.classList.add("expanded");
  capsule.classList.remove("lyric-collapsed");
}

function truncateUrl(url: string, max: number): string {
  if (url.length <= max) return url;
  return `${url.substring(0, max - 1)}…`;
}

btnPrev.addEventListener("click", (e) => {
  e.stopPropagation();
  void invoke("media_prev");
});

btnPlay.addEventListener("click", (e) => {
  e.stopPropagation();
  void invoke("media_play_pause");
});

btnNext.addEventListener("click", (e) => {
  e.stopPropagation();
  void invoke("media_next");
});

let isDragging = false;
let dragStarted = false;
let lastX = 0;
let lastY = 0;
let mouseDownX = 0;
let mouseDownY = 0;
const DRAG_THRESHOLD = 5; // 像素，超过此距离才算拖动

capsule.addEventListener("mousedown", (e: MouseEvent) => {
  // 右键不触发拖动
  if (e.button !== 0) return;
  const target = e.target as HTMLElement;
  if (target.closest(".url-item") || target.closest("#notice-area") || target.closest(".media-btn") || target.closest(".view-dot")) {
    return;
  }
  // Agent 展开态下，排除输入框和按钮，但允许拖动状态栏和消息区域
  if (currentView === "agent" && capsule.classList.contains("agent-expanded")) {
    if (target.closest("#agent-input") || target.closest("#agent-send-btn") || target.closest("#agent-stop-btn") || target.closest("#agent-clear-btn")) {
      return;
    }
  }

  isDragging = true;
  dragStarted = false;
  lastX = e.screenX;
  lastY = e.screenY;
  mouseDownX = e.screenX;
  mouseDownY = e.screenY;
});

// 右键菜单功能
capsule.addEventListener("contextmenu", (e: MouseEvent) => {
  e.preventDefault();

  // Agent 展开态时不显示菜单
  if (capsule.classList.contains("agent-expanded")) return;

  // 隐私弹窗显示时不显示菜单
  if (capsule.classList.contains("privacy-active")) return;

  // 显示系统菜单
  showContextMenu();
});

// 绿条点击展开
collapsedIndicator.addEventListener("click", (e: MouseEvent) => {
  e.stopPropagation();
  expandFromMinimized();
});

let agentClickTimer: number | null = null;

capsule.addEventListener("click", (e: MouseEvent) => {
  const target = e.target as HTMLElement;

  // 如果刚刚发生了拖动，不触发点击
  if (dragStarted) {
    dragStarted = false;
    return;
  }

  // Agent 视图特殊处理：单击展开/收起，但需要等待排除双击
  if (currentView === "agent") {
    // 展开态：只有点击状态栏才收起，但排除清空按钮
    if (capsule.classList.contains("agent-expanded")) {
      if (!target.closest("#agent-status-bar") || target.closest("#agent-clear-btn")) {
        return;
      }
    } else {
      // 收起态：排除交互元素，其他区域点击展开
      if (target.closest("#agent-input") || target.closest("#agent-send-btn") || target.closest("#agent-stop-btn") || target.closest("#agent-clear-btn") || target.closest(".thinking-section") || target.closest("#agent-messages") || target.closest("#agent-confirm-dialog")) {
        return;
      }
    }

    e.stopPropagation();

    // 延迟执行，等待可能的双击
    if (agentClickTimer) {
      clearTimeout(agentClickTimer);
      agentClickTimer = null;
      return; // 双击的第二次 click，忽略
    }

    agentClickTimer = window.setTimeout(() => {
      agentClickTimer = null;
      const willExpand = !capsule.classList.contains("agent-expanded");
      if (willExpand) {
        // 展开：同时启动后端窗口动画和前端 CSS 过渡
        skipResizeSync = true;
        capsule.classList.add("agent-expanded");
        void invoke("set_agent_expanded", { expanded: true });
        window.setTimeout(() => { skipResizeSync = false; }, 400);
      } else {
        // 收起：先淡出内容，再缩小尺寸，避免文字抖动
        skipResizeSync = true;
        const agentArea = document.getElementById("agent-area");
        if (agentArea) agentArea.classList.add("collapsing");
        window.setTimeout(() => {
          capsule.classList.remove("agent-expanded");
          void invoke("set_agent_expanded", { expanded: false });
          window.setTimeout(() => {
            if (agentArea) agentArea.classList.remove("collapsing");
            skipResizeSync = false;
          }, 500);
        }, 100);
      }
    }, 250);
    return;
  }
});

capsule.addEventListener("dblclick", (e: MouseEvent) => {
  const target = e.target as HTMLElement;
  if (target.closest(".url-item") || target.closest("#notice-area") || target.closest(".media-btn") || target.closest(".view-dot") || target.closest("#agent-input") || target.closest("#agent-send-btn") || target.closest("#agent-stop-btn") || target.closest("#agent-clear-btn")) {
    return;
  }

  // 取消 agent 单击延时
  if (agentClickTimer) {
    clearTimeout(agentClickTimer);
    agentClickTimer = null;
  }

  e.stopPropagation();
  switchToNextView();
});

document.addEventListener("mousemove", (e: MouseEvent) => {
  if (!isDragging) return;

  const dx = e.screenX - lastX;
  const dy = e.screenY - lastY;

  // 检查是否超过拖动阈值
  if (!dragStarted) {
    const totalDx = Math.abs(e.screenX - mouseDownX);
    const totalDy = Math.abs(e.screenY - mouseDownY);
    if (totalDx < DRAG_THRESHOLD && totalDy < DRAG_THRESHOLD) return;
    dragStarted = true;
    void invoke("start_drag");
  }

  lastX = e.screenX;
  lastY = e.screenY;

  if (dx !== 0 || dy !== 0) {
    void invoke("drag_move", { dx, dy });
  }
});

document.addEventListener("mouseup", () => {
  if (!isDragging) return;
  isDragging = false;
  if (dragStarted) {
    void invoke("end_drag");
  }
});

// 点击天气文本刷新
weatherText.style.cursor = "pointer";
weatherText.title = "点击刷新天气";
weatherText.addEventListener("click", (e) => {
  e.stopPropagation();
  void refreshWeather(true);
});

timeWrapper.addEventListener("mouseenter", () => {
  updateTimeAndDate();
  void refreshWeather();
});

setInterval(updateTimeAndDate, 1000);
updateTimeAndDate();

setInterval(() => {
  void refreshWeather();
}, WEATHER_REFRESH_MS);
void refreshWeather(true);

// 监听设置页天气城市变更 → 立即刷新
listen("weather-city-changed", () => {
  lastWeatherFetchAt = 0;
  void refreshWeather(true);
});

showOnlyView("time");
hidePrivacyPopup();
void syncCurrentView(currentView);

// 监听胶囊尺寸变化，动态调整窗口高度（agent 展开/收起由 set_agent_expanded 处理）
let skipResizeSync = false;
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
invoke<{ lyric_mode: string; indicator_color: string }>("get_settings").then((s) => {
  lyricMode = s.lyric_mode || "lyric";
  if (s.indicator_color) {
    applyIndicatorColor(s.indicator_color);
  }
});

// ==================== AI Agent 功能 ====================

// 简易 Markdown 渲染
function renderMarkdown(text: string): string {
  let html = text;

  // 代码块
  html = html.replace(/```([\s\S]*?)```/g, '<pre><code>$1</code></pre>');

  // 行内代码
  html = html.replace(/`([^`]+)`/g, '<code>$1</code>');

  // 加粗
  html = html.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');

  // 无序列表
  html = html.replace(/^\s*[-*]\s+(.+)$/gm, '<li>$1</li>');
  html = html.replace(/(<li>.*<\/li>)/s, '<ul>$1</ul>');

  // 有序列表
  html = html.replace(/^\s*\d+\.\s+(.+)$/gm, '<li>$1</li>');

  // 换行
  html = html.replace(/\n/g, '<br>');

  return html;
}

// 滚动消息到底部
function scrollMessagesToBottom() {
  agentMessages.scrollTop = agentMessages.scrollHeight;
}

// 添加用户消息
function addUserMessage(content: string) {
  const messageDiv = document.createElement("div");
  messageDiv.className = "agent-message user";

  const contentDiv = document.createElement("div");
  contentDiv.className = "message-content";
  contentDiv.textContent = content;

  messageDiv.appendChild(contentDiv);
  agentMessages.appendChild(messageDiv);
  scrollMessagesToBottom();
}

// 添加助手消息
function addAssistantMessage() {
  const messageDiv = document.createElement("div");
  messageDiv.className = "agent-message assistant";

  const contentDiv = document.createElement("div");
  contentDiv.className = "message-content";
  contentDiv.textContent = "";

  messageDiv.appendChild(contentDiv);
  agentMessages.appendChild(messageDiv);
  currentAssistantMessage = contentDiv;
  scrollMessagesToBottom();

  return messageDiv;
}

// 添加思考区域
function addThinkingSection(parentMessage: HTMLDivElement) {
  const thinkingDiv = document.createElement("div");
  thinkingDiv.className = "thinking-section";

  const headerDiv = document.createElement("div");
  headerDiv.className = "thinking-header";
  headerDiv.innerHTML = '<span>思考中...</span><span class="thinking-toggle">▼</span>';

  const contentDiv = document.createElement("div");
  contentDiv.className = "thinking-content";
  contentDiv.textContent = "";

  thinkingDiv.appendChild(headerDiv);
  thinkingDiv.appendChild(contentDiv);
  parentMessage.appendChild(thinkingDiv);

  // 点击展开/折叠
  thinkingDiv.addEventListener("click", () => {
    thinkingDiv.classList.toggle("expanded");
    const toggle = thinkingDiv.querySelector(".thinking-toggle");
    if (toggle) {
      toggle.textContent = thinkingDiv.classList.contains("expanded") ? "▲" : "▼";
    }
  });

  currentThinkingSection = contentDiv;
  thinkingStartTime = Date.now();

  return contentDiv;
}

// 更新七彩流光状态
function updateAgentBorderState(state: "idle" | "thinking" | "generating" | "error") {
  capsule.classList.remove("agent-idle", "agent-thinking", "agent-generating", "agent-error");
  if (state !== "idle") {
    capsule.classList.add(`agent-${state}`);
  }
}

// 更新状态标签
function updateAgentStatus(status: string, isError = false) {
  agentStatusLabel.textContent = status;
  agentStatusLabel.className = "agent-status-label";
  if (isError) {
    agentStatusLabel.classList.add("error");
  } else if (status === "思考中...") {
    agentStatusLabel.classList.add("thinking");
  } else if (status === "生成中...") {
    agentStatusLabel.classList.add("generating");
  }
}

// 发送消息
async function sendMessage() {
  const content = agentInput.value.trim();
  if (!content || aiGenerating) return;

  // 清空输入框
  agentInput.value = "";

  // 添加用户消息
  addUserMessage(content);

  // 显示停止按钮
  agentSendBtn.style.display = "none";
  agentStopBtn.style.display = "flex";

  aiGenerating = true;

  try {
    await invoke("ai_send_message", { content });
  } catch (error) {
    console.error("发送消息失败:", error);
    // 在消息区域显示错误
    const errDiv = document.createElement("div");
    errDiv.className = "agent-message assistant";
    const errContent = document.createElement("div");
    errContent.className = "message-content";
    errContent.style.color = "#ff6b6b";
    errContent.textContent = `错误: ${error}`;
    errDiv.appendChild(errContent);
    agentMessages.appendChild(errDiv);
    scrollMessagesToBottom();

    updateAgentStatus("发送失败", true);
    updateAgentBorderState("error");
    agentSendBtn.style.display = "flex";
    agentStopBtn.style.display = "none";
    aiGenerating = false;
  }
}

// 停止生成
async function stopGeneration() {
  await invoke("ai_stop_generation");
  agentSendBtn.style.display = "flex";
  agentStopBtn.style.display = "none";
  aiGenerating = false;
  updateAgentStatus("已停止");
  updateAgentBorderState("idle");
}

// 清空历史
function showClearConfirm() {
  agentConfirmDialog.classList.add("visible");
  agentConfirmDialog.style.display = "flex";
}

function hideClearConfirm() {
  agentConfirmDialog.classList.remove("visible");
  agentConfirmDialog.style.display = "none";
}

async function clearHistory() {
  await invoke("ai_clear_history");
  agentMessages.innerHTML = "";
  currentAssistantMessage = null;
  currentThinkingSection = null;
  hideClearConfirm();
}

agentConfirmCancel.addEventListener("click", (e) => {
  e.stopPropagation();
  hideClearConfirm();
});

agentConfirmOk.addEventListener("click", (e) => {
  e.stopPropagation();
  void clearHistory();
});

// 监听 AI 事件
listen<{ text: string }>("ai-token", (event) => {
  if (!currentAssistantMessage) {
    const messageDiv = addAssistantMessage();
    currentAssistantMessage = messageDiv.querySelector(".message-content") as HTMLDivElement;
  }

  const currentText = currentAssistantMessage.textContent || "";
  currentAssistantMessage.innerHTML = renderMarkdown(currentText + event.payload.text);
  scrollMessagesToBottom();
});

listen<{ text: string }>("ai-thinking-token", (event) => {
  if (!currentThinkingSection) {
    const messageDiv = agentMessages.lastElementChild as HTMLDivElement;
    if (!messageDiv || !messageDiv.classList.contains("assistant")) {
      addAssistantMessage();
    }
    const parentMessage = agentMessages.lastElementChild as HTMLDivElement;
    addThinkingSection(parentMessage);
  }

  if (currentThinkingSection) {
    currentThinkingSection.textContent += event.payload.text;
  }
});

listen<{ status: string; error?: string }>("ai-status", (event) => {
  const { status, error } = event.payload;

  if (status === "thinking") {
    updateAgentStatus("思考中...");
    updateAgentBorderState("thinking");
  } else if (status === "generating") {
    updateAgentStatus("生成中...");
    updateAgentBorderState("generating");

    // 更新思考时间
    if (currentThinkingSection && thinkingStartTime > 0) {
      const thinkingTime = ((Date.now() - thinkingStartTime) / 1000).toFixed(1);
      const thinkingHeader = currentThinkingSection.parentElement?.querySelector(".thinking-header span");
      if (thinkingHeader) {
        thinkingHeader.textContent = `思考完成 (${thinkingTime}s)`;
      }
    }
  } else if (status === "completed") {
    updateAgentStatus("就绪");
    updateAgentBorderState("idle");
    agentSendBtn.style.display = "flex";
    agentStopBtn.style.display = "none";
    aiGenerating = false;
    currentAssistantMessage = null;
    currentThinkingSection = null;
    thinkingStartTime = 0;
  } else if (status === "error") {
    updateAgentStatus(error || "错误", true);
    updateAgentBorderState("error");
    agentSendBtn.style.display = "flex";
    agentStopBtn.style.display = "none";
    aiGenerating = false;

    // 在消息区域显示错误
    if (error) {
      const errDiv = document.createElement("div");
      errDiv.className = "agent-message assistant";
      const errContent = document.createElement("div");
      errContent.className = "message-content";
      errContent.style.color = "#ff6b6b";
      errContent.style.fontSize = "12px";
      errContent.textContent = `⚠ ${error}`;
      errDiv.appendChild(errContent);
      agentMessages.appendChild(errDiv);
      scrollMessagesToBottom();
    }

    currentAssistantMessage = null;
    currentThinkingSection = null;
  }
});

// 输入框事件
agentInput.addEventListener("keydown", (e) => {
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    void sendMessage();
  }
});

agentSendBtn.addEventListener("click", () => {
  void sendMessage();
});

agentStopBtn.addEventListener("click", () => {
  void stopGeneration();
});

agentClearBtn.addEventListener("click", () => {
  showClearConfirm();
});

// 初始化 AI 配置
invoke<{ api_url: string; model: string }>("ai_get_settings").then((settings) => {
  console.log("AI settings loaded:", settings);
  aiEnabled = !!(settings.api_url && settings.model);
  console.log("AI enabled:", aiEnabled);
  if (aiEnabled) {
    agentModelName.textContent = settings.model;
    updateAgentBorderState("idle");
    updateAgentStatus("就绪");
    updateSwitcherUI();
  }
}).catch((error) => {
  console.error("加载 AI 设置失败:", error);
});

// 监听 AI 设置变更
listen("ai-settings-changed", () => {
  void invoke<{ api_url: string; model: string }>("ai_get_settings").then((settings) => {
    const wasEnabled = aiEnabled;
    aiEnabled = !!(settings.api_url && settings.model);

    if (aiEnabled) {
      agentModelName.textContent = settings.model;
      if (!wasEnabled) {
        updateAgentBorderState("idle");
        updateAgentStatus("就绪");
      }
    } else {
      capsule.classList.remove("agent-active", "agent-idle", "agent-thinking", "agent-generating", "agent-error");
    }

    updateSwitcherUI();
  });
});

