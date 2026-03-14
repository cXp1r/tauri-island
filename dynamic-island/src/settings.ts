import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";

type SettingsResponse = {
  clipboard_enabled: boolean;
  shortcut_key: string;
  lyric_mode: string;
};

type AISettingsResponse = {
  api_url: string;
  api_key: string;
  model: string;
  is_reasoning_model: boolean;
};

type PluginMarketRepairResult = {
  root: string;
  runtime_patched: boolean;
  archive_patched: boolean;
};

const INFLINK_URL = "https://github.com/BetterNCM/InfinityLink";

const clipboardToggle = document.getElementById("clipboard-toggle") as HTMLInputElement;
const shortcutInput = document.getElementById("shortcut-input") as HTMLInputElement;
const lyricModeSelect = document.getElementById("lyric-mode") as HTMLSelectElement;
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

let isRecording = false;
let statusTimer: number | null = null;
const shortcutHint = "请按下快捷键...";

async function loadSettings() {
  const settings = await invoke<SettingsResponse>("get_settings");
  clipboardToggle.checked = settings.clipboard_enabled;
  shortcutInput.value = settings.shortcut_key;
  lyricModeSelect.value = settings.lyric_mode || "lyric";

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

void loadSettings();
