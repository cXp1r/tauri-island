import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { marked } from "marked";
import katex from "katex";
// @ts-ignore
import "katex/dist/katex.min.css";
import { hljs } from "../highlight-setup";
import {
  capsule,
  agentMessages, agentInput, agentSendBtn, agentStopBtn,
  agentModelName, agentStatusLabel, agentClearBtn,
  agentConfirmDialog, agentConfirmCancel, agentConfirmOk,
} from "../dom";
import {
  aiEnabled, setAiEnabled,
  aiGenerating, setAiGenerating,
  currentAssistantMessage, setCurrentAssistantMessage,
  currentAssistantRawText, setCurrentAssistantRawText,
  currentThinkingSection, setCurrentThinkingSection,
  thinkingStartTime, setThinkingStartTime,
  thinkingTimer, setThinkingTimer,
  currentAssistantContainer, setCurrentAssistantContainer,
} from "../state";
import { updateSwitcherUI } from "./view-switcher";
import { logd, loge, logi } from "../logger";

const TAG: string = "Ai";

// ==================== AI Agent 功能 ====================



// KaTeX 数学渲染

function renderLatex(tex: string, displayMode: boolean): string {

  try {

    return katex.renderToString(tex, {

      displayMode,

      throwOnError: false,

      trust: true,

    });

  } catch {

    return tex;

  }

}



// 预处理数学公式：先将 LaTeX 替换为占位符，markdown 处理后再恢复

function renderMarkdown(text: string): string {

  const mathBlocks: string[] = [];

  let placeholder = (i: number) => `%%MATH_BLOCK_${i}%%`;



  // 1. 块级公式 $$...$$ 

  let processed = text.replace(/\$\$([\s\S]*?)\$\$/g, (_, tex) => {

    const idx = mathBlocks.length;

    mathBlocks.push(renderLatex(tex.trim(), true));

    return placeholder(idx);

  });



  // 2. 块级公式 \[...\]

  processed = processed.replace(/\\\[([\s\S]*?)\\\]/g, (_, tex) => {

    const idx = mathBlocks.length;

    mathBlocks.push(renderLatex(tex.trim(), true));

    return placeholder(idx);

  });



  // 3. 行内公式 \(...\)

  processed = processed.replace(/\\\(([\s\S]*?)\\\)/g, (_, tex) => {

    const idx = mathBlocks.length;

    mathBlocks.push(renderLatex(tex.trim(), false));

    return placeholder(idx);

  });



  // 4. 行内公式 $...$（避免匹配货币符号如 $5）

  processed = processed.replace(/(?<!\$)\$(?!\$)([^\n$]+?)\$(?!\$)/g, (_, tex) => {

    const idx = mathBlocks.length;

    mathBlocks.push(renderLatex(tex.trim(), false));

    return placeholder(idx);

  });



  // 5. Markdown 渲染

  try {

    let html = marked.parse(processed, { async: false }) as string;

    // 6. 恢复数学公式

    mathBlocks.forEach((rendered, i) => {

      html = html.replace(placeholder(i), rendered);

    });

    return html;

  } catch {

    return text.replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/\n/g, "<br>");

  }

}



marked.setOptions({

  gfm: true,

  breaks: true,

});



// 高亮代码块并添加复制按钮

function highlightAndAddCopyButtons(container: HTMLElement) {

  container.querySelectorAll("pre code").forEach((block) => {

    // 高亮

    try {

      hljs.highlightElement(block as HTMLElement);

    } catch { /* ignore */ }



    // 复制按钮（避免重复添加）

    const pre = block.parentElement;

    if (pre && !pre.querySelector(".code-copy-btn")) {

      const btn = document.createElement("button");

      btn.className = "code-copy-btn";

      btn.textContent = "复制";

      btn.addEventListener("click", (e) => {

        e.stopPropagation();

        const code = block.textContent || "";

        navigator.clipboard.writeText(code).then(() => {

          btn.textContent = "✓ 已复制";

          btn.classList.add("copied");

          setTimeout(() => {

            btn.textContent = "复制";

            btn.classList.remove("copied");

          }, 1500);

        });

      });

      pre.style.position = "relative";

      pre.appendChild(btn);

    }

  });

}



