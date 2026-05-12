import { invoke } from "@tauri-apps/api/core";
import {
  capsule,
  vinylCover,
  musicPanelCoverImg, musicPanelSong, musicPanelArtist,
} from "../dom";
import {
  currentView,
  dragStarted, setDragStarted,
  musicClickTimer, setMusicClickTimer,
  agentClickTimer, setAgentClickTimer,
  sadbClickTimer, setSadbClickTimer,
  isExpandAnimating, setIsExpandAnimating,
  setSkipResizeSync,
  currentSongTitle, currentArtistName, currentThumbnailUrl,
  emailClickTimer,
  setEmailClickTimer,
} from "../state";
import { switchToNextView } from "./view-switcher";
import { fetchAndUpdateVolume } from "./music-controls";
import { showContextMenu } from "./minimize-drag";
import { logd, logi } from "../logger";

export function initCapsuleInteraction() {
  capsule.addEventListener("click", (e: MouseEvent) => {
    logd("Capsule",`click on view '${currentView}'`);
    const target = e.target as HTMLElement;
    
    // 如果刚刚发生了拖动，不触发点击
    if (dragStarted) {
      setDragStarted(false);
      return;
    }
    // 音乐视图：单击展开/收起
    if (currentView === "lyric") {
      // 展开态：点击头部收起，排除交互元素
      if (capsule.classList.contains("music-expanded")) {
        if (!target.closest("#music-panel-header")) {
          return;
        }
      } else {
        // 收起态：排除播放控制
        if (target.closest(".media-btn") || target.closest(".progress-bar") || target.closest(".vol-btn")) {
          return;
        }
      }
      e.stopPropagation();
      if (musicClickTimer) {
        clearTimeout(musicClickTimer);
        setMusicClickTimer(null);
        return;
      }
      setMusicClickTimer(window.setTimeout(() => {
        setMusicClickTimer(null);
        if (isExpandAnimating) return;
        setIsExpandAnimating(true);
        const willExpand = !capsule.classList.contains("music-expanded");
        if (willExpand) {
          setSkipResizeSync(true);
          capsule.classList.add("music-expanded");
          musicPanelSong.textContent = currentSongTitle || "";
          musicPanelArtist.textContent = currentArtistName || "";
          if (currentThumbnailUrl) {
            vinylCover.style.backgroundImage = `url(${currentThumbnailUrl})`;
            musicPanelCoverImg.style.backgroundImage = `url(${currentThumbnailUrl})`;
          }
          fetchAndUpdateVolume();
          const bodyPad = parseFloat(getComputedStyle(document.body).paddingTop) || 0;
          void invoke("set_expanded", { expanded: true, width: 0, height: 420 + bodyPad + 5 });
          window.setTimeout(() => { setSkipResizeSync(false); setIsExpandAnimating(false); }, 400);
        } else {
          setSkipResizeSync(true);
          capsule.classList.remove("music-expanded");
          void invoke("set_expanded", { expanded: false, width: 0, height: 420 });
          window.setTimeout(() => { setSkipResizeSync(false); setIsExpandAnimating(false); }, 500);
        }
      }, 250));
      return;
    }
    // Agent 视图特殊处理：单击展开/收起，但需要等待排除双击
    if (currentView === "agent") {
      // 展开态：只有点击状态栏才收起，但排除清空按钮
      if (capsule.classList.contains("agent-expanded")) {
        if (!target.closest("#agent-status-bar") || target.closest("#agent-clear-btn")) {
          return;
        }
      } else {
        // 收起态：排除交互元素，其他区域点击展开
        if (target.closest("#agent-input") || target.closest("#agent-send-btn") || target.closest("#agent-stop-btn") || target.closest("#agent-clear-btn") || target.closest(".thinking-section") || target.closest("#agent-messages") || target.closest("#agent-confirm-dialog")) {
          return;
        }
      }
      e.stopPropagation();
      // 延迟执行，等待可能的双击
      if (agentClickTimer) {
        clearTimeout(agentClickTimer);
        setAgentClickTimer(null);
        return; // 双击的第二次 click，忽略
      }
      setAgentClickTimer(window.setTimeout(() => {
        setAgentClickTimer(null);
        if (isExpandAnimating) return;
        setIsExpandAnimating(true);
        if (!capsule.classList.contains("agent-expanded")) {
          setSkipResizeSync(true);
          capsule.classList.add("agent-expanded");
          void invoke("set_expanded", { expanded: true, width: 520, height: 550 });
          window.setTimeout(() => { setSkipResizeSync(false); setIsExpandAnimating(false); }, 400);
        } else {
          setSkipResizeSync(true);
          const agentArea = document.getElementById("agent-area");
          if (agentArea) agentArea.classList.add("collapsing");
          window.setTimeout(() => {
            capsule.classList.remove("agent-expanded");
            void invoke("set_expanded", { expanded: false, width: 0, height: 640 });
            window.setTimeout(() => {
              if (agentArea) agentArea.classList.remove("collapsing");
              setSkipResizeSync(false);
              setIsExpandAnimating(false);
            }, 50);
          }, 100);
        }
      }, 250));
      return;
    }
    // sadb 视图三态：胶囊 → 待机面板(idle) → 镜像(expanded)
    if (currentView === "sadb") {
      // 镜像中：所有操作由面板内按钮（Stop）处理，胶囊层不响应点击
      if (capsule.classList.contains("sadb-expanded")) return;
      // 待机面板：点击状态栏才收起回胶囊
      if (capsule.classList.contains("sadb-idle")) {
        if (!target.closest("#sadb-status-bar")) return;
        e.stopPropagation();
        if (sadbClickTimer) { clearTimeout(sadbClickTimer); setSadbClickTimer(null); return; }
        setSadbClickTimer(window.setTimeout(() => {
          setSadbClickTimer(null);
          if (isExpandAnimating) return;
          setIsExpandAnimating(true);
          setSkipResizeSync(true);
          capsule.classList.remove("sadb-idle");
          void invoke("set_expanded", { expanded: false ,width: 0, height: 0});
          window.setTimeout(() => { setSkipResizeSync(false); setIsExpandAnimating(false); }, 400);
        }, 250));
        return;
      }
      // 胶囊态：点击任意区域（排除按钮/canvas）→ 展开待机面板
      if (target.closest("#sadb-btn-start") || target.closest("#sadb-btn-stop") || target.closest("#sadb-canvas")) return;
      e.stopPropagation();
      if (sadbClickTimer) { clearTimeout(sadbClickTimer); setSadbClickTimer(null); return; }
      setSadbClickTimer(window.setTimeout(() => {
        setSadbClickTimer(null);
        if (isExpandAnimating) return;
        setIsExpandAnimating(true);
        setSkipResizeSync(true);
        capsule.classList.add("sadb-idle");
        void invoke("set_expanded", { expanded: true, width: 0, height: 430});
        window.setTimeout(() => { setSkipResizeSync(false); setIsExpandAnimating(false); }, 400);
      }, 250));
      return;
    }
    // email 视图
    if (currentView === "email") {
      if (emailClickTimer) {
        clearTimeout(emailClickTimer);
        setEmailClickTimer(null);
        return;
      }
      setEmailClickTimer(window.setTimeout(() => {
        if (capsule.classList.contains("email-expanded")){
          capsule.classList.remove("email-expanded");
          void invoke('set_expanded', { expanded: false, width: 0, height: 0 });
          return;
        }
        const rootStyles = getComputedStyle(document.documentElement);

        const emailW = parseFloat(rootStyles.getPropertyValue("--email-view-w").trim().replace("px", ".0"));
        const emailH = parseFloat(rootStyles.getPropertyValue("--email-view-h").trim().replace("px", ".0"));
        logi("Interaction", emailW, emailH);
        void invoke('set_expanded', { expanded: true, width: emailW, height: emailH + 10});
        capsule.classList.add("email-expanded");
      }, 250));
    }
  });
  capsule.addEventListener("dblclick", (e: MouseEvent) => {
    logd("Capsule",`double click on view '${currentView}'`);
    const target = e.target as HTMLElement;
    if (target.closest(".url-item") || target.closest("#notice-area") || target.closest(".media-btn") || target.closest(".view-dot") || target.closest("#agent-input") || target.closest("#agent-send-btn") || target.closest("#agent-stop-btn") || target.closest("#agent-clear-btn") || target.closest("#sadb-btn-start") || target.closest("#sadb-btn-stop") || target.closest("#sadb-canvas")) {
      return;
    }
    // 取消 agent 单击延时
    if (agentClickTimer) {
      clearTimeout(agentClickTimer);
      setAgentClickTimer(null);
    }
    // 取消 music 单击延时
    if (musicClickTimer) {
      clearTimeout(musicClickTimer);
      setMusicClickTimer(null);
    }
    // 取消 sadb 单击延时
    if (sadbClickTimer) {
      clearTimeout(sadbClickTimer);
      setSadbClickTimer(null);
    }

    if (emailClickTimer) {
      clearTimeout(emailClickTimer);
      setEmailClickTimer(null);
    }
    e.stopPropagation();
    switchToNextView();
  });
  // 右键菜单功能
  capsule.addEventListener("contextmenu", (e: MouseEvent) => {
    e.preventDefault();
    // Agent 展开态时不显示菜单
    if (capsule.classList.contains("agent-expanded") || capsule.classList.contains("music-expanded")) return;
    // 隐私弹窗显示时不显示菜单
    if (capsule.classList.contains("privacy-active")) return;
    // 显示系统菜单
    showContextMenu();
  });
}
