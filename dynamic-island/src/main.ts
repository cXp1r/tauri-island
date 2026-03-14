﻿import { listen } from "@tauri-apps/api/event";
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

let noticeTimer: number | null = null;
let pendingUrls: string[] = [];
let isShowingUrlList = false;
let isMusicPlaying = false;
let isPlaying = false;
let lyricMode = "lyric"; // "off" | "info" | "lyric"
let privacyPopupTimer: number | null = null;
let privacyPulseCleanupTimer: number | null = null;

// AI Agent 相关状态
let aiEnabled = false;
let aiGenerating = false;
let currentAssistantMessage: HTMLDivElement | null = null;
let currentThinkingSection: HTMLDivElement | null = null;
let thinkingStartTime = 0;

type ViewMode = "time" | "notice" | "urls" | "lyric" | "agent";
let currentView: ViewMode = "time";
let userChosenView: ViewMode = "time";

const viewElements: Record<ViewMode, HTMLElement> = {
  time: timeWrapper,
  notice: noticeArea,
  urls: urlList,
  lyric: lyricArea,
  agent: agentArea,
};

const WEATHER_REFRESH_MS = 20 * 60 * 1000;
let lastWeatherFetchAt = 0;
let weatherLoading = false;

type WttrCondition = {
  temp_C?: string;
  weatherCode?: string;
  weatherDesc?: Array<{ value?: string }>;
};

type WttrResponse = {
  current_condition?: WttrCondition[];
};

type PrivacyUsagePayload = {
  microphone: boolean;
  camera: boolean;
};

let lastPrivacyUsage: PrivacyUsagePayload = {
  microphone: false,
  camera: false,
};

const WEATHER_CODE_CN: Record<string, string> = {
  "113": "晴",
  "116": "少云",
  "119": "多云",
  "122": "阴",
  "143": "薄雾",
  "176": "小雨",
  "179": "雨夹雪",
  "182": "雨夹雪",
  "185": "冻雨",
  "200": "雷雨",
  "227": "吹雪",
  "230": "暴雪",
  "248": "雾",
  "260": "浓雾",
  "263": "零星毛毛雨",
  "266": "毛毛雨",
  "281": "冻毛毛雨",
  "284": "强冻毛毛雨",
  "293": "小雨",
  "296": "小雨",
  "299": "中雨",
  "302": "中雨",
  "305": "大雨",
  "308": "暴雨",
  "311": "小冻雨",
  "314": "冻雨",
  "317": "雨夹雪",
  "320": "雨夹雪",
  "323": "小雪",
  "326": "中雪",
  "329": "大雪",
  "332": "暴雪",
  "335": "暴雪",
  "338": "大雪",
  "350": "冰粒",
  "353": "小阵雨",
  "356": "阵雨",
  "359": "强阵雨",
  "362": "阵性雨夹雪",
  "365": "阵性雨夹雪",
  "368": "阵雪",
  "371": "强阵雪",
  "374": "冰雹",
  "377": "冰粒",
  "386": "雷阵雨",
  "389": "雷暴雨",
  "392": "雷阵雪",
  "395": "雷阵雪",
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
  syncPrivacyFocusForCurrentView();
  void syncCurrentView(mode);
  updateSwitcherUI();
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

function syncPrivacyFocusForCurrentView() {
  const popupActive = privacyIndicators.classList.contains("active");
  timeWrapper.classList.toggle("privacy-focus", popupActive && currentView === "time");
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
  syncPrivacyFocusForCurrentView();
}

function showPrivacyPopup(payload: PrivacyUsagePayload) {
  const { microphone, camera } = payload;
  if (!microphone && !camera) return;

  // AI 大屏展开时不显示隐私检测
  if (capsule.classList.contains("agent-expanded")) return;

  privacyMic.classList.toggle("active", microphone);
  privacyCamera.classList.toggle("active", camera);

  // 取消所有 content-wrapper 上的 fill:forwards 动画，否则动画填充值会覆盖 CSS 的 opacity:0
  (Object.keys(viewElements) as ViewMode[]).forEach((v) => {
    viewElements[v].getAnimations().forEach((a) => a.cancel());
  });
  // 确保当前视图仍然可见（cancel 会清除 fill 值）
  viewElements[currentView].style.display = "flex";

  syncPrivacyFocusForCurrentView();

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

async function requestWeather(): Promise<WttrResponse> {
  const controller = new AbortController();
  const timer = window.setTimeout(() => controller.abort(), 5500);

  try {
    const resp = await fetch("https://wttr.in/?format=j1", {
      cache: "no-store",
      signal: controller.signal,
    });

    if (!resp.ok) {
      throw new Error(`HTTP ${resp.status}`);
    }

    return (await resp.json()) as WttrResponse;
  } finally {
    window.clearTimeout(timer);
  }
}

function formatWeather(condition: WttrCondition): string {
  const label =
    (condition.weatherCode && WEATHER_CODE_CN[condition.weatherCode]) ||
    condition.weatherDesc?.[0]?.value ||
    "未知天气";
  const temp = condition.temp_C ?? "--";
  return `${label} ${temp}°C`;
}

async function refreshWeather(force = false) {
  const now = Date.now();
  if (weatherLoading) return;
  if (!force && now - lastWeatherFetchAt < WEATHER_REFRESH_MS) return;

  weatherLoading = true;
  if (lastWeatherFetchAt === 0 || force) {
    weatherText.textContent = "天气获取中...";
  }

  try {
    const data = await requestWeather();
    const condition = data.current_condition?.[0];
    if (!condition) {
      throw new Error("weather response empty");
    }

    weatherText.textContent = formatWeather(condition);
    lastWeatherFetchAt = Date.now();
  } catch (e) {
    if (lastWeatherFetchAt === 0) {
      weatherText.textContent = "天气不可用";
    }
    console.warn("weather fetch failed:", e);
  } finally {
    weatherLoading = false;
  }
}

function restoreUserView() {
  isShowingUrlList = false;
  if (noticeTimer) {
    clearTimeout(noticeTimer);
    noticeTimer = null;
  }

  urlList.innerHTML = "";
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
    // Agent 展开态时不响应普通展开
    if (capsule.classList.contains("agent-expanded")) return;
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
    }
  }
});