// 滚动消息到底部

function scrollMessagesToBottom() {

  agentMessages.scrollTop = agentMessages.scrollHeight;

}



// 添加用户消息

function addUserMessage(content: string) {

  const messageDiv = document.createElement("div");

  messageDiv.className = "agent-message user";



  const contentDiv = document.createElement("div");

  contentDiv.className = "message-content";

  contentDiv.textContent = content;



  messageDiv.appendChild(contentDiv);

  agentMessages.appendChild(messageDiv);

  scrollMessagesToBottom();

}



// 添加助手消息容器（不创建 content div，用于思考阶段）

function addAssistantContainer() {

  const messageDiv = document.createElement("div");

  messageDiv.className = "agent-message assistant";

  agentMessages.appendChild(messageDiv);

  scrollMessagesToBottom();

  return messageDiv;

}



// 确保助手消息容器中有 content div，没有则创建

function ensureAssistantContentDiv(container: HTMLDivElement): HTMLDivElement {

  let contentDiv = container.querySelector(".message-content") as HTMLDivElement | null;

  if (!contentDiv) {

    contentDiv = document.createElement("div");

    contentDiv.className = "message-content token-fade";

    contentDiv.textContent = "";

    container.appendChild(contentDiv);

  }

  return contentDiv;

}



// 停止思考计时器

function stopThinkingTimer() {

  if (thinkingTimer !== null) {

    clearInterval(thinkingTimer);

    setThinkingTimer(null);

  }

}



// 添加思考区域

function addThinkingSection(parentMessage: HTMLDivElement) {

  const thinkingDiv = document.createElement("div");

  thinkingDiv.className = "thinking-section active";



  const headerDiv = document.createElement("div");

  headerDiv.className = "thinking-header";



  const labelSpan = document.createElement("span");

  labelSpan.className = "thinking-label";

  labelSpan.textContent = "思考中...";



  const timeSpan = document.createElement("span");

  timeSpan.className = "thinking-time";

  timeSpan.textContent = "0.0s";



  const toggleSpan = document.createElement("span");

  toggleSpan.className = "thinking-toggle";

  toggleSpan.textContent = "▼";



  headerDiv.appendChild(labelSpan);

  headerDiv.appendChild(timeSpan);

  headerDiv.appendChild(toggleSpan);



  const contentDiv = document.createElement("div");

  contentDiv.className = "thinking-content";

  contentDiv.textContent = "";



  thinkingDiv.appendChild(headerDiv);

  thinkingDiv.appendChild(contentDiv);

  // 插入到消息内容之前，确保思考区域在回复上方

  const messageContent = parentMessage.querySelector(".message-content");

  if (messageContent) {

    parentMessage.insertBefore(thinkingDiv, messageContent);

  } else {

    parentMessage.appendChild(thinkingDiv);

  }



  // 点击展开/折叠

  thinkingDiv.addEventListener("click", () => {

    thinkingDiv.classList.toggle("expanded");

    const toggle = thinkingDiv.querySelector(".thinking-toggle");

    if (toggle) {

      toggle.textContent = thinkingDiv.classList.contains("expanded") ? "▲" : "▼";

    }

  });



  setCurrentThinkingSection(contentDiv);

  setThinkingStartTime(Date.now());



  // 启动实时计时器

  stopThinkingTimer();

  setThinkingTimer(window.setInterval(() => {

    if (thinkingStartTime > 0) {

      const elapsed = ((Date.now() - thinkingStartTime) / 1000).toFixed(1);

      timeSpan.textContent = `${elapsed}s`;

    }

  }, 100));



  return contentDiv;

}



// 更新七彩流光状态

