import { invoke } from "@tauri-apps/api/core";

type SettingsResponse = {
  clipboard_enabled: boolean;
  shortcut_key: string;
  lyric_mode: string;
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

let isRecording = false;
let statusTimer: number | null = null;
const shortcutHint = "请按下快捷键...";

async function loadSettings() {
  const settings = await invoke<SettingsResponse>("get_settings");
  clipboardToggle.checked = settings.clipboard_enabled;
  shortcutInput.value = settings.shortcut_key;
  lyricModeSelect.value = settings.lyric_mode || "lyric";
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
    showStatus("基础设置已保存");
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

void loadSettings();