listen<string>("show-notice", (event) => {
  showNotice(event.payload);
});

listen("notice-timeout", () => {
  if (!isShowingUrlList) restoreUserView();
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
  } else if (privacyIndicators.classList.contains("active") && (next.microphone || next.camera)) {
    showPrivacyPopup(next);
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

  setView("notice", true);
  noticeMsg.innerText = msg;
  capsule.classList.add("expanded");
  capsule.classList.remove("lyric-collapsed");

  if (noticeTimer) {
    clearTimeout(noticeTimer);
  }

  noticeTimer = window.setTimeout(() => {
    if (!isShowingUrlList) restoreUserView();
  }, 3000);
}

noticeArea.addEventListener("click", (e: MouseEvent) => {
  e.stopPropagation();
  if (pendingUrls.length === 0) return;

  if (pendingUrls.length === 1) {
    void invoke("open_url", { url: pendingUrls[0] });
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
  setView("urls", true);

  urlList.innerHTML = "";
  pendingUrls.forEach((url) => {
    const item = document.createElement("div");
    item.className = "url-item";
    item.textContent = truncateUrl(url, 50);
    item.title = url;
    item.addEventListener("click", (e) => {
      e.stopPropagation();
      void invoke("open_url", { url });
      void invoke("set_interacting", { active: false });
      void invoke("dismiss_island");
    });
    urlList.appendChild(item);
  });

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
        capsule.classList.add("agent-expanded");
        void invoke("set_agent_expanded", { expanded: true });
      } else {
        // 收起：同时启动后端窗口动画和前端 CSS 过渡
        capsule.classList.remove("agent-expanded");
        void invoke("set_agent_expanded", { expanded: false });
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

showOnlyView("time");
hidePrivacyPopup();
void syncCurrentView(currentView);
invoke<{ lyric_mode: string }>("get_settings").then((s) => {
  lyricMode = s.lyric_mode || "lyric";
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
    capsule.classList.add("agent-active");
    updateAgentBorderState("idle");
    updateAgentStatus("就绪");
    updateSwitcherUI();
  }
}).catch((error) => {
  console.error("Failed to load AI settings:", error);
});

// 监听 AI 设置变更
listen("ai-settings-changed", () => {
  void invoke<{ api_url: string; model: string }>("ai_get_settings").then((settings) => {
    const wasEnabled = aiEnabled;
    aiEnabled = !!(settings.api_url && settings.model);

    if (aiEnabled) {
      agentModelName.textContent = settings.model;
      if (!wasEnabled) {
        capsule.classList.add("agent-active");
        updateAgentBorderState("idle");
        updateAgentStatus("就绪");
      }
    } else {
      capsule.classList.remove("agent-active", "agent-idle", "agent-thinking", "agent-generating", "agent-error");
    }

    updateSwitcherUI();
  });
});

