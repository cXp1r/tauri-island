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

const btnPrev = document.getElementById("btn-prev") as HTMLButtonElement;
const btnPlay = document.getElementById("btn-play") as HTMLButtonElement;
const btnNext = document.getElementById("btn-next") as HTMLButtonElement;
const iconPlay = document.getElementById("icon-play") as HTMLElement;
const iconPause = document.getElementById("icon-pause") as HTMLElement;

const viewSwitcher = document.getElementById("view-switcher") as HTMLDivElement;
const viewDots = document.getElementById("view-dots") as HTMLDivElement;

let noticeTimer: number | null = null;
let pendingUrls: string[] = [];
let isShowingUrlList = false;
let isMusicPlaying = false;
let isPlaying = false;
let lyricMode = "lyric"; // "off" | "info" | "lyric"

type ViewMode = "time" | "notice" | "urls" | "lyric";
let currentView: ViewMode = "time";
let userChosenView: ViewMode = "time";

const viewElements: Record<ViewMode, HTMLElement> = {
  time: timeWrapper,
  notice: noticeArea,
  urls: urlList,
  lyric: lyricArea,
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
  if (views.length < 2) return;

  const currentIndex = views.indexOf(currentView);
  const nextIndex = currentIndex >= 0 ? (currentIndex + 1) % views.length : 0;
  const nextView = views[nextIndex];

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

  if (animated) {
    animateViewSwitch(previous, mode);
  } else {
    showOnlyView(mode);
  }

  updateCapsuleSize();
  void syncCurrentView(mode);
  updateSwitcherUI();
}

function syncCurrentView(mode: ViewMode) {
  invoke("set_current_view", { view: mode }).catch((e) => {
    console.warn("sync current view failed:", e);
  });
}

function updateCapsuleSize() {
  if (capsule.classList.contains("expanded")) {
    capsule.classList.remove("lyric-collapsed");
    return;
  }

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
    capsule.classList.add("expanded");
    capsule.classList.remove("lyric-collapsed");
    updateSwitcherUI();
  } else {
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
let lastX = 0;
let lastY = 0;

capsule.addEventListener("mousedown", (e: MouseEvent) => {
  if (e.button !== 0) return;
  const target = e.target as HTMLElement;
  if (target.closest(".url-item") || target.closest("#notice-area") || target.closest(".media-btn") || target.closest(".view-dot")) {
    return;
  }

  isDragging = true;
  lastX = e.screenX;
  lastY = e.screenY;
  void invoke("start_drag");
});

capsule.addEventListener("dblclick", (e: MouseEvent) => {
  const target = e.target as HTMLElement;
  if (target.closest(".url-item") || target.closest("#notice-area") || target.closest(".media-btn") || target.closest(".view-dot")) {
    return;
  }

  e.stopPropagation();
  switchToNextView();
});

document.addEventListener("mousemove", (e: MouseEvent) => {
  if (!isDragging) return;

  const dx = e.screenX - lastX;
  const dy = e.screenY - lastY;
  lastX = e.screenX;
  lastY = e.screenY;

  if (dx !== 0 || dy !== 0) {
    void invoke("drag_move", { dx, dy });
  }
});

document.addEventListener("mouseup", () => {
  if (!isDragging) return;
  isDragging = false;
  void invoke("end_drag");
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
void syncCurrentView(currentView);
invoke<{ lyric_mode: string }>("get_settings").then((s) => {
  lyricMode = s.lyric_mode || "lyric";
});
