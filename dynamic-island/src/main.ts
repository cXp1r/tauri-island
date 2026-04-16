import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { marked } from "marked";
import hljs from "highlight.js/lib/core";
import javascript from "highlight.js/lib/languages/javascript";
import typescript from "highlight.js/lib/languages/typescript";
import python from "highlight.js/lib/languages/python";
import css from "highlight.js/lib/languages/css";
import xml from "highlight.js/lib/languages/xml";
import json from "highlight.js/lib/languages/json";
import bash from "highlight.js/lib/languages/bash";
import rust from "highlight.js/lib/languages/rust";
import java from "highlight.js/lib/languages/java";
import cpp from "highlight.js/lib/languages/cpp";
import sql from "highlight.js/lib/languages/sql";
import markdown from "highlight.js/lib/languages/markdown";

hljs.registerLanguage("javascript", javascript);
hljs.registerLanguage("js", javascript);
hljs.registerLanguage("typescript", typescript);
hljs.registerLanguage("ts", typescript);
hljs.registerLanguage("python", python);
hljs.registerLanguage("py", python);
hljs.registerLanguage("css", css);
hljs.registerLanguage("html", xml);
hljs.registerLanguage("xml", xml);
hljs.registerLanguage("json", json);
hljs.registerLanguage("bash", bash);
hljs.registerLanguage("sh", bash);
hljs.registerLanguage("rust", rust);
hljs.registerLanguage("rs", rust);
hljs.registerLanguage("java", java);
hljs.registerLanguage("cpp", cpp);
hljs.registerLanguage("c", cpp);
hljs.registerLanguage("sql", sql);
hljs.registerLanguage("markdown", markdown);
hljs.registerLanguage("md", markdown);

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
const vinylDisc = document.getElementById("vinyl-disc") as HTMLDivElement;
const vinylCover = document.getElementById("vinyl-cover") as HTMLDivElement;
const progressBar = document.getElementById("progress-bar") as HTMLDivElement;
const progressFill = document.getElementById("progress-fill") as HTMLDivElement;
const progressThumb = document.getElementById("progress-thumb") as HTMLDivElement;

// 音乐展开面板
const musicPanelCoverImg = document.getElementById("music-panel-cover-img") as HTMLDivElement;
const musicPanelSong = document.getElementById("music-panel-song") as HTMLDivElement;
const musicPanelArtist = document.getElementById("music-panel-artist") as HTMLDivElement;
const mpProgressBar = document.getElementById("mp-progress-bar") as HTMLDivElement;
const mpProgressFill = document.getElementById("mp-progress-fill") as HTMLDivElement;
const mpProgressThumb = document.getElementById("mp-progress-thumb") as HTMLDivElement;
const mpTimeCurrent = document.getElementById("mp-time-current") as HTMLSpanElement;
const mpTimeTotal = document.getElementById("mp-time-total") as HTMLSpanElement;
const mpPrev = document.getElementById("mp-prev") as HTMLButtonElement;
const mpPlay = document.getElementById("mp-play") as HTMLButtonElement;
const mpNext = document.getElementById("mp-next") as HTMLButtonElement;
const mpIconPlay = mpPlay.querySelector(".mp-icon-play") as SVGElement;
const mpIconPause = mpPlay.querySelector(".mp-icon-pause") as SVGElement;
const mpVolumeBar = document.getElementById("mp-volume-bar") as HTMLDivElement;
const mpVolumeFill = document.getElementById("mp-volume-fill") as HTMLDivElement;
const mpVolumeThumb = document.getElementById("mp-volume-thumb") as HTMLDivElement;
const mpLyricText = document.getElementById("mp-lyric-text") as HTMLDivElement;

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
let currentDurationMs = 0; // 歌曲总时长
let isSeeking = false; // 是否正在拖动进度条
let isMpSeeking = false; // 面板进度条拖动
let isMpVolSeeking = false; // 面板音量条拖动
let isSeekable = true; // 当前播放器是否支持 seek
let musicClickTimer: number | null = null; // 音乐单击延时
let isExpandAnimating = false; // 展开/收起动画进行中，防止重复触发
let isMinimizeAnimating = false; // 最小化/恢复动画进行中
let currentSongTitle = "";
let currentArtistName = "";

