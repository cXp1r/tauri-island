import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize, LogicalPosition } from "@tauri-apps/api/window";
import { initLyricOffset } from "./settings-lyric-offset";



type SettingsResponse = {
  clipboard_enabled: boolean;
  shortcut_key: string;
  lyric_mode: string;
  lyric_ws_enabled: boolean;
  lyric_api_search_enabled: boolean;
  lyric_rust_api_enabled: boolean;
  lyric_offset_enabled: boolean;
  indicator_color: string;
  agent_window_size: string;
  weather_city: string;
  weather_lat: number;
  weather_lon: number;
  auto_start: boolean;
  log_level: string;
};

type AISettingsResponse = {
  api_url: string;
  api_key: string;
  model: string;
  is_reasoning_model: boolean;
};

type LinkHandler = {
  id: string;
  name: string;
  pattern: string;
  app_path: string;
  enabled: boolean;
};

type PluginMarketRepairResult = {
  root: string;
  runtime_patched: boolean;
  archive_patched: boolean;
};

type CityResult = {
  name: string;
  country: string;
  admin1: string;
  latitude: number;
  longitude: number;
};

const INFLINK_URL = "https://docs.pyisland.com/guide/tauri-island.html";

const clipboardToggle = document.getElementById("clipboard-toggle") as HTMLInputElement;
const shortcutInput = document.getElementById("shortcut-input") as HTMLInputElement;
const lyricModeSelect = document.getElementById("lyric-mode") as HTMLSelectElement;
const lyricOffsetEnabledToggle = document.getElementById("lyric-offset-enabled") as HTMLInputElement;
const indicatorColorInput = document.getElementById("indicator-color") as HTMLInputElement;
const saveBtn = document.getElementById("save-btn") as HTMLButtonElement;
const statusEl = document.getElementById("status") as HTMLDivElement;
const autoStartToggle = document.getElementById("auto-start-toggle") as HTMLInputElement;
const logLevelSelect = document.getElementById("log-level-select") as HTMLSelectElement | null;

const betterncmPathInput = document.getElementById("betterncm-path") as HTMLInputElement;
const repairBtn = document.getElementById("install-betterncm-btn") as HTMLButtonElement;
const openInfLinkBtn = document.getElementById("open-inflink-btn") as HTMLButtonElement;

const aiApiUrlInput = document.getElementById("ai-api-url") as HTMLInputElement;
const aiApiKeyInput = document.getElementById("ai-api-key") as HTMLInputElement;
const aiModelInput = document.getElementById("ai-model") as HTMLInputElement;
const aiDetectBtn = document.getElementById("ai-detect-btn") as HTMLButtonElement;
const aiModelTypeResult = document.getElementById("ai-model-type-result") as HTMLParagraphElement;
const agentWindowSizeSelect = document.getElementById("agent-window-size") as HTMLSelectElement;

let isRecording = false;
let statusTimer: number | null = null;
const shortcutHint = "请按下快捷键...";

// 天气城市搜索相关
const weatherCitySearch = document.getElementById("weather-city-search") as HTMLInputElement;
const cityResultsEl = document.getElementById("city-results") as HTMLDivElement;
const cityCurrent = document.getElementById("city-current") as HTMLDivElement;
const cityTag = document.getElementById("city-tag") as HTMLSpanElement;
const clearCityBtn = document.getElementById("clear-city-btn") as HTMLButtonElement;
let citySearchTimer: number | null = null;

async function loadSettings() {
  const settings = await invoke<SettingsResponse>("get_settings");
  clipboardToggle.checked = settings.clipboard_enabled;
  shortcutInput.value = settings.shortcut_key;
  lyricModeSelect.value = settings.lyric_mode || "lyric";
  lyricOffsetEnabledToggle.checked = settings.lyric_offset_enabled ?? true;
  indicatorColorInput.value = settings.indicator_color || "#2edb67";
  agentWindowSizeSelect.value = settings.agent_window_size || "medium";
  autoStartToggle.checked = settings.auto_start || false;

  // 加载日志等级
  if (logLevelSelect) {
    logLevelSelect.value = settings.log_level || "info";
  }

  // 加载 AI 设置
  try {
    const aiSettings = await invoke<AISettingsResponse>("ai_get_settings");
    aiApiUrlInput.value = aiSettings.api_url || "";
    aiApiKeyInput.value = aiSettings.api_key || "";
    aiModelInput.value = aiSettings.model || "";

    if (aiSettings.is_reasoning_model) {
      aiModelTypeResult.textContent = "✅ 思考模型";
      aiModelTypeResult.style.color = "#39d98a";
    } else if (aiSettings.model) {
      aiModelTypeResult.textContent = "普通模型";
      aiModelTypeResult.style.color = "#93a4c8";
    } else {
      aiModelTypeResult.textContent = "未检测";
      aiModelTypeResult.style.color = "#93a4c8";
    }
  } catch (e) {
    console.error("加载 AI 设置失败:", e);
  }

  // 加载天气城市
  if (settings.weather_city) {
    cityTag.textContent = settings.weather_city;
    cityCurrent.style.display = "flex";
  } else {
    cityCurrent.style.display = "none";
  }
}

