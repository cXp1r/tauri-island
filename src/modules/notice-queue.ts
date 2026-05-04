import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { capsule, noticeArea } from "../dom";
import {
  setPendingUrls,
  isMinimized,
  userChosenView, setUserChosenView,
} from "../state";
import { truncateUrl } from "../utils";
import { getAvailableViews, setView } from "./view-switcher";

// ===== 通知类型 =====

export type NoticeType = "clipboard" | "email" | "generic";


const MAX_DURATION = 3000; // 每条通知最大显示 3s

// ===== 通知队列项 =====

export interface NoticeItem {
  id: string;
  uuid?: string;
  type: NoticeType;
  message: string;
  duration: number;       // ms（受 MAX_DURATION 上限约束）
  payload: unknown;       // clipboard → string[]，email → {uid,message}
  timestamp: number;      // 入队时间
}

type NoticeRenderer = {
  html: (item: NoticeItem) => string;
  bind: (item: NoticeItem) => void;
};

// ===== 图标常量 =====

const ICON_INFO = `<svg viewBox="0 0 1024 1024" xmlns="http://www.w3.org/2000/svg" width="18" height="18"><path d="M512 426.688a42.688 42.688 0 0 0-42.688 42.688v298.688a42.688 42.688 0 0 0 85.376 0V469.376A42.688 42.688 0 0 0 512 426.688zM507.776 213.376a59.776 59.776 0 1 0 0 119.552 59.776 59.776 0 0 0 0-119.552z" fill="#ffffff"/><path d="M512 0a512 512 0 1 0 0 1024 512 512 0 0 0 0-1024z m0 938.688a426.624 426.624 0 0 1-426.688-426.688c0-235.648 190.976-426.688 426.688-426.688s426.688 190.976 426.688 426.688-190.976 426.688-426.688 426.688z" fill="#ffffff"/></svg>`;

const ICON_EMAIL = `<svg viewBox="0 0 24 24" width="18" height="18" fill="none" xmlns="http://www.w3.org/2000/svg"><path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z" stroke="#fff" stroke-width="1.6"/><polyline points="22,6 12,13 2,6" stroke="#fff" stroke-width="1.6"/></svg>`;

const ICON_LINK = `<svg viewBox="0 0 24 24" width="18" height="18" fill="none" xmlns="http://www.w3.org/2000/svg"><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" stroke="#fff" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71" stroke="#fff" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>`;

// ===== 内部状态 =====

const queue: NoticeItem[] = [];
let activeItem: NoticeItem | null = null;
let displayTimer: number | null = null;
let noticeIdCounter = 0;
let urlListMode = false; // notice-area 当前是否展示 URL 列表

// ===== 公开 API =====

/** 入队一条通知 */
export function enqueueNotice(item: NoticeItem): void {
  item.duration = Math.min(item.duration, MAX_DURATION);
  item.uuid = item.uuid || createNoticeUuid();
  console.log(`[NoticeQueue] enqueue: ${describeNotice(item)} duration=${item.duration}ms queueBefore=${queue.length} active=${activeItem?.id || "none"} urlListMode=${urlListMode}`);
  queue.push(item);
  console.log(`[NoticeQueue] queued: id=${item.id} queueAfter=${queue.length}`);
  if (!activeItem && !urlListMode) {
    console.log(`[NoticeQueue] enqueue-trigger-show: id=${item.id}`);
    showNext();
  }
}

/** 兼容接口：其他模块直接入队一条通用通知 */
export function showNotice(msg: string): void {
  enqueueNotice({
    id: `generic-${++noticeIdCounter}`,
    type: "generic",
    message: msg,
    duration: MAX_DURATION,
    payload: null,
    timestamp: Date.now(),
  });
}

/** 清空队列并关闭显示 */
export function clearQueue(): void {
  console.log(`[NoticeQueue] clearQueue: active=${activeItem ? describeNotice(activeItem) : "none"} queued=${queue.length} urlListMode=${urlListMode}`);
  queue.length = 0;
  activeItem = null;
  urlListMode = false;
  clearTimer();
  finishAll();
}

// ===== 渲染 =====

function iconForType(type: NoticeType): string {
  switch (type) {
    case "clipboard": return ICON_LINK;
    case "email":     return ICON_EMAIL;
    default:          return ICON_INFO;
  }
}

/** 渲染消息模式（图标 + 文本） */
function renderMessage(item: NoticeItem): void {
  console.log(`[NoticeQueue] renderMessage: ${describeNotice(item)}`);
  noticeArea.classList.remove("notice-urllist");
  const renderer = rendererForType(item.type);
  noticeArea.innerHTML = renderer.html(item);
  renderer.bind(item);
}

