import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  emailClearCacheBtn,
  emailContent,
  emailCount,
  emailListItems,
  emailRefreshBtn,
} from "../dom";
import { setEmailConfigure } from "../state";
import { logd, logi } from "../logger";
type EmailMeta = {
  uid: string;
  from: string;
  subject: string;
  date: string;
  cached: boolean;
};

let initialized = false;
let loading = false;
let activeUid: string | null = null;
let allUids: number[] = [];
let cachedMetas: EmailMeta[] = [];
const metaCache = new Map<string, EmailMeta>();

const TAG: string = "Email"

function debugEmail(message: string, data?: unknown) {
  if (data === undefined) {
    logd(TAG, "[EmailView]", message);
  } else {
    logd(TAG, "[EmailView]", message, data);
  }
}

function esc(s: string) {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

function sanitizeMailHtml(html: string) {
  const doc = new DOMParser().parseFromString(html, "text/html");
  doc.querySelectorAll("script, iframe, object, embed, link[rel='import']").forEach((el) => el.remove());
  doc.querySelectorAll<HTMLElement>("*").forEach((el) => {
    for (const attr of Array.from(el.attributes)) {
      const name = attr.name.toLowerCase();
      const value = attr.value.trim().toLowerCase();
      if (name.startsWith("on")) el.removeAttribute(attr.name);
      if ((name === "href" || name === "src") && value.startsWith("javascript:")) el.removeAttribute(attr.name);
    }
  });
  return doc.body.innerHTML || html;
}

function setButtonsDisabled(disabled: boolean) {
  emailRefreshBtn.disabled = disabled;
  emailClearCacheBtn.disabled = disabled;
}

function renderList() {
  emailListItems.innerHTML = "";
  const metas = cachedMetas.length > 0 ? cachedMetas : Array.from(metaCache.values());
  if (metas.length === 0 && allUids.length === 0) {
    emailListItems.innerHTML = '<div class="email-empty-state">暂无邮件</div>';
    emailCount.textContent = "";
    return;
  }

  const ordered = metas.slice().sort((a, b) => Number(b.uid) - Number(a.uid));
  emailCount.textContent = `${Math.max(allUids.length, ordered.length)} 封`;
  for (const meta of ordered) {
    const item = document.createElement("div");
    item.className = "email-item" + (meta.uid === activeUid ? " active" : "");
    item.dataset.uid = meta.uid;
    item.innerHTML = `
      <div class="email-item-from">${esc(meta.from || "(未知发件人)")}</div>
      <div class="email-item-subject">${esc(meta.subject || "(无主题)")}</div>
      <div class="email-item-date">${esc(meta.date || "")}</div>
    `;
    item.addEventListener("click", () => { void selectMail(meta); });
    emailListItems.appendChild(item);
  }
}

function renderLoadingList() {
  emailListItems.innerHTML = `
    <div class="email-item placeholder"><div></div><div></div><div></div></div>
    <div class="email-item placeholder"><div></div><div></div><div></div></div>
    <div class="email-item placeholder"><div></div><div></div><div></div></div>
  `;
}

async function loadMailboxList() {
  debugEmail("loadMailboxList:start");
  cachedMetas = await invoke<EmailMeta[]>("fetch_emails");
  debugEmail("loadMailboxList:cached metas", cachedMetas.length);
  for (const meta of cachedMetas) metaCache.set(String(meta.uid), meta);
  if (cachedMetas.length > 0) {
    allUids = cachedMetas.map((m) => Number(m.uid));
    renderList();
    debugEmail("loadMailboxList:render cached", allUids.length);
    return;
  }

  allUids = await invoke<number[]>("fetch_email_uid_list");
  debugEmail("loadMailboxList:uid list", allUids.length);
  if (allUids.length === 0) {
    renderList();
    return;
  }

  const metas = await invoke<EmailMeta[]>("fetch_email_metas_by_uids", { uids: allUids.slice(0, 20) });
  debugEmail("loadMailboxList:fetched metas", metas.length);
  cachedMetas = metas;
  for (const meta of metas) metaCache.set(String(meta.uid), meta);
  renderList();
}

async function ensureFirstSelected() {
  debugEmail("ensureFirstSelected:start", { activeUid, allUids: allUids.length, cachedMetas: cachedMetas.length });
  if (activeUid) return;
  let firstMeta = cachedMetas[0] || metaCache.get(String(allUids[0]));
  if (!firstMeta && allUids.length > 0) {
    debugEmail("ensureFirstSelected:fetch first meta", allUids[0]);
    const metas = await invoke<EmailMeta[]>("fetch_email_metas_by_uids", { uids: [allUids[0]] });
    firstMeta = metas[0];
    if (firstMeta) {
      metaCache.set(String(firstMeta.uid), firstMeta);
      cachedMetas = [firstMeta, ...cachedMetas.filter((m) => m.uid !== firstMeta.uid)];
      renderList();
    }
  }
  if (firstMeta) {
    debugEmail("ensureFirstSelected:select first", firstMeta.uid);
    await selectMail(firstMeta);
  }
}

async function selectMail(meta: EmailMeta) {
  debugEmail("selectMail:start", meta.uid);
  activeUid = String(meta.uid);
  renderList();
  emailContent.innerHTML = `
    <div class="email-content-header">
      <div class="email-content-subject">${esc(meta.subject || "(无主题)")}</div>
      <div class="email-content-meta">${esc(meta.from || "")} · ${esc(meta.date || "")}</div>
    </div>
    <div class="email-content-body"><div class="email-loading-body">加载邮件内容中...</div></div>
  `;

  try {
    await invoke<boolean>("fetch_email_body_by_uid", { uid: Number(meta.uid) });
    if (activeUid !== String(meta.uid)) return;
    const html = await invoke<string>("read_email_body_by_uid", { uid: Number(meta.uid) });
    debugEmail("selectMail:body loaded", { uid: meta.uid, bytes: html.length });
    if (activeUid !== String(meta.uid)) return;
    const body = emailContent.querySelector(".email-content-body") as HTMLDivElement | null;
    if (body) body.innerHTML = sanitizeMailHtml(html);
  } catch (e) {
    debugEmail("selectMail:failed", e);
    if (activeUid !== String(meta.uid)) return;
    const body = emailContent.querySelector(".email-content-body") as HTMLDivElement | null;
    if (body) body.innerHTML = '<div class="email-empty-state">邮件内容加载失败</div>';
  }
}

async function refreshMailbox() {
  debugEmail("refreshMailbox:start");
  setButtonsDisabled(true);
  const oldText = emailRefreshBtn.textContent;
  emailRefreshBtn.textContent = "刷新中";
  try {
    activeUid = null;
    cachedMetas = await invoke<EmailMeta[]>("refresh_emails");
    debugEmail("refreshMailbox:done", cachedMetas.length);
    metaCache.clear();
    for (const meta of cachedMetas) metaCache.set(String(meta.uid), meta);
    allUids = cachedMetas.map((m) => Number(m.uid));
    renderList();
    await ensureFirstSelected();
  } finally {
    emailRefreshBtn.textContent = oldText;
    setButtonsDisabled(false);
  }
}

async function clearCache() {
  debugEmail("clearCache:start");
  setButtonsDisabled(true);
  const oldText = emailClearCacheBtn.textContent;
  emailClearCacheBtn.textContent = "清理中";
  try {
    await invoke("clear_email_cache");
    debugEmail("clearCache:done");
    activeUid = null;
    allUids = [];
    cachedMetas = [];
    metaCache.clear();
    emailContent.innerHTML = '<div class="email-empty-state">选择一封邮件查看详情</div>';
    renderList();
  } finally {
    emailClearCacheBtn.textContent = oldText;
    setButtonsDisabled(false);
  }
}

export async function showEmbeddedEmailView() {
  debugEmail("showEmbeddedEmailView:start", { initialized, loading });
  if (loading) return;
  loading = true;
  try {
    if (!initialized) {
      renderLoadingList();
      await loadMailboxList();
      initialized = true;
    }
    await ensureFirstSelected();
  } finally {
    loading = false;
    debugEmail("showEmbeddedEmailView:done", { initialized, activeUid });
  }
}

export function initEmailView() {
  invoke<boolean>("is_email_configured").then(res => {
    logi("Email",`is_configured: ${res}`);
    setEmailConfigure(res);
  })
  
  debugEmail("initEmailView");
  emailRefreshBtn.addEventListener("click", () => { void refreshMailbox(); });
  emailClearCacheBtn.addEventListener("click", () => { void clearCache(); });
  emailContent.addEventListener("click", (event) => {
    const target = event.target as HTMLElement | null;
    const link = target?.closest?.("a[href]") as HTMLAnchorElement | null;
    if (!link) return;
    const url = new URL(link.getAttribute("href") || "", window.location.href).toString();
    if (!url.startsWith("http://") && !url.startsWith("https://")) return;
    event.preventDefault();
    event.stopPropagation();
    void invoke("open_url", { url });
  }, true);
  void listen("email-updated", async () => {
    debugEmail("event:email-updated");
    initialized = false;
    await showEmbeddedEmailView();
  });
  void listen<boolean>("email-configured", (event) => {
    logi(TAG, event.payload);
    setEmailConfigure(event.payload);
  });
}