function openExternal(url: string) {
  void invoke("open_url", { url });
}

function showStatus(msg: string, isError = false, durationMs = 2600) {
  if (statusTimer) {
    clearTimeout(statusTimer);
    statusTimer = null;
  }
  statusEl.textContent = msg;
  statusEl.style.color = isError ? "#ff6f7f" : "#39d98a";
  statusTimer = window.setTimeout(() => {
    statusEl.textContent = "";
    statusTimer = null;
  }, durationMs);
}

shortcutInput.addEventListener("click", () => {
  isRecording = true;
  shortcutInput.value = shortcutHint;
  shortcutInput.classList.add("recording");
});

shortcutInput.addEventListener("blur", () => {
  if (!isRecording) return;
  isRecording = false;
  shortcutInput.classList.remove("recording");
  void loadSettings();
});

shortcutInput.addEventListener("keydown", (e: KeyboardEvent) => {
  if (!isRecording) return;
  e.preventDefault();

  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push("Super");

  const ignored = ["Control", "Alt", "Shift", "Meta"];
  if (!ignored.includes(e.key)) {
    parts.push(e.key.length === 1 ? e.key.toUpperCase() : e.key);
    shortcutInput.value = parts.join("+");
    shortcutInput.classList.remove("recording");
    isRecording = false;
  }
});

// 歌词偏移补偿总开关及按播放器子页的所有交互，集中在 settings-lyric-offset 模块处理
initLyricOffset();

saveBtn.addEventListener("click", async () => {
  const shortcut = shortcutInput.value.trim();
  if (!shortcut || shortcut === shortcutHint) {
    showStatus("请先设置快捷键", true);
    return;
  }

  try {
    await invoke("save_settings", {
      clipboardEnabled: clipboardToggle.checked,
      shortcutKey: shortcut,
      lyricMode: lyricModeSelect.value,
      lyricOffsetEnabled: lyricOffsetEnabledToggle.checked,
      indicatorColor: indicatorColorInput.value,
      agentWindowSize: agentWindowSizeSelect.value,
      autoStart: autoStartToggle.checked,
      logLevel: logLevelSelect ? logLevelSelect.value : undefined,
    });

    // 保存 AI 设置
    const apiUrl = aiApiUrlInput.value.trim();
    const apiKey = aiApiKeyInput.value.trim();
    const model = aiModelInput.value.trim();

    if (apiUrl || apiKey || model) {
      await invoke("ai_save_settings", {
        apiUrl: apiUrl,
        apiKey: apiKey,
        model: model,
      });

      // 通知主窗口更新 AI 状态
      await emit("ai-settings-changed", {});
    }

    // 保存设置时顺带检测模型类型（配置完整时触发）
    if (apiUrl && apiKey && model) {
      aiModelTypeResult.textContent = "检测中...";
      aiModelTypeResult.style.color = "#93a4c8";
      try {
        await invoke("ai_detect_model_type");
      } catch {
        aiModelTypeResult.textContent = "检测失败";
        aiModelTypeResult.style.color = "#ff6f7f";
      }
    }

    // 保存链接处理器
    await invoke("save_link_handlers", { handlers: linkHandlers });

    showStatus("设置已保存");
  } catch (e) {
    showStatus(`保存失败: ${String(e)}`, true, 4500);
  }
});

openInfLinkBtn.addEventListener("click", () => {
  openExternal(INFLINK_URL);
});

repairBtn.addEventListener("click", async () => {
  const installRoot = betterncmPathInput.value.trim();
  const originalText = repairBtn.textContent || "执行 main.js 源修复";

  repairBtn.disabled = true;
  repairBtn.textContent = "修复中...";

  try {
    const result = await invoke<PluginMarketRepairResult>("install_betterncm_support", {
      installRoot: installRoot || null,
    });

    const parts: string[] = [];
    parts.push(result.runtime_patched ? "运行时 main.js 已替换" : "运行时 main.js 无需替换");
    parts.push(result.archive_patched ? "Plugin 包 main.js 已替换" : "Plugin 包 main.js 无需替换");

    showStatus(`修复完成（${result.root}）：${parts.join("，")}`, false, 7000);
  } catch (e) {
    showStatus(`修复失败: ${String(e)}`, true, 7000);
  } finally {
    repairBtn.disabled = false;
    repairBtn.textContent = originalText;
  }
});