/** 渲染 URL 列表模式 */
function renderUrlList(urls: string[]): void {
  console.log(`[NoticeQueue] renderUrlList: count=${urls.length} active=${activeItem ? describeNotice(activeItem) : "none"}`);
  noticeArea.classList.add("notice-urllist");
  noticeArea.innerHTML = "";
  urls.forEach((url) => {
    const el = document.createElement("div");
    el.className = "url-item";
    el.textContent = truncateUrl(url, 50);
    el.title = url;
    el.addEventListener("click", (e) => {
      e.stopPropagation();
      console.log(`[NoticeQueue] url-click: url=${url}`);
      void invoke("open_link_with_handler", { url });
      void invoke("set_interacting", { active: false });
      exitUrlListMode();
    });
    noticeArea.appendChild(el);
  });
}

function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function createNoticeUuid(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  return `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
}

function baseNoticeHtml(item: NoticeItem): string {
  const uuid = item.uuid || item.id;
  const shortUuid = uuid.replace(/-/g, "").slice(0, 8);
  return `<div class="notice-content"><div class="notice-main"><div class="icon-box">${iconForType(item.type)}</div><div class="notice-text"><div class="notice-msg">${escapeHtml(item.message)}</div><div class="notice-uuid" title="${escapeHtml(uuid)}">#${escapeHtml(shortUuid)}</div></div></div><button class="notice-dismiss" type="button">忽略</button></div>`;
}

function describeNotice(item: NoticeItem): string {
  return `id=${item.id} uuid=${item.uuid || "none"} type=${item.type} msg="${item.message}"`;
}

function bindBaseNotice(item: NoticeItem, action: () => void): void {
  const main = noticeArea.querySelector(".notice-main");
  const dismiss = noticeArea.querySelector(".notice-dismiss");

  main?.addEventListener("click", (e) => {
    e.stopPropagation();
    if (activeItem?.id !== item.id) {
      console.log(`[NoticeQueue] main-click-ignored: clicked=${item.id} active=${activeItem?.id || "none"}`);
      return;
    }
    console.log(`[NoticeQueue] main-click: ${describeNotice(item)}`);
    action();
  });

  dismiss?.addEventListener("click", (e) => {
    e.stopPropagation();
    if (activeItem?.id !== item.id) {
      console.log(`[NoticeQueue] dismiss-ignored: clicked=${item.id} active=${activeItem?.id || "none"}`);
      return;
    }
    console.log(`[NoticeQueue] dismiss: ${describeNotice(item)} queued=${queue.length}`);
    completeActiveNotice(true, "dismiss");
  });
}

function rendererForType(type: NoticeType): NoticeRenderer {
  switch (type) {
    case "clipboard":
      return {
        html: baseNoticeHtml,
        bind: (item) => bindBaseNotice(item, () => handleClipboardNotice(item)),
      };
    case "email":
      return {
        html: baseNoticeHtml,
        bind: (item) => bindBaseNotice(item, () => handleEmailNotice(item)),
      };
    default:
      return {
        html: baseNoticeHtml,
        bind: (item) => bindBaseNotice(item, () => completeActiveNotice()),
      };
  }
}

// ===== 队列推进 =====

function showNext(): void {
  clearTimer();

  if (urlListMode) {
    console.log(`[NoticeQueue] showNext-deferred: urlListMode=true queued=${queue.length} active=${activeItem?.id || "none"}`);
    return;
  }

  if (queue.length === 0) {
    console.log(`[NoticeQueue] showNext-empty: active=${activeItem?.id || "none"}`);
    activeItem = null;
    finishAll();
    return;
  }

  activeItem = queue.shift()!;
  console.log(`[NoticeQueue] showNext: ${describeNotice(activeItem)} remaining=${queue.length}`);
  renderMessage(activeItem);
  capsule.classList.add("notice-active");
  noticeArea.classList.add("active");

  if (!capsule.classList.contains("agent-expanded") && !isMinimized) {
    capsule.classList.add("expanded");
    capsule.classList.remove("lyric-collapsed");
  }
  displayTimer = window.setTimeout(() => {
    const expired = activeItem;
    if (expired) {
      console.log(`[NoticeQueue] timeout: ${describeNotice(expired)} queued=${queue.length}`);
    } else {
      console.log(`[NoticeQueue] timeout: active=none queued=${queue.length}`);
    }
    activeItem = null;
    displayTimer = null;
    showNext();
  }, activeItem.duration);
}

// ===== 点击处理（根据 type 分发） =====

function handleClick(): void {
  // URL 列表模式下点击空白区域 → 退出
  if (urlListMode) {
    console.log(`[NoticeQueue] notice-area-click: exit-url-list active=${activeItem ? describeNotice(activeItem) : "none"}`);
    exitUrlListMode();
    return;
  }

  if (!activeItem) {
    console.log("[NoticeQueue] notice-area-click: no active notice");
    return;
  }
  console.log(`[NoticeQueue] notice-area-click: ignored active=${describeNotice(activeItem)}`);
}