function updateAgentBorderState(state: "idle" | "thinking" | "generating" | "error") {

  capsule.classList.remove("agent-idle", "agent-thinking", "agent-generating", "agent-error");

  if (state !== "idle") {

    capsule.classList.add(`agent-${state}`);

  }

}



// 更新状态标签

function updateAgentStatus(status: string, isError = false) {

  agentStatusLabel.textContent = status;

  agentStatusLabel.className = "agent-status-label";

  if (isError) {

    agentStatusLabel.classList.add("error");

  } else if (status === "思考中...") {

    agentStatusLabel.classList.add("thinking");

  } else if (status === "生成中...") {

    agentStatusLabel.classList.add("generating");

  }

}



// 发送消息

async function sendMessage() {

  const content = agentInput.value.trim();

  if (!content || aiGenerating) return;



  // 立即标记生成状态，防止快速双击重复发送

  setAiGenerating(true);



  agentInput.value = "";



  addUserMessage(content);



  agentSendBtn.style.display = "none";

  agentStopBtn.style.display = "flex";



  try {

    await invoke("ai_send_message", { content });

  } catch (error) {

    loge(TAG, "发送消息失败:", error);

    // 在消息区域显示错误

    const errDiv = document.createElement("div");

    errDiv.className = "agent-message assistant";

    const errContent = document.createElement("div");

    errContent.className = "message-content";

    errContent.style.color = "#ff6b6b";

    errContent.textContent = `错误: ${error}`;

    errDiv.appendChild(errContent);

    agentMessages.appendChild(errDiv);

    scrollMessagesToBottom();



    updateAgentStatus("发送失败", true);

    updateAgentBorderState("error");

    agentSendBtn.style.display = "flex";

    agentStopBtn.style.display = "none";

    setAiGenerating(false);

  }

}



// 停止生成

async function stopGeneration() {

  await invoke("ai_stop_generation");

  agentSendBtn.style.display = "flex";

  agentStopBtn.style.display = "none";

  setAiGenerating(false);

  updateAgentStatus("已停止");

  updateAgentBorderState("idle");

}



// 清空历史

function showClearConfirm() {

  agentConfirmDialog.classList.add("visible");

  agentConfirmDialog.style.display = "flex";

}



function hideClearConfirm() {

  agentConfirmDialog.classList.remove("visible");

  agentConfirmDialog.style.display = "none";

}



async function clearHistory() {

  await invoke("ai_clear_history");

  agentMessages.innerHTML = "";

  setCurrentAssistantMessage(null);

  setCurrentThinkingSection(null);

  hideClearConfirm();

}