aiDetectBtn.addEventListener("click", async () => {
  const apiUrl = aiApiUrlInput.value.trim();
  const apiKey = aiApiKeyInput.value.trim();
  const model = aiModelInput.value.trim();

  if (!apiUrl || !apiKey || !model) {
    showStatus("请先填写完整的 AI 配置", true);
    return;
  }

  aiDetectBtn.disabled = true;
  aiDetectBtn.textContent = "检测中...";
  aiModelTypeResult.textContent = "检测中...";
  aiModelTypeResult.style.color = "#93a4c8";

  try {
    // 先保存设置
    await invoke("ai_save_settings", { apiUrl, apiKey, model });
    // 触发后台检测（结果通过 ai-model-type-detected 事件返回）
    await invoke("ai_detect_model_type");
    showStatus("模型检测已发起，请稍候...");
  } catch (e) {
    aiModelTypeResult.textContent = "检测失败";
    aiModelTypeResult.style.color = "#ff6f7f";
    showStatus(`检测失败: ${String(e)}`, true, 4500);
  } finally {
    aiDetectBtn.disabled = false;
    aiDetectBtn.textContent = "检测模型类型";
  }
});

// 监听后端 AI 模型检测结果
listen<{ is_reasoning_model: boolean }>("ai-model-type-detected", (event) => {
  const result = event.payload;
  if (result.is_reasoning_model) {
    aiModelTypeResult.textContent = "✅ 思考模型";
    aiModelTypeResult.style.color = "#39d98a";
    showStatus("检测完成：这是一个思考模型");
  } else {
    aiModelTypeResult.textContent = "普通模型";
    aiModelTypeResult.style.color = "#93a4c8";
    showStatus("检测完成：这是一个普通模型");
  }
  // 通知主窗口更新 AI 状态
  void emit("ai-settings-changed", {});
});

// 窗口调整大小功能
const appWindow = getCurrentWindow();
let isResizing = false;
let resizeDirection = "";
let startX = 0;
let startY = 0;
let startWidth = 0;
let startHeight = 0;
let startPosX = 0;
let startPosY = 0;

const resizeHandles = document.querySelectorAll(".resize-handle");

resizeHandles.forEach((handle) => {
  handle.addEventListener("mousedown", async (e: Event) => {
    const mouseEvent = e as MouseEvent;
    e.preventDefault();
    isResizing = true;
    resizeDirection = (handle as HTMLElement).dataset.direction || "";
    startX = mouseEvent.screenX;
    startY = mouseEvent.screenY;

    const size = await appWindow.outerSize();
    const position = await appWindow.outerPosition();
    startWidth = size.width;
    startHeight = size.height;
    startPosX = position.x;
    startPosY = position.y;
  });
});

document.addEventListener("mousemove", async (e: MouseEvent) => {
  if (!isResizing) return;

  const deltaX = e.screenX - startX;
  const deltaY = e.screenY - startY;

  let newWidth = startWidth;
  let newHeight = startHeight;
  let newX = startPosX;
  let newY = startPosY;

  // 最小尺寸限制
  const minWidth = 600;
  const minHeight = 400;

  if (resizeDirection.includes("e")) {
    newWidth = Math.max(minWidth, startWidth + deltaX);
  }
  if (resizeDirection.includes("w")) {
    const proposedWidth = startWidth - deltaX;
    if (proposedWidth >= minWidth) {
      newWidth = proposedWidth;
      newX = startPosX + deltaX;
    }
  }
  if (resizeDirection.includes("s")) {
    newHeight = Math.max(minHeight, startHeight + deltaY);
  }
  if (resizeDirection.includes("n")) {
    const proposedHeight = startHeight - deltaY;
    if (proposedHeight >= minHeight) {
      newHeight = proposedHeight;
      newY = startPosY + deltaY;
    }
  }

  try {
    await appWindow.setSize(new LogicalSize(newWidth, newHeight));
    if (newX !== startPosX || newY !== startPosY) {
      await appWindow.setPosition(new LogicalPosition(newX, newY));
    }
  } catch (err) {
    console.error("调整窗口大小失败:", err);
  }
});

document.addEventListener("mouseup", () => {
  isResizing = false;
  resizeDirection = "";
});

void loadSettings();

// 链接处理器管理
const linkHandlersList = document.getElementById("link-handlers-list") as HTMLDivElement;
const addHandlerBtn = document.getElementById("add-handler-btn") as HTMLButtonElement;
const handlerDetailPage = document.getElementById("handler-detail-page") as HTMLDivElement;
const handlerDetailBack = document.getElementById("handler-detail-back") as HTMLButtonElement;
const handlerDetailTitle = document.getElementById("handler-detail-title") as HTMLHeadingElement;
const handlerDetailSave = document.getElementById("handler-detail-save") as HTMLButtonElement;
const handlerDetailDelete = document.getElementById("handler-detail-delete") as HTMLButtonElement;
const testAppBtn = document.getElementById("test-app-btn") as HTMLButtonElement;
const detailName = document.getElementById("detail-name") as HTMLInputElement;
const detailPattern = document.getElementById("detail-pattern") as HTMLInputElement;
const detailAppPath = document.getElementById("detail-app-path") as HTMLInputElement;

