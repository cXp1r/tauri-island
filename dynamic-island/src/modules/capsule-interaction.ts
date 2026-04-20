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
  isExpandAnimating, setIsExpandAnimating,
  setSkipResizeSync,
  currentSongTitle, currentArtistName, currentThumbnailUrl,
} from "../state";
import { switchToNextView } from "./view-switcher";
import { fetchAndUpdateVolume } from "./music-controls";
import { showContextMenu } from "./minimize-drag";

export function initCapsuleInteraction() {

  capsule.addEventListener("click", (e: MouseEvent) => {

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

          void invoke("set_music_expanded", { expanded: true, width: 380, height: 420 + bodyPad + 5 });

          window.setTimeout(() => { setSkipResizeSync(false); setIsExpandAnimating(false); }, 400);

        } else {

          setSkipResizeSync(true);

          capsule.classList.remove("music-expanded");

          void invoke("set_music_expanded", { expanded: false, width: 380, height: 420 });

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

        const willExpand = !capsule.classList.contains("agent-expanded");

        if (willExpand) {

          setSkipResizeSync(true);

          capsule.classList.add("agent-expanded");

          void invoke("set_agent_expanded", { expanded: true });

          window.setTimeout(() => { setSkipResizeSync(false); setIsExpandAnimating(false); }, 400);

        } else {

          setSkipResizeSync(true);

          const agentArea = document.getElementById("agent-area");

          if (agentArea) agentArea.classList.add("collapsing");

          window.setTimeout(() => {

            capsule.classList.remove("agent-expanded");

            void invoke("set_agent_expanded", { expanded: false });

            window.setTimeout(() => {

              if (agentArea) agentArea.classList.remove("collapsing");

              setSkipResizeSync(false);

              setIsExpandAnimating(false);

            }, 500);

          }, 100);

        }

      }, 250));

      return;

    }

  });



  capsule.addEventListener("dblclick", (e: MouseEvent) => {

    const target = e.target as HTMLElement;

    if (target.closest(".url-item") || target.closest("#notice-area") || target.closest(".media-btn") || target.closest(".view-dot") || target.closest("#agent-input") || target.closest("#agent-send-btn") || target.closest("#agent-stop-btn") || target.closest("#agent-clear-btn")) {

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
