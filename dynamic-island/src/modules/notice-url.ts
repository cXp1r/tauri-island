import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { capsule, noticeArea, noticeMsg, urlList } from "../dom";
import {
  noticeTimer, setNoticeTimer,
  pendingUrls, setPendingUrls,
  isShowingUrlList, setIsShowingUrlList,
  isMinimized,
  userChosenView, setUserChosenView,
} from "../state";
import { truncateUrl } from "../utils";
import { getAvailableViews, setView, updateSwitcherUI, updateCapsuleSize } from "./view-switcher";

export function dismissOverlays() {

  noticeArea.classList.remove("active");

  urlList.classList.remove("active");

  urlList.innerHTML = "";

}



export function restoreUserView() {

  setIsShowingUrlList(false);

  if (noticeTimer) {

    clearTimeout(noticeTimer);

    setNoticeTimer(null);

  }



  dismissOverlays();



  const views = getAvailableViews();

  if (views.includes(userChosenView)) {

    setView(userChosenView, true);

  } else {

    setUserChosenView("time");

    setView("time", true);

  }

}



export function showNotice(msg: string) {

  if (isShowingUrlList) return;



  noticeMsg.innerText = msg;

  noticeArea.classList.add("active");

  capsule.classList.add("expanded");

  capsule.classList.remove("lyric-collapsed");



  if (noticeTimer) {

    clearTimeout(noticeTimer);

  }



  setNoticeTimer(window.setTimeout(() => {

    if (!isShowingUrlList) restoreUserView();

  }, 3000));

}



function showUrlList() {

  if (noticeTimer) {

    clearTimeout(noticeTimer);

    setNoticeTimer(null);

  }



  setIsShowingUrlList(true);

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



export function initNoticeUrl() {

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



  listen<string[]>("clipboard-urls", (event) => {

    setPendingUrls(event.payload);

  });

}