let linkHandlers: LinkHandler[] = [];
let editingHandlerIndex: number = -1;

async function loadLinkHandlers() {
  try {
    linkHandlers = await invoke<LinkHandler[]>("get_link_handlers");
    renderLinkHandlersList();
  } catch (e) {
    console.error("加载链接处理器失败:", e);
  }
}

function renderLinkHandlersList() {
  linkHandlersList.innerHTML = "";

  if (linkHandlers.length === 0) {
    const emptyMsg = document.createElement("p");
    emptyMsg.style.color = "var(--text-muted)";
    emptyMsg.style.fontSize = "13px";
    emptyMsg.textContent = "暂无处理器，点击下方按钮添加。";
    linkHandlersList.appendChild(emptyMsg);
    return;
  }

  linkHandlers.forEach((handler, index) => {
    const item = document.createElement("div");
    item.className = "handler-list-item";

    const info = document.createElement("div");
    info.className = "handler-list-info";

    const nameSpan = document.createElement("span");
    nameSpan.className = "handler-list-name" + (handler.name ? "" : " empty");
    nameSpan.textContent = handler.name || "未命名处理器";
    info.appendChild(nameSpan);

    const status = document.createElement("span");
    status.className = "handler-list-status" + (handler.enabled ? " active" : "");
    status.textContent = handler.enabled ? "已启用" : "已禁用";
    info.appendChild(status);

    item.appendChild(info);

    const actions = document.createElement("div");
    actions.className = "handler-list-actions";

    const switchLabel = document.createElement("label");
    switchLabel.className = "switch";
    switchLabel.style.transform = "scale(0.8)";

    const toggleInput = document.createElement("input");
    toggleInput.type = "checkbox";
    toggleInput.checked = handler.enabled;
    toggleInput.addEventListener("change", () => {
      linkHandlers[index].enabled = toggleInput.checked;
    });
    switchLabel.appendChild(toggleInput);

    const slider = document.createElement("span");
    slider.className = "slider";
    switchLabel.appendChild(slider);

    actions.appendChild(switchLabel);

    const configBtn = document.createElement("button");
    configBtn.className = "btn btn-small";
    configBtn.textContent = "配置";
    configBtn.addEventListener("click", () => {
      openHandlerDetail(index);
    });
    actions.appendChild(configBtn);

    item.appendChild(actions);
    linkHandlersList.appendChild(item);
  });
}

function openHandlerDetail(index: number) {
  editingHandlerIndex = index;
  const handler = linkHandlers[index];

  detailName.value = handler.name;
  detailPattern.value = handler.pattern;
  detailAppPath.value = handler.app_path;

  handlerDetailTitle.textContent = handler.name || "配置处理器";
  handlerDetailPage.classList.add("active");
}

function closeHandlerDetail() {
  handlerDetailPage.classList.remove("active");
  editingHandlerIndex = -1;
}

handlerDetailBack.addEventListener("click", () => {
  closeHandlerDetail();
});

handlerDetailSave.addEventListener("click", () => {
  if (editingHandlerIndex < 0 || editingHandlerIndex >= linkHandlers.length) {
    return;
  }

  linkHandlers[editingHandlerIndex].name = detailName.value.trim();
  linkHandlers[editingHandlerIndex].pattern = detailPattern.value.trim();
  linkHandlers[editingHandlerIndex].app_path = detailAppPath.value.trim();

  renderLinkHandlersList();
  closeHandlerDetail();
  showStatus("处理器已更新");
});

handlerDetailDelete.addEventListener("click", () => {
  if (editingHandlerIndex < 0 || editingHandlerIndex >= linkHandlers.length) {
    return;
  }

  linkHandlers.splice(editingHandlerIndex, 1);
  renderLinkHandlersList();
  closeHandlerDetail();
  showStatus("处理器已删除");
});

testAppBtn.addEventListener("click", async () => {
  const appPath = detailAppPath.value.trim();

  if (!appPath) {
    showStatus("请先填写应用路径", true);
    return;
  }

  testAppBtn.disabled = true;
  testAppBtn.textContent = "测试中...";

  try {
    await invoke("test_link_handler", { appPath });
    showStatus("应用启动成功");
  } catch (e) {
    showStatus(`启动失败: ${String(e)}`, true, 4500);
  } finally {
    testAppBtn.disabled = false;
    testAppBtn.textContent = "测试打开";
  }
});

addHandlerBtn.addEventListener("click", () => {
  const newHandler: LinkHandler = {
    id: `handler-${Date.now()}`,
    name: "",
    pattern: "",
    app_path: "",
    enabled: true,
  };
  linkHandlers.push(newHandler);
  openHandlerDetail(linkHandlers.length - 1);
});

// 页面加载时加载链接处理器
void loadLinkHandlers();

