import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

const capsule = document.getElementById("island-capsule")!;
const timeText = document.getElementById("time-text")!;
const timeWrapper = document.getElementById("time-wrapper")!;
const noticeArea = document.getElementById("notice-area")!;
const noticeMsg = document.getElementById("notice-msg")!;
const urlList = document.getElementById("url-list")!;
const lyricArea = document.getElementById("lyric-area")!;
const lyricText = document.getElementById("lyric-text")!;
const lyricMeta = document.getElementById("lyric-meta")!;
const musicIndicator = document.getElementById("music-indicator")!;
const btnPrev = document.getElementById("btn-prev")!;
const btnPlay = document.getElementById("btn-play")!;
const btnNext = document.getElementById("btn-next")!;
const iconPlay = document.getElementById("icon-play")!;
const iconPause = document.getElementById("icon-pause")!;
const viewSwitcher = document.getElementById("view-switcher")!;
const viewDots = document.getElementById("view-dots")!;

let noticeTimer: number | null = null;
let pendingUrls: string[] = [];
let isShowingUrlList = false;
let isMusicPlaying = false;
let isPlaying = false;
let lyricMode = "lyric"; // "off" | "info" | "lyric"

// ====== 瑙嗗浘绠＄悊绯荤粺 ======
// 鍙敤瑙嗗浘鍒楄〃锛堝姩鎬佸彉鍖栵級锛岀敤鎴锋墜鍔ㄩ€夋嫨鐨勮鍥句細琚寔涔呬繚鎸?
type ViewMode = "time" | "notice" | "urls" | "lyric";
let currentView: ViewMode = "time";
let userChosenView: ViewMode = "time"; // 鐢ㄦ埛鎵嬪姩閫夋嫨鐨勮鍥撅紝鎸佷箙淇濇寔

// 鑾峰彇褰撳墠鍙垏鎹㈢殑瑙嗗浘鍒楄〃锛堜笉鍚复鏃惰鍥惧 notice/urls锛?
function getAvailableViews(): ViewMode[] {
  const views: ViewMode[] = ["time"];
  if (isMusicPlaying && lyricMode !== "off") {
    views.push("lyric");
  }
  return views;
}

// 鏇存柊鍒囨崲鍣?UI锛堝簳閮ㄧ偣鐘舵寚绀哄櫒锛?
function updateSwitcherUI() {
  const views = getAvailableViews();

  // 澶氫釜瑙嗗浘鏃舵坊鍔?has-views class锛圕SS 鎺у埗灞曞紑鏃舵墠鏄剧ず锛?
  if (views.length > 1) {
    viewSwitcher.classList.add("has-views");
  } else {
    viewSwitcher.classList.remove("has-views");
  }

  // 鏇存柊鎸囩ず鐐癸紙鍙偣鍑诲垏鎹級
  viewDots.innerHTML = "";
  views.forEach((v) => {
    const dot = document.createElement("div");
    dot.className = "view-dot" + (v === currentView ? " active" : "");
    dot.title = v === "time" ? "鏃堕棿瑙嗗浘" : "姝岃瘝瑙嗗浘";
    dot.addEventListener("click", (e) => {
      e.stopPropagation();
      userChosenView = v;
      setView(v);
    });
    viewDots.appendChild(dot);
  });
}

function switchToNextView() {
  const views = getAvailableViews();
  if (views.length < 2) return;
  const currentIndex = views.indexOf(currentView);
  const nextIndex = currentIndex >= 0 ? (currentIndex + 1) % views.length : 0;
  const nextView = views[nextIndex];
  userChosenView = nextView;
  setView(nextView);
}

