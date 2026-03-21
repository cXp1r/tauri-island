import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize, LogicalPosition } from "@tauri-apps/api/window";

type SettingsResponse = {
  clipboard_enabled: boolean;
  shortcut_key: string;
  lyric_mode: string;
  indicator_color: string;
  agent_window_size: string;
  weather_city: string;
  weather_lat: number;
  weather_lon: number;
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

const INFLINK_URL = "https://github.com/BetterNCM/InfinityLink";

const clipboardToggle = document.getElementById("clipboard-toggle") as HTMLInputElement;
const shortcutInput = document.getElementById("shortcut-input") as HTMLInputElement;
const lyricModeSelect = document.getElementById("lyric-mode") as HTMLSelectElement;
const indicatorColorInput = document.getElementById("indicator-color") as HTMLInputElement;
const saveBtn = document.getElementById("save-btn") as HTMLButtonElement;
const statusEl = document.getElementById("status") as HTMLDivElement;

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
  indicatorColorInput.value = settings.indicator_color || "#2edb67";
  agentWindowSizeSelect.value = settings.agent_window_size || "medium";

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
      indicatorColor: indicatorColorInput.value,
      agentWindowSize: agentWindowSizeSelect.value,
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

      // 自动检测模型类型（后台执行，不阻塞保存）
      if (apiUrl && apiKey && model) {
        aiModelTypeResult.textContent = "检测中...";
        aiModelTypeResult.style.color = "#93a4c8";
        invoke<{ is_reasoning_model: boolean }>("ai_detect_model_type")
          .then((result) => {
            if (result.is_reasoning_model) {
              aiModelTypeResult.textContent = "✅ 思考模型";
              aiModelTypeResult.style.color = "#39d98a";
            } else {
              aiModelTypeResult.textContent = "普通模型";
              aiModelTypeResult.style.color = "#93a4c8";
            }
            // 通知主窗口更新
            void emit("ai-settings-changed", {});
          })
          .catch(() => {
            aiModelTypeResult.textContent = "检测失败";
            aiModelTypeResult.style.color = "#ff6f7f";
          });
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

  const originalText = aiDetectBtn.textContent || "检测模型类型";
  aiDetectBtn.disabled = true;
  aiDetectBtn.textContent = "检测中...";
  aiModelTypeResult.textContent = "检测中...";
  aiModelTypeResult.style.color = "#93a4c8";

  try {
    // 先保存设置
    await invoke("ai_save_settings", {
      apiUrl: apiUrl,
      apiKey: apiKey,
      model: model,
    });

    // 执行检测
    const result = await invoke<{ is_reasoning_model: boolean }>("ai_detect_model_type");

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
    await emit("ai-settings-changed", {});
  } catch (e) {
    aiModelTypeResult.textContent = "检测失败";
    aiModelTypeResult.style.color = "#ff6f7f";
    showStatus(`检测失败: ${String(e)}`, true, 4500);
  } finally {
    aiDetectBtn.disabled = false;
    aiDetectBtn.textContent = originalText;
  }
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