// ===== 天气城市搜索 =====
weatherCitySearch.addEventListener("input", () => {
  if (citySearchTimer) {
    clearTimeout(citySearchTimer);
  }
  const query = weatherCitySearch.value.trim();
  if (!query) {
    cityResultsEl.classList.remove("active");
    cityResultsEl.innerHTML = "";
    return;
  }
  citySearchTimer = window.setTimeout(async () => {
    try {
      const results = await invoke<CityResult[]>("search_city", { query });
      cityResultsEl.innerHTML = "";
      if (results.length === 0) {
        const empty = document.createElement("div");
        empty.className = "city-result-item";
        empty.style.color = "var(--text-muted)";
        empty.textContent = "未找到匹配城市";
        cityResultsEl.appendChild(empty);
      } else {
        results.forEach((city) => {
          const item = document.createElement("div");
          item.className = "city-result-item";
          const nameDiv = document.createElement("div");
          nameDiv.className = "city-name";
          nameDiv.textContent = city.name;
          item.appendChild(nameDiv);
          const detailDiv = document.createElement("div");
          detailDiv.className = "city-detail";
          detailDiv.textContent = [city.admin1, city.country].filter(Boolean).join(", ");
          item.appendChild(detailDiv);
          item.addEventListener("click", async () => {
            await invoke("save_weather_city", {
              city: city.name,
              lat: city.latitude,
              lon: city.longitude,
            });
            cityTag.textContent = city.name;
            cityCurrent.style.display = "flex";
            weatherCitySearch.value = "";
            cityResultsEl.classList.remove("active");
            cityResultsEl.innerHTML = "";
            showStatus(`天气位置已设置为 ${city.name}`);
          });
          cityResultsEl.appendChild(item);
        });
      }
      cityResultsEl.classList.add("active");
      // 自动滚动让搜索结果可见
      cityResultsEl.scrollIntoView({ behavior: "smooth", block: "nearest" });
    } catch (e) {
      console.error("搜索城市失败:", e);
    }
  }, 400);
});

// 点击外部关闭下拉
document.addEventListener("click", (e) => {
  if (!weatherCitySearch.contains(e.target as Node) && !cityResultsEl.contains(e.target as Node)) {
    cityResultsEl.classList.remove("active");
  }
});

clearCityBtn.addEventListener("click", async () => {
  await invoke("save_weather_city", { city: "", lat: 0.0, lon: 0.0 });
  cityCurrent.style.display = "none";
  cityTag.textContent = "";
  showStatus("已清除天气位置，将使用自动定位");
});

// ==================== 关于与更新 ====================

type UpdateInfo = {
  has_update: boolean;
  current_version: string;
  latest_version: string;
  release_notes: string;
  download_url: string;
  published_at: string;
  file_size: number;
};

const currentVersionEl = document.getElementById("current-version") as HTMLSpanElement;
const updateStatusText = document.getElementById("update-status-text") as HTMLParagraphElement;
const updateInfoCard = document.getElementById("update-info-card") as HTMLDivElement;
const updateLatestVersion = document.getElementById("update-latest-version") as HTMLSpanElement;
const updatePublished = document.getElementById("update-published") as HTMLParagraphElement;
const updateNotes = document.getElementById("update-notes") as HTMLDivElement;
const updateCardTitle = document.getElementById("update-card-title") as HTMLSpanElement;
const updateProgressWrapper = document.getElementById("update-progress-wrapper") as HTMLDivElement;
const updateProgressText = document.getElementById("update-progress-text") as HTMLSpanElement;
const updateProgressPercent = document.getElementById("update-progress-percent") as HTMLSpanElement;
const updateProgressBar = document.getElementById("update-progress-bar") as HTMLDivElement;
const checkUpdateBtn = document.getElementById("check-update-btn") as HTMLButtonElement;
const downloadUpdateBtn = document.getElementById("download-update-btn") as HTMLButtonElement;
const openReleaseBtn = document.getElementById("open-release-btn") as HTMLButtonElement;
const openGithubBtn = document.getElementById("open-github-btn") as HTMLButtonElement;
const previewUpdatesToggle = document.getElementById("preview-updates-toggle") as HTMLInputElement;
const previewToggleRow = document.getElementById("preview-toggle-row") as HTMLElement;
const disablePreviewWrap = document.getElementById("disable-preview-wrap") as HTMLElement;
const disablePreviewBtn = document.getElementById("disable-preview-btn") as HTMLButtonElement;

let pendingDownloadUrl = "";
let showPreviewEnabled = false;

// 加载预览更新开关
invoke<boolean>("get_preview_updates").then((enabled) => {
  if (previewUpdatesToggle) previewUpdatesToggle.checked = enabled;
}).catch(() => {});

if (previewUpdatesToggle) {
  previewUpdatesToggle.addEventListener("change", () => {
    void invoke("set_preview_updates", { enabled: previewUpdatesToggle.checked });
  });
}