// AI Agent 相关状态
let aiEnabled = false;
let aiGenerating = false;
let currentAssistantMessage: HTMLDivElement | null = null;
let currentAssistantRawText = "";
let currentThinkingSection: HTMLDivElement | null = null;
let thinkingStartTime = 0;
let thinkingTimer: number | null = null;

type ViewMode = "time" | "lyric" | "agent";
let currentView: ViewMode = "time";
let userChosenView: ViewMode = "time";

const viewElements: Record<ViewMode, HTMLElement> = {
  time: timeWrapper,
  lyric: lyricArea,
  agent: agentArea,
};



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

  // 如果从 lyric 展开态切走，收起（使用 skipResizeSync 避免过渡中 ResizeObserver 干扰）
  if (previous === "lyric" && mode !== "lyric" && capsule.classList.contains("music-expanded")) {
    skipResizeSync = true;
    isExpandAnimating = false;
    capsule.classList.remove("music-expanded");
    void invoke("set_music_expanded", { expanded: false, width: 380, height: 420 });
    window.setTimeout(() => { skipResizeSync = false; }, 500);
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


function updatePlayIcon() {
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

function formatTime(ms: number): string {
  const totalSec = Math.floor(ms / 1000);
  const m = Math.floor(totalSec / 60);
  const s = totalSec % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
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
  if (isMinimized || isMinimizeAnimating) return;
  isMinimized = true;
  isMinimizeAnimating = true;

  capsule.classList.add("minimizing");

  setTimeout(() => {
    capsule.classList.remove("minimizing");
    capsule.classList.add("minimized");
    document.body.classList.add("minimized");
    void invoke("set_minimized", { minimized: true });
    isMinimizeAnimating = false;
  }, 300);
}

function expandFromMinimized() {
  if (!isMinimized || isMinimizeAnimating) return;
  isMinimized = false;
  isMinimizeAnimating = true;

  void invoke("set_minimized", { minimized: false });

  document.body.classList.remove("minimized");

  capsule.classList.remove("minimized");
  capsule.classList.add("expanding");

  setTimeout(() => {
    capsule.classList.remove("expanding");
    isMinimizeAnimating = false;
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

// ===== 天气功能（后端后台线程推送）=====
async function refreshWeather(force = false) {
  if (force) {
    weatherText.textContent = "获取中...";
    void invoke("refresh_weather");
    return;
  }
  // 非强制刷新：尝试读取缓存
  try {
    const result = await invoke<WeatherResult>("get_weather");
    if (result.city) {
      weatherText.textContent = `${result.city} ${result.desc} ${result.temp}°C`;
    } else {
      weatherText.textContent = `${result.desc} ${result.temp}°C`;
    }
  } catch {
    // 缓存尚未就绪，后台线程会自动推送
    if (weatherText.textContent === "") {
      weatherText.textContent = "获取中...";
    }
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

listen<string>("agent-window-size-changed", async (event) => {
  // 更新 CSS 变量
  updateAgentCSSSize(event.payload);
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

listen<{ text: string | null; title: string; artist: string; genre?: string; position_ms?: number; duration_ms?: number; is_playing?: boolean; seekable?: boolean; nearby_lyrics?: Array<{text: string; is_current: boolean}> } | null>("lyric-update", (event) => {
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
  const { text, title, artist, position_ms, duration_ms } = event.payload;

  // 从 lyric-update 同步播放状态，避免 playback-state 事件丢失
  if (event.payload.is_playing !== undefined && event.payload.is_playing !== isPlaying) {
    isPlaying = event.payload.is_playing;
    updatePlayIcon();
  }

  // 同步 seekable 状态
  if (event.payload.seekable !== undefined) {
    updateSeekable(event.payload.seekable);
  }

  // 更新进度条（收起态 + 面板）
  if (duration_ms && duration_ms > 0 && position_ms !== undefined) {
    currentDurationMs = duration_ms;
    const pct = Math.min(100, Math.max(0, (position_ms / duration_ms) * 100));
    if (!isSeeking) {
      progressFill.style.width = `${pct}%`;
      progressThumb.style.left = `${pct}%`;
    }
    if (!isMpSeeking) {
      mpProgressFill.style.width = `${pct}%`;
      mpProgressThumb.style.left = `${pct}%`;
      mpTimeCurrent.textContent = formatTime(position_ms);
      mpTimeTotal.textContent = formatTime(duration_ms);
    }
  }

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

  // 同步歌词到展开面板（多行）— 使用固定槽位，平滑过渡
  const nearby = event.payload.nearby_lyrics;
  if (nearby && nearby.length > 0) {
    const slots = mpLyricText.children;
    // 首次从文本节点切换到槽位时，清除残留文本节点（如 "♪"）
    if (slots.length === 0 && mpLyricText.childNodes.length > 0) {
      mpLyricText.textContent = "";
    }
    // 确保槽位数量匹配
    while (slots.length < nearby.length) {
      const div = document.createElement("div");
      div.className = "mp-lyric-line";
      mpLyricText.appendChild(div);
    }
    while (slots.length > nearby.length) {
      mpLyricText.removeChild(mpLyricText.lastChild!);
    }
    // 更新每个槽位的文字和样式
    for (let i = 0; i < nearby.length; i++) {
      const el = slots[i] as HTMLElement;
      const line = nearby[i];
      if (el.textContent !== line.text) {
        // 文字变化时做微淡入
        el.style.opacity = "0";
        setTimeout(() => {
          el.textContent = line.text;
          el.style.opacity = "";
        }, 120);
      }
      el.className = line.is_current ? "mp-lyric-line mp-lyric-current" : "mp-lyric-line";
    }
  } else if (text !== null && text !== undefined) {
    // 前奏/等待歌词阶段：强制显示音乐符号，避免残留多行歌词造成“提前显示后续歌词”
    if (text === "♪") {
      mpLyricText.textContent = "♪";
    } else {
      // 如果面板已有多行歌词槽位，不用单行文本覆盖
      if (mpLyricText.children.length === 0) {
        mpLyricText.textContent = text;
      }
    }
  } else {
    mpLyricText.textContent = title;
  }

  updateSwitcherUI();
});

listen<{ title: string; artist: string; genre?: string; thumbnail?: string | null; duration_ms?: number; seekable?: boolean }>("media-changed", (event) => {
  isMusicPlaying = true;
  currentSongTitle = event.payload.title;
  currentArtistName = event.payload.artist;
  console.log(`[SMTC] genre='${event.payload.genre ?? ""}' title='${event.payload.title}' artist='${event.payload.artist}'`);
  lyricText.textContent = "♪";
  lyricMeta.textContent = `${event.payload.artist} - ${event.payload.title}`;
  mpLyricText.textContent = "♪";
  lyricMeta.style.fontSize = "";
  lyricMeta.style.color = "";

  // 同步面板信息
  musicPanelSong.textContent = event.payload.title;
  musicPanelArtist.textContent = event.payload.artist;

  // 更新封面
  if (event.payload.thumbnail) {
    vinylCover.style.backgroundImage = `url(${event.payload.thumbnail})`;
    musicPanelCoverImg.style.backgroundImage = `url(${event.payload.thumbnail})`;
  } else {
    vinylCover.style.backgroundImage = "";
    musicPanelCoverImg.style.backgroundImage = "";
  }

  // 更新时长
  if (event.payload.duration_ms) {
    currentDurationMs = event.payload.duration_ms;
    mpTimeTotal.textContent = formatTime(event.payload.duration_ms);
  } else {
    currentDurationMs = 0;
    mpTimeTotal.textContent = "0:00";
  }

  // 更新 seekable 状态
  updateSeekable(event.payload.seekable ?? true);

  // 重置进度条
  progressFill.style.width = "0%";
  progressThumb.style.left = "0%";
  mpProgressFill.style.width = "0%";
  mpProgressThumb.style.left = "0%";
  mpTimeCurrent.textContent = "0:00";

  updateSwitcherUI();
});

listen<{ title: string; artist: string }>("media-paused", () => {
  isMusicPlaying = true;
});

// 异步封面加载完成
listen<{ thumbnail: string }>("media-thumbnail", (event) => {
  if (event.payload.thumbnail) {
    vinylCover.style.backgroundImage = `url(${event.payload.thumbnail})`;
    musicPanelCoverImg.style.backgroundImage = `url(${event.payload.thumbnail})`;
  }
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
    if (!isShowingUrlList) restoreUserView();
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

// ===== 面板音量滑块 =====
// 展开面板时获取当前音量
function fetchAndUpdateVolume() {
  invoke<number>("media_get_volume").then((vol) => {
    const pct = Math.min(100, Math.max(0, vol * 100));
    mpVolumeFill.style.width = `${pct}%`;
    mpVolumeThumb.style.left = `${pct}%`;
  }).catch(() => {});
}

mpVolumeBar.addEventListener("mousedown", (e: MouseEvent) => {
  e.stopPropagation();
  isMpVolSeeking = true;
  mpVolumeBar.classList.add("seeking");
  const pct = updateMpVolumeFromMouse(e);
  void invoke("media_set_volume", { volume: pct }).catch(() => {});
});

let volThrottleTimer: number | null = null;

function updateMpVolumeFromMouse(e: MouseEvent) {
  const rect = mpVolumeBar.getBoundingClientRect();
  const pct = Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width));
  mpVolumeFill.style.width = `${pct * 100}%`;
  mpVolumeThumb.style.left = `${pct * 100}%`;
  return pct;
}

document.addEventListener("mousemove", (e: MouseEvent) => {
  if (!isMpVolSeeking) return;
  const pct = updateMpVolumeFromMouse(e);
  // 节流：拖动时每50ms更新一次音量
  if (!volThrottleTimer) {
    volThrottleTimer = window.setTimeout(() => {
      volThrottleTimer = null;
    }, 50);
    void invoke("media_set_volume", { volume: pct }).catch(() => {});
  }
});

document.addEventListener("mouseup", (e: MouseEvent) => {
  if (!isMpVolSeeking) return;
  isMpVolSeeking = false;
  mpVolumeBar.classList.remove("seeking");
  const pct = updateMpVolumeFromMouse(e);
  void invoke("media_set_volume", { volume: pct }).catch((err: unknown) => {
    console.warn("Set volume failed:", err);
  });
});

// ===== 进度条拖动（Seek）=====
function updateSeekable(seekable: boolean) {
  if (isSeekable === seekable) return;
  isSeekable = seekable;
  progressBar.classList.toggle("no-seek", !seekable);
  mpProgressBar.classList.toggle("no-seek", !seekable);
}

progressBar.addEventListener("mousedown", (e: MouseEvent) => {
  if (currentDurationMs <= 0 || !isSeekable) return;
  e.stopPropagation();
  isSeeking = true;
  progressBar.classList.add("seeking");
  updateProgressFromMouse(e);
});

function updateProgressFromMouse(e: MouseEvent) {
  const rect = progressBar.getBoundingClientRect();
  const pct = Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width));
  progressFill.style.width = `${pct * 100}%`;
  progressThumb.style.left = `${pct * 100}%`;
  return pct;
}

document.addEventListener("mousemove", (e: MouseEvent) => {
  if (!isSeeking) return;
  updateProgressFromMouse(e);
});

document.addEventListener("mouseup", (e: MouseEvent) => {
  if (!isSeeking) return;
  isSeeking = false;
  progressBar.classList.remove("seeking");
  const pct = updateProgressFromMouse(e);
  const seekMs = Math.round(pct * currentDurationMs);
  void invoke("media_seek", { positionMs: seekMs }).catch((err: unknown) => {
    console.warn("Seek failed:", err);
  });
});

// ===== 面板播放控制 =====
mpPrev.addEventListener("click", (e) => {
  e.stopPropagation();
  void invoke("media_prev");
});
mpPlay.addEventListener("click", (e) => {
  e.stopPropagation();
  void invoke("media_play_pause");
});
mpNext.addEventListener("click", (e) => {
  e.stopPropagation();
  void invoke("media_next");
});

// ===== 面板进度条拖动（Seek）=====
mpProgressBar.addEventListener("mousedown", (e: MouseEvent) => {
  if (currentDurationMs <= 0 || !isSeekable) return;
  e.stopPropagation();
  isMpSeeking = true;
  mpProgressBar.classList.add("seeking");
  updateMpProgressFromMouse(e);
});

function updateMpProgressFromMouse(e: MouseEvent) {
  const rect = mpProgressBar.getBoundingClientRect();
  const pct = Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width));
  mpProgressFill.style.width = `${pct * 100}%`;
  mpProgressThumb.style.left = `${pct * 100}%`;
  mpTimeCurrent.textContent = formatTime(pct * currentDurationMs);
  return pct;
}

document.addEventListener("mousemove", (e: MouseEvent) => {
  if (!isMpSeeking) return;
  updateMpProgressFromMouse(e);
});

document.addEventListener("mouseup", (e: MouseEvent) => {
  if (!isMpSeeking) return;
  isMpSeeking = false;
  mpProgressBar.classList.remove("seeking");
  const pct = updateMpProgressFromMouse(e);
  const seekMs = Math.round(pct * currentDurationMs);
  void invoke("media_seek", { positionMs: seekMs }).catch((err: unknown) => {
    console.warn("Seek failed:", err);
  });
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
  if (capsule.classList.contains("agent-expanded") || capsule.classList.contains("music-expanded")) return;

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

  // 音乐视图：单击展开/收起
  if (currentView === "lyric") {
    // 展开态：点击头部收起，排除交互元素
    if (capsule.classList.contains("music-expanded")) {
      if (!target.closest("#music-panel-header")) {
        return;
      }
    } else {
      // 收起态：排除播放控制
      if (target.closest(".media-btn") || target.closest(".progress-bar") || target.closest(".vol-btn")) {
        return;
      }
    }

    e.stopPropagation();

    if (musicClickTimer) {
      clearTimeout(musicClickTimer);
      musicClickTimer = null;
      return;
    }

    musicClickTimer = window.setTimeout(() => {
      musicClickTimer = null;
      if (isExpandAnimating) return;
      isExpandAnimating = true;
      const willExpand = !capsule.classList.contains("music-expanded");
      if (willExpand) {
        skipResizeSync = true;
        capsule.classList.add("music-expanded");
        musicPanelSong.textContent = currentSongTitle || "";
        musicPanelArtist.textContent = currentArtistName || "";
        fetchAndUpdateVolume();
        const bodyPad = parseFloat(getComputedStyle(document.body).paddingTop) || 0;
        void invoke("set_music_expanded", { expanded: true, width: 380, height: 420 + bodyPad + 5 });
        window.setTimeout(() => { skipResizeSync = false; isExpandAnimating = false; }, 400);
      } else {
        skipResizeSync = true;
        capsule.classList.remove("music-expanded");
        void invoke("set_music_expanded", { expanded: false, width: 380, height: 420 });
        window.setTimeout(() => { skipResizeSync = false; isExpandAnimating = false; }, 500);
      }
    }, 250);
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
      if (isExpandAnimating) return;
      isExpandAnimating = true;
      const willExpand = !capsule.classList.contains("agent-expanded");
      if (willExpand) {
        skipResizeSync = true;
        capsule.classList.add("agent-expanded");
        void invoke("set_agent_expanded", { expanded: true });
        window.setTimeout(() => { skipResizeSync = false; isExpandAnimating = false; }, 400);
      } else {
        skipResizeSync = true;
        const agentArea = document.getElementById("agent-area");
        if (agentArea) agentArea.classList.add("collapsing");
        window.setTimeout(() => {
          capsule.classList.remove("agent-expanded");
          void invoke("set_agent_expanded", { expanded: false });
          window.setTimeout(() => {
            if (agentArea) agentArea.classList.remove("collapsing");
            skipResizeSync = false;
            isExpandAnimating = false;
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
  // 取消 music 单击延时
  if (musicClickTimer) {
    clearTimeout(musicClickTimer);
    musicClickTimer = null;
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
    // 不立即重置 dragStarted，留给 click handler 检测并阻断点击
    // 安全兜底：如果 click 事件未触发（如焦点丢失），100ms 后自动重置
    window.setTimeout(() => { dragStarted = false; }, 100);
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
});

setInterval(updateTimeAndDate, 1000);
updateTimeAndDate();

// 启动时尝试读取缓存（后台线程会自动获取并推送）
void refreshWeather();

// 监听后端天气更新推送
listen<{ desc: string; temp: number; city: string }>("weather-updated", (event) => {
  const r = event.payload;
  if (r.city) {
    weatherText.textContent = `${r.city} ${r.desc} ${r.temp}°C`;
  } else {
    weatherText.textContent = `${r.desc} ${r.temp}°C`;
  }
});

listen<{ error: string }>("weather-error", () => {
  if (weatherText.textContent === "获取中...") {
    weatherText.textContent = "天气暂不可用";
  }
});

// 监听设置页天气城市变更
listen("weather-city-changed", () => {
  weatherText.textContent = "获取中...";
  // 后端已自动触发 force refresh，等待 weather-updated 事件即可
});

// 监听启动时自动检查更新结果
listen<{ has_update: boolean; latest_version: string }>("update-available", (event) => {
  if (event.payload.has_update) {
    showNotice(`发现新版本 v${event.payload.latest_version}，请前往设置更新`);
  }
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
invoke<{ lyric_mode: string; indicator_color: string; agent_window_size: string }>("get_settings").then((s) => {
  lyricMode = s.lyric_mode || "lyric";
  if (s.indicator_color) {
    applyIndicatorColor(s.indicator_color);
  }
  if (s.agent_window_size) {
    updateAgentCSSSize(s.agent_window_size);
  }
});

// 根据窗口大小档位更新 CSS 变量
function updateAgentCSSSize(size: string) {
  let w: number, h: number;
  switch (size) {
    case "small":  w = 380; h = 400; break;
    case "large":  w = 620; h = 640; break;
    default:       w = 520; h = 540; break; // medium
  }
  capsule.style.setProperty("--agent-w", `${w}px`);
  capsule.style.setProperty("--agent-h", `${h}px`);
}

// ==================== AI Agent 功能 ====================

// KaTeX 数学渲染
import katex from "katex";
import "katex/dist/katex.min.css";

function renderLatex(tex: string, displayMode: boolean): string {
  try {
    return katex.renderToString(tex, {
      displayMode,
      throwOnError: false,
      trust: true,
    });
  } catch {
    return tex;
  }
}

// 预处理数学公式：先将 LaTeX 替换为占位符，markdown 处理后再恢复
function renderMarkdown(text: string): string {
  const mathBlocks: string[] = [];
  let placeholder = (i: number) => `%%MATH_BLOCK_${i}%%`;

  // 1. 块级公式 $$...$$ 
  let processed = text.replace(/\$\$([\s\S]*?)\$\$/g, (_, tex) => {
    const idx = mathBlocks.length;
    mathBlocks.push(renderLatex(tex.trim(), true));
    return placeholder(idx);
  });

  // 2. 块级公式 \[...\]
  processed = processed.replace(/\\\[([\s\S]*?)\\\]/g, (_, tex) => {
    const idx = mathBlocks.length;
    mathBlocks.push(renderLatex(tex.trim(), true));
    return placeholder(idx);
  });

  // 3. 行内公式 \(...\)
  processed = processed.replace(/\\\(([\s\S]*?)\\\)/g, (_, tex) => {
    const idx = mathBlocks.length;
    mathBlocks.push(renderLatex(tex.trim(), false));
    return placeholder(idx);
  });

  // 4. 行内公式 $...$（避免匹配货币符号如 $5）
  processed = processed.replace(/(?<!\$)\$(?!\$)([^\n$]+?)\$(?!\$)/g, (_, tex) => {
    const idx = mathBlocks.length;
    mathBlocks.push(renderLatex(tex.trim(), false));
    return placeholder(idx);
  });

  // 5. Markdown 渲染
  try {
    let html = marked.parse(processed, { async: false }) as string;
    // 6. 恢复数学公式
    mathBlocks.forEach((rendered, i) => {
      html = html.replace(placeholder(i), rendered);
    });
    return html;
  } catch {
    return text.replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/\n/g, "<br>");
  }
}

marked.setOptions({
  gfm: true,
  breaks: true,
});

// 高亮代码块并添加复制按钮
function highlightAndAddCopyButtons(container: HTMLElement) {
  container.querySelectorAll("pre code").forEach((block) => {
    // 高亮
    try {
      hljs.highlightElement(block as HTMLElement);
    } catch { /* ignore */ }

    // 复制按钮（避免重复添加）
    const pre = block.parentElement;
    if (pre && !pre.querySelector(".code-copy-btn")) {
      const btn = document.createElement("button");
      btn.className = "code-copy-btn";
      btn.textContent = "复制";
      btn.addEventListener("click", (e) => {
        e.stopPropagation();
        const code = block.textContent || "";
        navigator.clipboard.writeText(code).then(() => {
          btn.textContent = "✓ 已复制";
          btn.classList.add("copied");
          setTimeout(() => {
            btn.textContent = "复制";
            btn.classList.remove("copied");
          }, 1500);
        });
      });
      pre.style.position = "relative";
      pre.appendChild(btn);
    }
  });
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

// 添加助手消息容器（不创建 content div，用于思考阶段）
function addAssistantContainer() {
  const messageDiv = document.createElement("div");
  messageDiv.className = "agent-message assistant";
  agentMessages.appendChild(messageDiv);
  scrollMessagesToBottom();
  return messageDiv;
}

// 确保助手消息容器中有 content div，没有则创建
function ensureAssistantContentDiv(container: HTMLDivElement): HTMLDivElement {
  let contentDiv = container.querySelector(".message-content") as HTMLDivElement | null;
  if (!contentDiv) {
    contentDiv = document.createElement("div");
    contentDiv.className = "message-content token-fade";
    contentDiv.textContent = "";
    container.appendChild(contentDiv);
  }
  return contentDiv;
}

// 停止思考计时器
function stopThinkingTimer() {
  if (thinkingTimer !== null) {
    clearInterval(thinkingTimer);
    thinkingTimer = null;
  }
}

// 添加思考区域
function addThinkingSection(parentMessage: HTMLDivElement) {
  const thinkingDiv = document.createElement("div");
  thinkingDiv.className = "thinking-section active";

  const headerDiv = document.createElement("div");
  headerDiv.className = "thinking-header";

  const labelSpan = document.createElement("span");
  labelSpan.className = "thinking-label";
  labelSpan.textContent = "思考中...";

  const timeSpan = document.createElement("span");
  timeSpan.className = "thinking-time";
  timeSpan.textContent = "0.0s";

  const toggleSpan = document.createElement("span");
  toggleSpan.className = "thinking-toggle";
  toggleSpan.textContent = "▼";

  headerDiv.appendChild(labelSpan);
  headerDiv.appendChild(timeSpan);
  headerDiv.appendChild(toggleSpan);

  const contentDiv = document.createElement("div");
  contentDiv.className = "thinking-content";
  contentDiv.textContent = "";

  thinkingDiv.appendChild(headerDiv);
  thinkingDiv.appendChild(contentDiv);
  // 插入到消息内容之前，确保思考区域在回复上方
  const messageContent = parentMessage.querySelector(".message-content");
  if (messageContent) {
    parentMessage.insertBefore(thinkingDiv, messageContent);
  } else {
    parentMessage.appendChild(thinkingDiv);
  }

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

  // 启动实时计时器
  stopThinkingTimer();
  thinkingTimer = window.setInterval(() => {
    if (thinkingStartTime > 0) {
      const elapsed = ((Date.now() - thinkingStartTime) / 1000).toFixed(1);
      timeSpan.textContent = `${elapsed}s`;
    }
  }, 100);

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

  // 立即标记生成状态，防止快速双击重复发送
  aiGenerating = true;

  agentInput.value = "";

  addUserMessage(content);

  agentSendBtn.style.display = "none";
  agentStopBtn.style.display = "flex";

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
let currentAssistantContainer: HTMLDivElement | null = null;

listen<{ text: string }>("ai-token", (event) => {
  // 跳过空 token，避免思考阶段提前创建正文气泡
  if (!event.payload.text) return;

  // 确保有容器
  if (!currentAssistantContainer) {
    currentAssistantContainer = addAssistantContainer();
  }
  // 确保有 content div（思考结束后首次创建）
  if (!currentAssistantMessage) {
    currentAssistantMessage = ensureAssistantContentDiv(currentAssistantContainer);
    currentAssistantRawText = "";
  }

  currentAssistantRawText += event.payload.text;
  currentAssistantMessage.innerHTML = renderMarkdown(currentAssistantRawText);
  highlightAndAddCopyButtons(currentAssistantMessage);
  scrollMessagesToBottom();
});

listen<{ text: string }>("ai-thinking-token", (event) => {
  if (!currentThinkingSection) {
    // 只创建容器，不创建 content div
    if (!currentAssistantContainer) {
      currentAssistantContainer = addAssistantContainer();
    }
    addThinkingSection(currentAssistantContainer);
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

    // 停止计时器，更新思考完成时间
    stopThinkingTimer();
    if (currentThinkingSection && thinkingStartTime > 0) {
      const thinkingTime = ((Date.now() - thinkingStartTime) / 1000).toFixed(1);
      const thinkingSection = currentThinkingSection.parentElement;
      if (thinkingSection) {
        thinkingSection.classList.remove("active");
        const labelSpan = thinkingSection.querySelector(".thinking-label");
        const timeSpan = thinkingSection.querySelector(".thinking-time");
        if (labelSpan) labelSpan.textContent = "思考完成";
        if (timeSpan) timeSpan.textContent = `${thinkingTime}s`;
      }
    }
  } else if (status === "completed") {
    updateAgentStatus("就绪");
    updateAgentBorderState("idle");
    stopThinkingTimer();
    agentSendBtn.style.display = "flex";
    agentStopBtn.style.display = "none";
    aiGenerating = false;
    currentAssistantMessage = null;
    currentAssistantRawText = "";
    currentThinkingSection = null;
    currentAssistantContainer = null;
    thinkingStartTime = 0;
  } else if (status === "error") {
    updateAgentStatus(error || "错误", true);
    updateAgentBorderState("error");
    stopThinkingTimer();
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