function setView(mode: ViewMode) {
  currentView = mode;
  timeWrapper.style.display = mode === "time" ? "flex" : "none";
  noticeArea.style.display = mode === "notice" ? "flex" : "none";
  urlList.style.display = mode === "urls" ? "flex" : "none";
  lyricArea.style.display = mode === "lyric" ? "flex" : "none";
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

// --- 鏃堕棿 ---
function updateTime() {
  timeText.innerText = new Date().toLocaleTimeString("zh-CN", { hour12: false });
}
setInterval(updateTime, 1000);
updateTime();

// --- 鎭㈠鍒扮敤鎴烽€夋嫨鐨勮鍥?---
function restoreUserView() {
  isShowingUrlList = false;
  if (noticeTimer) { clearTimeout(noticeTimer); noticeTimer = null; }
  urlList.innerHTML = "";
  // 妫€鏌ョ敤鎴烽€夋嫨鐨勮鍥炬槸鍚︿粛鐒跺彲鐢?
  const views = getAvailableViews();
  if (views.includes(userChosenView)) {
    setView(userChosenView);
  } else {
    // 鐢ㄦ埛閫夋嫨鐨勮鍥句笉鍙敤浜嗭紙姣斿闊充箰鍋滀簡锛夛紝鍥炲埌 time
    userChosenView = "time";
    setView("time");
  }
}

// --- 灞曞紑/鏀剁缉 ---
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

// --- 閫氱煡 ---
listen<string>("show-notice", (event) => {
  showNotice(event.payload);
});

listen("notice-timeout", () => {
  if (!isShowingUrlList) restoreUserView();
});

listen("reset-view", () => { restoreUserView(); });

// --- 鍓创鏉块摼鎺?---
listen<string[]>("clipboard-urls", (event) => {
  pendingUrls = event.payload;
});

// --- 姝岃瘝妯″紡鍙樻洿 ---
listen<string>("lyric-mode-changed", (event) => {
  lyricMode = event.payload;
  if (lyricMode === "off" && currentView === "lyric") {
    userChosenView = "time";
    setView("time");
  }
  updateSwitcherUI();
});

// --- 鎾斁鐘舵€?---
listen<boolean>("playback-state", (event) => {
  isPlaying = event.payload;
  updatePlayIcon();
});

// --- 姝岃瘝鏇存柊 ---
listen<{ text: string | null; title: string; artist: string } | null>("lyric-update", (event) => {
  if (event.payload === null) {
    const wasPlaying = isMusicPlaying;
    isMusicPlaying = false;
    isPlaying = false;
    updatePlayIcon();
    // 闊充箰鍋滀簡锛屽鏋滃綋鍓嶅湪姝岃瘝瑙嗗浘锛屽垏鍥炴椂闂?
    if (wasPlaying && currentView === "lyric") {
      userChosenView = "time";
      setView("time");
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
      setTimeout(() => {
        lyricText.textContent = text;
        lyricText.classList.remove("fade");
      }, 140);
    }
  }

  // 棣栨妫€娴嬪埌闊充箰锛氬鏋滅敤鎴疯繕娌℃墜鍔ㄩ€夎繃锛岃嚜鍔ㄥ垏鍒版瓕璇?
  if (!wasPlaying && lyricMode !== "off" && userChosenView === "time") {
    userChosenView = "lyric";
    setView("lyric");
  }
  updateSwitcherUI();
});

// --- 姝屾洸鍒囨崲 ---
listen<{ title: string; artist: string }>("media-changed", (event) => {
  isMusicPlaying = true;
  lyricText.textContent = "♪";
  lyricMeta.textContent = `${event.payload.artist} - ${event.payload.title}`;
  lyricMeta.style.fontSize = "";
  lyricMeta.style.color = "";
  updateSwitcherUI();
});

// --- 鏆傚仠 ---
listen<{ title: string; artist: string }>("media-paused", (_event) => {
  isMusicPlaying = true;
});

function showNotice(msg: string) {
  if (isShowingUrlList) return;
  setView("notice");
  noticeMsg.innerText = msg;
  capsule.classList.add("expanded");
  capsule.classList.remove("lyric-collapsed");
  if (noticeTimer) clearTimeout(noticeTimer);
  noticeTimer = window.setTimeout(() => {
    if (!isShowingUrlList) restoreUserView();
  }, 3000);
}

// 鐐瑰嚮閫氱煡鍖哄煙
noticeArea.addEventListener("click", (e: MouseEvent) => {
  e.stopPropagation();
  if (pendingUrls.length === 0) return;
  if (pendingUrls.length === 1) {
    invoke("open_url", { url: pendingUrls[0] });
    invoke("dismiss_island");
  } else {
    showUrlList();
  }
});

function showUrlList() {
  if (noticeTimer) { clearTimeout(noticeTimer); noticeTimer = null; }
  isShowingUrlList = true;
  invoke("set_interacting", { active: true });
  setView("urls");
  urlList.innerHTML = "";
  pendingUrls.forEach((url) => {
    const item = document.createElement("div");
    item.className = "url-item";
    item.textContent = truncateUrl(url, 50);
    item.title = url;
    item.addEventListener("click", (e) => {
      e.stopPropagation();
      invoke("open_url", { url });
      invoke("set_interacting", { active: false });
      invoke("dismiss_island");
    });
    urlList.appendChild(item);
  });
  capsule.classList.add("expanded");
  capsule.classList.remove("lyric-collapsed");
}

function truncateUrl(url: string, max: number): string {
  if (url.length <= max) return url;
  return url.substring(0, max - 1) + "…";
}

// --- 濯掍綋鎺у埗鎸夐挳 ---
btnPrev.addEventListener("click", (e) => {
  e.stopPropagation();
  invoke("media_prev");
});
btnPlay.addEventListener("click", (e) => {
  e.stopPropagation();
  invoke("media_play_pause");
});
btnNext.addEventListener("click", (e) => {
  e.stopPropagation();
  invoke("media_next");
});

// --- 鎷栧姩 ---
let isDragging = false;
let lastX = 0;
let lastY = 0;

capsule.addEventListener("mousedown", (e: MouseEvent) => {
  if (e.button !== 0) return;
  const target = e.target as HTMLElement;
  if (target.closest(".url-item") || target.closest("#notice-area") || target.closest(".media-btn") || target.closest(".view-dot")) return;
  isDragging = true;
  lastX = e.screenX;
  lastY = e.screenY;
  invoke("start_drag");
});

capsule.addEventListener("dblclick", (e: MouseEvent) => {
  const target = e.target as HTMLElement;
  if (target.closest(".url-item") || target.closest("#notice-area") || target.closest(".media-btn") || target.closest(".view-dot")) return;
  e.stopPropagation();
  switchToNextView();
});

document.addEventListener("mousemove", (e: MouseEvent) => {
  if (!isDragging) return;
  const dx = e.screenX - lastX;
  const dy = e.screenY - lastY;
  lastX = e.screenX;
  lastY = e.screenY;
  if (dx !== 0 || dy !== 0) invoke("drag_move", { dx, dy });
});

document.addEventListener("mouseup", () => {
  if (!isDragging) return;
  isDragging = false;
  invoke("end_drag");
});

// 鍔犺浇璁剧疆涓殑姝岃瘝妯″紡
void syncCurrentView(currentView);
invoke<{ lyric_mode: string }>("get_settings").then((s) => {
  lyricMode = s.lyric_mode || "lyric";
});