// 后端控制：是否显示预览版开关行
function applyPreviewVisibility(enabled: boolean) {
  showPreviewEnabled = enabled;
  if (previewToggleRow) previewToggleRow.style.display = enabled ? "" : "none";
  if (disablePreviewWrap) disablePreviewWrap.style.display = enabled ? "" : "none";
}

invoke<boolean>("get_show_preview_toggle").then(applyPreviewVisibility).catch(() => {});

if (disablePreviewBtn) {
  disablePreviewBtn.addEventListener("click", () => {
    void invoke("set_show_preview_toggle", { enabled: false });
    applyPreviewVisibility(false);
  });
}

// 加载当前版本号
invoke<string>("get_app_version").then((ver) => {
  currentVersionEl.textContent = `v${ver}`;
}).catch(() => {
  currentVersionEl.textContent = "未知";
});

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

checkUpdateBtn.addEventListener("click", async () => {
  checkUpdateBtn.disabled = true;
  checkUpdateBtn.textContent = "检查中...";
  updateStatusText.style.color = "var(--text-secondary)";
  updateStatusText.textContent = "正在检查更新...";
  updateInfoCard.style.display = "none";
  downloadUpdateBtn.style.display = "none";
  openReleaseBtn.style.display = "none";

  const isPreview = showPreviewEnabled && (previewUpdatesToggle?.checked ?? false);

  let failed = false;
  try {
    const info = await invoke<UpdateInfo>("check_for_updates", { preview: isPreview });
    currentVersionEl.textContent = `v${info.current_version}`;

    if (info.has_update) {
      updateStatusText.textContent = isPreview ? "发现预览构建！" : "发现新版本！";
      updateStatusText.style.color = "var(--primary)";
      if (updateCardTitle) updateCardTitle.textContent = isPreview ? "🚧 发现预览构建" : "🎉 发现新版本";
      updateLatestVersion.textContent = isPreview ? `预览: ${info.latest_version}` : `v${info.latest_version}`;
      updatePublished.textContent = info.published_at
        ? `发布于 ${new Date(info.published_at).toLocaleDateString("zh-CN")}`
        : "";
      updateNotes.textContent = info.release_notes || "暂无更新说明";
      updateInfoCard.style.display = "block";
      downloadUpdateBtn.style.display = "inline-flex";
      openReleaseBtn.style.display = "inline-flex";
      pendingDownloadUrl = info.download_url;
    } else {
      updateStatusText.textContent = isPreview
        ? `当前是最新测试版 (v${info.current_version})`
        : `当前是主分支最新版 (v${info.current_version})`;
      updateStatusText.style.color = "var(--ok)";
    }
  } catch (e) {
    failed = true;
    updateStatusText.textContent = `检查更新失败: ${e}`;
    updateStatusText.style.color = "var(--danger)";
  } finally {
    if (failed) {
      // 失败后冷却 10 秒，防止频繁触发 rate limit
      let cd = 10;
      checkUpdateBtn.textContent = `重试 (${cd}s)`;
      const cdTimer = setInterval(() => {
        cd--;
        if (cd <= 0) {
          clearInterval(cdTimer);
          checkUpdateBtn.disabled = false;
          checkUpdateBtn.textContent = "检查更新";
        } else {
          checkUpdateBtn.textContent = `重试 (${cd}s)`;
        }
      }, 1000);
    } else {
      checkUpdateBtn.disabled = false;
      checkUpdateBtn.textContent = "检查更新";
    }
  }
});

downloadUpdateBtn.addEventListener("click", async () => {
  if (!pendingDownloadUrl) return;
  downloadUpdateBtn.disabled = true;
  downloadUpdateBtn.textContent = "下载中...";
  updateProgressWrapper.style.display = "block";
  updateProgressBar.style.width = "0%";
  updateProgressPercent.textContent = "0%";
  updateProgressText.textContent = "下载中...";

  try {
    await invoke("download_and_install_update", { url: pendingDownloadUrl });
  } catch (e) {
    updateProgressText.textContent = `下载失败: ${e}`;
    updateProgressText.style.color = "var(--danger)";
    downloadUpdateBtn.disabled = false;
    downloadUpdateBtn.textContent = "重试下载";
  }
});

// 监听下载进度
listen<{ downloaded: number; total: number; percent: number }>("update-download-progress", (event) => {
  const { downloaded, total, percent } = event.payload;
  updateProgressBar.style.width = `${percent.toFixed(1)}%`;
  updateProgressPercent.textContent = `${percent.toFixed(1)}%`;
  updateProgressText.textContent = `${formatFileSize(downloaded)} / ${formatFileSize(total)}`;
});

// 监听下载完成
listen("update-download-complete", () => {
  updateProgressText.textContent = "下载完成，正在启动安装程序...";
  updateProgressBar.style.width = "100%";
  updateProgressPercent.textContent = "100%";
});

