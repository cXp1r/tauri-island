import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { capsule, searchInput, searchNextBtn, searchPageLabel, searchPrevBtn, searchResults } from "../dom";
import { currentView, isMinimized } from "../state";
import { setView } from "./view-switcher";
import { expandFromMinimized } from "./minimize-drag";

// ===== Types =====

interface SearchResult {
  id: string;
  title: string;
  desc: string;
  icon: string;
  action: string;
}

interface SearchQueryResponse {
  items: SearchResult[];
  has_next: boolean;
}

// ===== State =====

let activeIndex = -1;
let results: SearchResult[] = [];
let debounceTimer: number | null = null;
let dismissSyncTimer: number | null = null;
let isDismissingSearch = false;
const DEBOUNCE_MS = 400;
const PAGE_SIZE = 10;
let previousView: "time" | "lyric" | "agent" = "time";
let currentQuery = "";
let currentOffset = 0;
let hasNextPage = false;
let searchRequestId = 0;

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

function updatePagination() {
  const visible = currentQuery.length > 0 && (results.length > 0 || currentOffset > 0);
  searchPrevBtn.hidden = !visible;
  searchNextBtn.hidden = !visible;
  searchPageLabel.hidden = !visible;
  searchPageLabel.textContent = `第 ${Math.floor(currentOffset / PAGE_SIZE) + 1} 页`;
  searchPrevBtn.disabled = currentOffset === 0;
  searchNextBtn.disabled = !hasNextPage;
}

function resetSearchPagination() {
  currentQuery = "";
  currentOffset = 0;
  hasNextPage = false;
  updatePagination();
}

async function fetchSearchPage(query: string, offset: number) {
  const trimmed = query.trim();
  if (!trimmed) {
    searchRequestId += 1;
    resetSearchPagination();
    renderResults([], 0, false);
    return;
  }

  currentQuery = trimmed;
  currentOffset = offset;
  hasNextPage = false;
  updatePagination();

  const requestId = ++searchRequestId;

  try {
    const res = await invoke<SearchQueryResponse>("search_query", {
      query: trimmed,
      offset,
      count: PAGE_SIZE,
    });
    if (requestId !== searchRequestId || currentView !== "search" || trimmed !== currentQuery) {
      return;
    }
    renderResults(res.items, offset, res.has_next);
  } catch (err: any) {
    if (requestId !== searchRequestId || currentView !== "search" || trimmed !== currentQuery) {
      return;
    }
    const errStr = String(err);
    console.error("[Search] search_query failed:", errStr, err);
    renderError(errStr);
  }
}

// ===== Render =====

function renderResults(items: SearchResult[], offset = 0, nextPageAvailable = false) {
  results = items;
  activeIndex = items.length > 0 ? 0 : -1;
  currentOffset = offset;
  hasNextPage = nextPageAvailable;
  searchResults.innerHTML = "";
  updatePagination();

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
    el.className = "search-result-item"
      + (i === 0 ? " active" : "")
      + (item.desc ? " has-desc" : "");
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
  hasNextPage = false;
  searchResults.innerHTML = "";
  searchPrevBtn.hidden = true;
  searchNextBtn.hidden = true;
  searchPageLabel.hidden = true;

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
    debounceTimer = null;
  }
  if (!query.trim()) {
    void fetchSearchPage("", 0);
    return;
  }
  debounceTimer = window.setTimeout(async () => {
    debounceTimer = null;
    void fetchSearchPage(query, 0);
  }, DEBOUNCE_MS);
}

// ===== Activate / Dismiss =====

export function activateSearch() {
  if (isDismissingSearch) return;

  // 如果窗口处于收起状态（小绿条），先展开再激活搜索
  if (isMinimized) {
    expandFromMinimized();
    setTimeout(() => activateSearch(), 350);
    return;
  }

  // Remember where we came from so we can go back
  if (currentView !== "search") {
    previousView = currentView as "time" | "lyric" | "agent";
  }
  // Clean other expand classes
  capsule.classList.remove("expanded", "lyric-collapsed", "agent-expanded", "music-expanded", "search-expanded");
  capsule.classList.add("search-active");
  setView("search", false);
  searchRequestId += 1;
  if (debounceTimer !== null) {
    clearTimeout(debounceTimer);
    debounceTimer = null;
  }
  searchInput.value = "";
  searchResults.innerHTML = "";
  results = [];
  activeIndex = -1;
  resetSearchPagination();
  searchInput.focus();
  // 延迟再次 focus，防止后端 set_focus 与 webview input focus 竞争
  setTimeout(() => searchInput.focus(), 50);
}

export function dismissSearch() {
  if (currentView !== "search" || isDismissingSearch) return;
  isDismissingSearch = true;
  searchRequestId += 1;
  if (debounceTimer !== null) {
    clearTimeout(debounceTimer);
    debounceTimer = null;
  }
  searchInput.value = "";
  searchInput.blur();
  searchResults.innerHTML = "";
  results = [];
  activeIndex = -1;
  resetSearchPagination();
  capsule.classList.remove("search-active", "search-expanded");
  setView(previousView, true);
  // 等 CSS transition(350ms) 完成后再同步窗口高度，否则 offsetHeight 仍是展开态的值
  if (dismissSyncTimer !== null) clearTimeout(dismissSyncTimer);
  dismissSyncTimer = window.setTimeout(() => {
    dismissSyncTimer = null;
    isDismissingSearch = false;
    syncSearchWindowHeight();
  }, 360);
}

// ===== Init =====

export function initSearch() {
  // Input listener
  searchInput.addEventListener("input", () => {
    doSearch(searchInput.value);
  });

  [searchPrevBtn, searchNextBtn].forEach((btn) => {
    btn.addEventListener("mousedown", (e) => {
      e.preventDefault();
    });
  });

  searchPrevBtn.addEventListener("click", () => {
    if (!currentQuery || currentOffset === 0) return;
    void fetchSearchPage(currentQuery, Math.max(0, currentOffset - PAGE_SIZE));
    searchInput.focus();
  });

  searchNextBtn.addEventListener("click", () => {
    if (!currentQuery || !hasNextPage) return;
    void fetchSearchPage(currentQuery, currentOffset + PAGE_SIZE);
    searchInput.focus();
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
