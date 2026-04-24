import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { capsule, searchInput, searchResults } from "../dom";
import { currentView } from "../state";
import { setView } from "./view-switcher";

// ===== Types =====

interface SearchResult {
  id: string;
  title: string;
  desc: string;
  icon: string;
  action: string;
}

// ===== State =====

let activeIndex = -1;
let results: SearchResult[] = [];
let debounceTimer: number | null = null;
const DEBOUNCE_MS = 400;
const PAGE_SIZE = 10;
let previousView: "time" | "lyric" | "agent" = "time";

// ===== Window height sync =====

const BODY_PAD = 5; // body padding-top

function syncSearchWindowHeight() {
  requestAnimationFrame(() => {
    let h: number;
    if (capsule.classList.contains("search-expanded")) {
      // 使用 CSS 变量目标高度，不等 transition 结束
      const raw = getComputedStyle(document.documentElement).getPropertyValue("--search-expanded-h");
      h = parseFloat(raw) || capsule.offsetHeight;
    } else {
      h = capsule.offsetHeight;
    }
    void invoke("sync_window_height", { height: h + BODY_PAD + 2 });
  });
}

// ===== Render =====

function renderResults(items: SearchResult[]) {
  results = items;
  activeIndex = items.length > 0 ? 0 : -1;
  searchResults.innerHTML = "";

  if (items.length === 0) {
    capsule.classList.remove("search-expanded");
    capsule.classList.add("search-active");
    syncSearchWindowHeight();
    return;
  }

  capsule.classList.remove("search-active");
  capsule.classList.add("search-expanded");

  items.forEach((item, i) => {
    const el = document.createElement("div");
    el.className = "search-result-item" + (i === 0 ? " active" : "");
    el.innerHTML = `
      <div class="search-result-icon">${item.icon || "📄"}</div>
      <div class="search-result-text">
        <div class="search-result-title">${escapeHtml(item.title)}</div>
        ${item.desc ? `<div class="search-result-desc">${escapeHtml(item.desc)}</div>` : ""}
      </div>
    `;
    el.addEventListener("click", (e) => {
      e.stopPropagation();
      selectResult(i);
    });
    searchResults.appendChild(el);
  });
  syncSearchWindowHeight();
}

/** 显示错误提示 */
function renderError(msg: string) {
  results = [];
  activeIndex = -1;
  searchResults.innerHTML = "";

  capsule.classList.remove("search-active");
  capsule.classList.add("search-expanded");

  const el = document.createElement("div");
  el.className = "search-error-hint";
  el.innerHTML = `<span>${escapeHtml(msg)}</span>`;
  searchResults.appendChild(el);
  syncSearchWindowHeight();
}

function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

// ===== Select =====

function selectResult(index: number) {
  if (index < 0 || index >= results.length) return;
  const item = results[index];
  void invoke("search_execute", { id: item.id, action: item.action });
  dismissSearch();
}

function updateActiveHighlight() {
  const items = searchResults.querySelectorAll(".search-result-item");
  items.forEach((el, i) => {
    el.classList.toggle("active", i === activeIndex);
  });
  const activeEl = items[activeIndex] as HTMLElement | undefined;
  activeEl?.scrollIntoView({ block: "nearest" });
}

// ===== Search request (debounce) =====

function doSearch(query: string) {
  if (debounceTimer !== null) {
    clearTimeout(debounceTimer);
  }
  debounceTimer = window.setTimeout(async () => {
    debounceTimer = null;
    if (!query.trim()) {
      renderResults([]);
      return;
    }
    try {
      const res = await invoke<SearchResult[]>("search_query", {
        query,
        offset: 0,
        count: PAGE_SIZE,
      });
      if (currentView === "search") {
        renderResults(res);
      }
    } catch (err: any) {
      const errStr = String(err);
      console.error("[Search] search_query failed:", errStr, err);
      renderError(errStr);
    }
  }, DEBOUNCE_MS);
}

// ===== Activate / Dismiss =====

export function activateSearch() {
  // Remember where we came from so we can go back
  if (currentView !== "search") {
    previousView = currentView as "time" | "lyric" | "agent";
  }
  // Clean other expand classes
  capsule.classList.remove("expanded", "lyric-collapsed", "agent-expanded", "music-expanded", "search-expanded");
  capsule.classList.add("search-active");
  setView("search", false);
  searchInput.value = "";
  searchResults.innerHTML = "";
  searchInput.focus();
  // 延迟再次 focus，防止后端 set_focus 与 webview input focus 竞争
  setTimeout(() => searchInput.focus(), 50);
}

export function dismissSearch() {
  searchInput.value = "";
  searchInput.blur();
  searchResults.innerHTML = "";
  results = [];
  activeIndex = -1;
  capsule.classList.remove("search-active", "search-expanded");
  setView(previousView, true);
  syncSearchWindowHeight();
}

// ===== Init =====

export function initSearch() {
  // Input listener
  searchInput.addEventListener("input", () => {
    doSearch(searchInput.value);
  });

  // 拦截 Alt+Space，防止浏览器默认行为干扰搜索激活
  document.addEventListener("keydown", (e) => {
    if (e.altKey && e.code === "Space") {
      e.preventDefault();
    }
  }, true);

  // Global Esc (capture phase to beat browser blur)
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && currentView === "search") {
      e.preventDefault();
      e.stopImmediatePropagation();
      dismissSearch();
    }
  }, true);

  // Keyboard navigation inside search input
  searchInput.addEventListener("keydown", (e) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      if (results.length > 0) {
        activeIndex = (activeIndex + 1) % results.length;
        updateActiveHighlight();
      }
      return;
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      if (results.length > 0) {
        activeIndex = (activeIndex - 1 + results.length) % results.length;
        updateActiveHighlight();
      }
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      if (activeIndex >= 0) {
        selectResult(activeIndex);
      }
      return;
    }
  });

  // 失去前台焦点时自动复原搜索
  window.addEventListener("blur", () => {
    if (currentView === "search") {
      dismissSearch();
    }
  });

  // Backend shortcut toggle
  listen("activate-search", () => {
    if (currentView === "search") {
      dismissSearch();
    } else {
      activateSearch();
    }
  });

  // Async search results from backend
  listen<SearchResult[]>("search-results", (event) => {
    if (currentView === "search") {
      renderResults(event.payload);
    }
  });
}