// 监听下载错误
listen<string>("update-error", (event) => {
  updateProgressText.textContent = `错误: ${event.payload}`;
  updateProgressText.style.color = "var(--danger)";
  downloadUpdateBtn.disabled = false;
  downloadUpdateBtn.textContent = "重试下载";
});

openReleaseBtn.addEventListener("click", () => {
  const url = (previewUpdatesToggle?.checked)
    ? "https://github.com/cXp1r/Python-island/releases/tag/tauri-test"
    : "https://github.com/Python-island/Python-island/releases/latest";
  invoke("open_url", { url });
});

openGithubBtn.addEventListener("click", () => {
  invoke("open_url", { url: "https://github.com/Python-island/Python-island" });
});

const logPathText = document.getElementById("log-path-text") as HTMLParagraphElement;
const openLogDirBtn = document.getElementById("open-log-dir-btn") as HTMLButtonElement;

invoke<string>("get_log_path").then((p) => {
  if (logPathText) logPathText.textContent = p;
}).catch(() => {
  if (logPathText) logPathText.textContent = "获取失败";
});

if (openLogDirBtn) {
  openLogDirBtn.addEventListener("click", () => {
    void invoke("open_log_dir");
  });
}

// ==================== 黑名单 ====================

const blacklistInput = document.getElementById("blacklist-input") as HTMLInputElement | null;
const blacklistAddBtn = document.getElementById("blacklist-add-btn") as HTMLButtonElement | null;
const blacklistList = document.getElementById("blacklist-list") as HTMLDivElement | null;
const blacklistEnabledToggle = document.getElementById("blacklist-enabled-toggle") as HTMLInputElement | null;
const blacklistContentGroup = document.getElementById("blacklist-content-group") as HTMLDivElement | null;

let blacklistProcesses: string[] = [];

function updateBlacklistContentVisibility(enabled: boolean) {
  if (blacklistContentGroup) {
    blacklistContentGroup.style.opacity = enabled ? "1" : "0.4";
    blacklistContentGroup.style.pointerEvents = enabled ? "" : "none";
  }
}

async function loadBlacklist() {
  try {
    blacklistProcesses = await invoke<string[]>("get_blacklist");
    renderBlacklist();
  } catch (e) {
    console.error("加载黑名单失败:", e);
  }
}

async function loadBlacklistEnabled() {
  try {
    const enabled = await invoke<boolean>("get_blacklist_enabled");
    if (blacklistEnabledToggle) blacklistEnabledToggle.checked = enabled;
    updateBlacklistContentVisibility(enabled);
  } catch (e) {
    console.error("加载黑名单开关失败:", e);
  }
}

if (blacklistEnabledToggle) {
  blacklistEnabledToggle.addEventListener("change", async () => {
    const enabled = blacklistEnabledToggle.checked;
    updateBlacklistContentVisibility(enabled);
    try {
      await invoke("set_blacklist_enabled", { enabled });
      showStatus(enabled ? "黑名单已启用" : "黑名单已禁用");
    } catch (e) {
      showStatus(`保存失败: ${String(e)}`, true, 4500);
    }
  });
}

function renderBlacklist() {
  if (!blacklistList) return;
  blacklistList.innerHTML = "";

  if (blacklistProcesses.length === 0) {
    const empty = document.createElement("p");
    empty.style.color = "var(--text-muted)";
    empty.style.fontSize = "13px";
    empty.textContent = "黑名单为空，添加进程名后生效。";
    blacklistList.appendChild(empty);
    return;
  }

  blacklistProcesses.forEach((name, index) => {
    const row = document.createElement("div");
    row.style.cssText = "display:flex;align-items:center;justify-content:space-between;padding:8px 12px;background:var(--surface);border-radius:8px;gap:8px;";

    const label = document.createElement("span");
    label.style.cssText = "font-family:monospace;font-size:13px;color:var(--text);flex:1;";
    label.textContent = name;
    row.appendChild(label);

    const delBtn = document.createElement("button");
    delBtn.className = "btn btn-small";
    delBtn.style.color = "var(--danger, #ff6f7f)";
    delBtn.textContent = "删除";
    delBtn.addEventListener("click", async () => {
      blacklistProcesses.splice(index, 1);
      renderBlacklist();
      await saveBlacklist();
    });
    row.appendChild(delBtn);
    blacklistList.appendChild(row);
  });
}

async function saveBlacklist() {
  try {
    await invoke("save_blacklist", { processes: blacklistProcesses });
    showStatus("黑名单已保存");
  } catch (e) {
    showStatus(`保存失败: ${String(e)}`, true, 4500);
  }
}

async function addBlacklistEntry() {
  if (!blacklistInput) return;
  const val = blacklistInput.value.trim().toLowerCase();
  if (!val) return;
  if (blacklistProcesses.includes(val)) {
    showStatus("该进程已在黑名单中", true);
    return;
  }
  blacklistProcesses.push(val);
  blacklistInput.value = "";
  renderBlacklist();
  await saveBlacklist();
}