export function initAgent() {

  agentConfirmCancel.addEventListener("click", (e) => {

    e.stopPropagation();

    hideClearConfirm();

  });



  agentConfirmOk.addEventListener("click", (e) => {

    e.stopPropagation();

    void clearHistory();

  });



  // 监听 AI 事件

  listen<{ text: string }>("ai-token", (event) => {

    // 跳过空 token，避免思考阶段提前创建正文气泡

    if (!event.payload.text) return;



    // 确保有容器

    if (!currentAssistantContainer) {

      setCurrentAssistantContainer(addAssistantContainer());

    }

    // 确保有 content div（思考结束后首次创建）

    if (!currentAssistantMessage) {

      setCurrentAssistantMessage(ensureAssistantContentDiv(currentAssistantContainer!));

      setCurrentAssistantRawText("");

    }



    setCurrentAssistantRawText(currentAssistantRawText + event.payload.text);

    currentAssistantMessage!.innerHTML = renderMarkdown(currentAssistantRawText);

    highlightAndAddCopyButtons(currentAssistantMessage!);

    scrollMessagesToBottom();

  });



  listen<{ text: string }>("ai-thinking-token", (event) => {

    if (!currentThinkingSection) {

      // 只创建容器，不创建 content div

      if (!currentAssistantContainer) {

        setCurrentAssistantContainer(addAssistantContainer());

      }

      addThinkingSection(currentAssistantContainer!);

    }



    if (currentThinkingSection) {

      currentThinkingSection.textContent += event.payload.text;

    }

  });



  listen<{ status: string; error?: string }>("ai-status", (event) => {

    const { status, error } = event.payload;



    if (status === "thinking") {

      updateAgentStatus("思考中...");

      updateAgentBorderState("thinking");

    } else if (status === "generating") {

      updateAgentStatus("生成中...");

      updateAgentBorderState("generating");



      // 停止计时器，更新思考完成时间

      stopThinkingTimer();

      if (currentThinkingSection && thinkingStartTime > 0) {

        const thinkingTime = ((Date.now() - thinkingStartTime) / 1000).toFixed(1);

        const thinkingSection = currentThinkingSection.parentElement;

        if (thinkingSection) {

          thinkingSection.classList.remove("active");

          const labelSpan = thinkingSection.querySelector(".thinking-label");

          const timeSpan = thinkingSection.querySelector(".thinking-time");

          if (labelSpan) labelSpan.textContent = "思考完成";

          if (timeSpan) timeSpan.textContent = `${thinkingTime}s`;

        }

      }

    } else if (status === "completed") {

      updateAgentStatus("就绪");

      updateAgentBorderState("idle");

      stopThinkingTimer();

      agentSendBtn.style.display = "flex";

      agentStopBtn.style.display = "none";

      setAiGenerating(false);

      setCurrentAssistantMessage(null);

      setCurrentAssistantRawText("");

      setCurrentThinkingSection(null);

      setCurrentAssistantContainer(null);

      setThinkingStartTime(0);

    } else if (status === "error") {

      updateAgentStatus(error || "错误", true);

      updateAgentBorderState("error");

      stopThinkingTimer();

      agentSendBtn.style.display = "flex";

      agentStopBtn.style.display = "none";

      setAiGenerating(false);



      // 在消息区域显示错误

      if (error) {

        const errDiv = document.createElement("div");

        errDiv.className = "agent-message assistant";

        const errContent = document.createElement("div");

        errContent.className = "message-content";

        errContent.style.color = "#ff6b6b";

        errContent.style.fontSize = "12px";

        errContent.textContent = `⚠ ${error}`;

        errDiv.appendChild(errContent);

        agentMessages.appendChild(errDiv);

        scrollMessagesToBottom();

      }



      setCurrentAssistantMessage(null);

      setCurrentThinkingSection(null);

    }

  });



  // 输入框事件

  agentInput.addEventListener("keydown", (e) => {

    if (e.key === "Enter" && !e.shiftKey) {

      e.preventDefault();

      void sendMessage();

    }

  });



  agentSendBtn.addEventListener("click", () => {

    void sendMessage();

  });



  agentStopBtn.addEventListener("click", () => {

    void stopGeneration();

  });



  agentClearBtn.addEventListener("click", () => {

    showClearConfirm();

  });



  // 初始化 AI 配置

  invoke<{ api_url: string; model: string }>("ai_get_settings").then((settings) => {

    logd(TAG, "AI settings loaded:", settings);

    setAiEnabled(!!(settings.api_url && settings.model));

    logi(TAG, "AI enabled:", aiEnabled);

    if (aiEnabled) {

      agentModelName.textContent = settings.model;

      updateAgentBorderState("idle");

      updateAgentStatus("就绪");

      updateSwitcherUI();

    }

  }).catch((error) => {

    loge(TAG, "加载 AI 设置失败:", error);

  });



  // 监听 AI 设置变更

  listen("ai-settings-changed", () => {

    void invoke<{ api_url: string; model: string }>("ai_get_settings").then((settings) => {

      const wasEnabled = aiEnabled;

      setAiEnabled(!!(settings.api_url && settings.model));



      if (aiEnabled) {

        agentModelName.textContent = settings.model;

        if (!wasEnabled) {

          updateAgentBorderState("idle");

          updateAgentStatus("就绪");

        }

      } else {

        capsule.classList.remove("agent-active", "agent-idle", "agent-thinking", "agent-generating", "agent-error");

      }



      updateSwitcherUI();

    });

  });

}