function handleClipboardNotice(item: NoticeItem): void {
  const urls = item.payload as string[];
  console.log(`[NoticeQueue] clipboard-action: ${describeNotice(item)} urlCount=${urls.length}`);
  clearTimer();
  if (urls.length === 1) {
    console.log(`[NoticeQueue] clipboard-open-single: id=${item.id} url=${urls[0]}`);
    void invoke("open_link_with_handler", { url: urls[0] });
    completeActiveNotice(false, "clipboard-open-single");
  } else {
    console.log(`[NoticeQueue] clipboard-open-list: id=${item.id} count=${urls.length}`);
    urlListMode = true;
    void invoke("set_interacting", { active: true });
    renderUrlList(urls);
  }
}

function handleEmailNotice(item: NoticeItem): void {
  //console.log(`[NoticeQueue] email-action: active=${activeItem ? describeNotice(activeItem) : "none"}`);
  const payload = item.payload as { uid?: string | number } | null;
  openEmailWindow(payload?.uid);
  completeActiveNotice(true, "email-open");
}

function openEmailWindow(uid?: string | number): Promise<void> {
  const normalizedUid = uid != null ? String(uid) : undefined;
  console.log("[NoticeQueue] open_email_window test:", normalizedUid || "(no uid)");
  if (normalizedUid) {
    return invoke("open_email_window", { uid: normalizedUid });
  }
  return invoke("open_email_window");
}

function completeActiveNotice(shouldClearTimer = true, reason = "complete"): void {
  console.log(`[NoticeQueue] complete: reason=${reason} active=${activeItem ? describeNotice(activeItem) : "none"} queued=${queue.length} clearTimer=${shouldClearTimer}`);
  if (shouldClearTimer) clearTimer();
  activeItem = null;
  urlListMode = false;
  void invoke("set_interacting", { active: false });
  advanceOrFinish();
}

function advanceOrFinish(): void {
  if (queue.length > 0) {
    console.log(`[NoticeQueue] advance: queued=${queue.length}`);
    showNext();
  } else {
    console.log("[NoticeQueue] advance: queue empty, finish");
    finishAll();
  }
}

function exitUrlListMode(): void {
  console.log(`[NoticeQueue] exitUrlListMode: active=${activeItem ? describeNotice(activeItem) : "none"} queued=${queue.length}`);
  urlListMode = false;
  activeItem = null;
  void invoke("set_interacting", { active: false });
  advanceOrFinish();
}

// ===== 工具 =====

function clearTimer(): void {
  if (displayTimer !== null) {
    clearTimeout(displayTimer);
    displayTimer = null;
  }
}

function finishAll(): void {
  console.log(`[NoticeQueue] finishAll: queue empty, collapsing`);
  // 先收起胶囊，再移除 overlay，避免底层视图闪烁
  capsule.classList.remove("expanded");
  capsule.classList.remove("notice-active");

  noticeArea.classList.remove("active", "notice-urllist");
  noticeArea.innerHTML = "";

  // 通知后端释放 is_notifying / is_expanded
  void invoke("dismiss_island");

  const views = getAvailableViews();
  if (views.includes(userChosenView)) {
    setView(userChosenView, true);
  } else {
    setUserChosenView("time");
    setView("time", true);
  }
}

// ===== 初始化：注册所有事件监听 =====

export function initNoticeQueue(): void {
  // 点击 notice-area
  noticeArea.addEventListener("click", (e: MouseEvent) => {
    e.stopPropagation();
    handleClick();
  });

  // 剪贴板链接
  listen<string[]>("clipboard-urls", (event) => {
    const urls = event.payload;
    if (!urls || urls.length === 0) return;
    setPendingUrls(urls);

    const shortcut = "Alt+O";
    const msg = urls.length === 1
      ? `已复制链接，按 ${shortcut} 或点击打开`
      : `检测到 ${urls.length} 个链接，点击查看`;

    enqueueNotice({
      id: `clip-${++noticeIdCounter}`,
      type: "clipboard",
      message: msg,
      duration: MAX_DURATION,
      payload: urls,
      timestamp: Date.now(),
    });
  });

  // 邮件通知
  listen<{ uid: string; message: string }>("email-notice", (event) => {
    enqueueNotice({
      id: `email-${++noticeIdCounter}`,
      type: "email",
      message: event.payload.message,
      duration: MAX_DURATION,
      payload: event.payload,
      timestamp: Date.now(),
    });
  });

  // 后端通用 show-notice（兜底）
  listen<string>("show-notice", (event) => {
    console.log(`[NoticeQueue] show-notice event received: "${event.payload}"`);
    enqueueNotice({
      id: `generic-${++noticeIdCounter}`,
      type: "generic",
      message: event.payload,
      duration: MAX_DURATION,
      payload: null,
      timestamp: Date.now(),
    });
  });

  // notice-timeout：后端遗留，已由队列管理，空监听防 warn
  listen("notice-timeout", () => {});
}