if (blacklistAddBtn) blacklistAddBtn.addEventListener("click", () => void addBlacklistEntry());
if (blacklistInput) blacklistInput.addEventListener("keydown", (e) => {
  if (e.key === "Enter") void addBlacklistEntry();
});

void loadBlacklist();
void loadBlacklistEnabled();

// ==================== SMTC 白名单 ====================

const smtcWhitelistInput = document.getElementById("smtc-whitelist-input") as HTMLInputElement | null;
const smtcWhitelistAddBtn = document.getElementById("smtc-whitelist-add-btn") as HTMLButtonElement | null;
const smtcWhitelistList = document.getElementById("smtc-whitelist-list") as HTMLDivElement | null;
const smtcWhitelistEnabledToggle = document.getElementById("smtc-whitelist-enabled") as HTMLInputElement | null;
const smtcWhitelistContentGroup = document.getElementById("smtc-whitelist-content-group") as HTMLDivElement | null;

let smtcWhitelistApps: string[] = [];

function updateSmtcWhitelistContentVisibility(enabled: boolean) {
  if (smtcWhitelistContentGroup) {
    smtcWhitelistContentGroup.style.opacity = enabled ? "1" : "0.4";
    smtcWhitelistContentGroup.style.pointerEvents = enabled ? "" : "none";
  }
}

async function loadSmtcWhitelist() {
  try {
    smtcWhitelistApps = await invoke<string[]>("get_smtc_whitelist");
    renderSmtcWhitelist();
  } catch (e) {
    console.error("加载 SMTC 白名单失败:", e);
  }
}

async function loadSmtcWhitelistEnabled() {
  try {
    const enabled = await invoke<boolean>("get_smtc_whitelist_enabled");
    if (smtcWhitelistEnabledToggle) smtcWhitelistEnabledToggle.checked = enabled;
    updateSmtcWhitelistContentVisibility(enabled);
  } catch (e) {
    console.error("加载 SMTC 白名单开关失败:", e);
  }
}

if (smtcWhitelistEnabledToggle) {
  smtcWhitelistEnabledToggle.addEventListener("change", async () => {
    const enabled = smtcWhitelistEnabledToggle.checked;
    updateSmtcWhitelistContentVisibility(enabled);
    try {
      await invoke("set_smtc_whitelist_enabled", { enabled });
      showStatus(enabled ? "SMTC 白名单已启用" : "SMTC 白名单已禁用");
    } catch (e) {
      showStatus(`保存失败: ${String(e)}`, true, 4500);
    }
  });
}

function renderSmtcWhitelist() {
  if (!smtcWhitelistList) return;
  smtcWhitelistList.innerHTML = "";

  if (smtcWhitelistApps.length === 0) {
    const empty = document.createElement("p");
    empty.style.color = "var(--text-muted)";
    empty.style.fontSize = "13px";
    empty.textContent = "白名单为空，添加 app_id 后生效。";
    smtcWhitelistList.appendChild(empty);
    return;
  }

  smtcWhitelistApps.forEach((name, index) => {
    const row = document.createElement("div");
    row.style.cssText = "display:flex;align-items:center;justify-content:space-between;padding:8px 12px;background:var(--surface);border-radius:8px;gap:8px;";

    const label = document.createElement("span");
    label.textContent = name;
    row.appendChild(label);

    const delBtn = document.createElement("button");
    delBtn.className = "btn btn-small";
    delBtn.style.color = "var(--danger, #ff6f7f)";
    delBtn.textContent = "删除";
    delBtn.addEventListener("click", async () => {
      smtcWhitelistApps.splice(index, 1);
      renderSmtcWhitelist();
      await saveSmtcWhitelist();
    });
    row.appendChild(delBtn);
    smtcWhitelistList.appendChild(row);
  });
}

async function saveSmtcWhitelist() {
  try {
    await invoke("save_smtc_whitelist", { appIds: smtcWhitelistApps });
    showStatus("SMTC 白名单已保存");
  } catch (e) {
    showStatus(`保存失败: ${String(e)}`, true, 4500);
  }
}

async function addSmtcWhitelistEntry() {
  if (!smtcWhitelistInput) return;
  const val = smtcWhitelistInput.value.trim().toLowerCase();
  if (!val) return;
  if (smtcWhitelistApps.includes(val)) {
    showStatus("该 app_id 已在白名单中", true);
    return;
  }
  smtcWhitelistApps.push(val);
  smtcWhitelistInput.value = "";
  renderSmtcWhitelist();
  await saveSmtcWhitelist();
}

if (smtcWhitelistAddBtn) smtcWhitelistAddBtn.addEventListener("click", () => void addSmtcWhitelistEntry());
if (smtcWhitelistInput) smtcWhitelistInput.addEventListener("keydown", (e) => {
  if (e.key === "Enter") void addSmtcWhitelistEntry();
});

void loadSmtcWhitelist();
void loadSmtcWhitelistEnabled();
