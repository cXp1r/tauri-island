import { listen } from "@tauri-apps/api/event";
import { capsule, noticeArea } from "../dom";
import { currentView, isMinimized, userChosenView, setUserChosenView } from "../state";
import { getAvailableViews, setView, updateSwitcherUI, updateCapsuleSize } from "./view-switcher";

// 从 notice-queue 重新导出，保持其他模块 import 路径不变
export { showNotice } from "./notice-queue";

export function dismissOverlays() {
  noticeArea.classList.remove("active", "notice-urllist");
  noticeArea.innerHTML = "";
}

export function restoreUserView() {
  dismissOverlays();

  const views = getAvailableViews();
  if (views.includes(userChosenView)) {
    setView(userChosenView, true);
  } else {
    setUserChosenView("time");
    setView("time", true);
  }
}

export function initNoticeUrl() {
  // 展开 / 收起（鼠标悬停、后端指令等）
  listen<boolean>("set-expand", (event) => {
    if (currentView === "email") return;
    if (event.payload) {
      if (capsule.classList.contains("agent-expanded")) return;
      if (isMinimized) return;
      capsule.classList.add("expanded");
      capsule.classList.remove("lyric-collapsed");
      updateSwitcherUI();
    } else {
      if (capsule.classList.contains("agent-expanded")) return;
      capsule.classList.remove("expanded");
      updateCapsuleSize();
      dismissOverlays();
    }
  });

  // reset-view
  listen("reset-view", () => { restoreUserView(); });
}
