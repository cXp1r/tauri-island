import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import {
  lyricTextInner, lyricMeta,
  mpLyricText,
  vinylCover,
  musicPanelCoverImg, musicPanelSong, musicPanelArtist,
  progressBar, progressFill, progressThumb,
  mpProgressBar, mpProgressFill, mpProgressThumb,
  mpTimeCurrent, mpTimeTotal,
  mpPrev, mpPlay, mpNext,
  mpVolumeBar, mpVolumeFill, mpVolumeThumb,
  btnPrev, btnPlay, btnNext,
} from "../dom";
import {
  setIsMusicPlaying,
  setIsPlaying,
  setCurrentSongTitle, setCurrentArtistName,
  setCurrentThumbnailUrl,
  currentDurationMs, setCurrentDurationMs,
  isSeeking, setIsSeeking,
  isMpSeeking, setIsMpSeeking,
  isMpVolSeeking, setIsMpVolSeeking,
  isSeekable, setIsSeekable,
  volThrottleTimer, setVolThrottleTimer,
} from "../state";
import { formatTime } from "../utils";
import { updatePlayIcon, updateSwitcherUI } from "./view-switcher";
import { resetMpLyricFlipState } from "./lyric-renderer";
import { logw, logi } from "../logger";

const TAG: string = "MusicController";
// ===== 面板音量滑块 =====

// 展开面板时获取当前音量
export function fetchAndUpdateVolume() {
  invoke<number>("media_get_volume").then((vol) => {
    const pct = Math.min(100, Math.max(0, vol * 100));
    mpVolumeFill.style.width = `${pct}%`;
    mpVolumeThumb.style.left = `${pct}%`;
  }).catch(() => {});
}

function updateMpVolumeFromMouse(e: MouseEvent) {
  const rect = mpVolumeBar.getBoundingClientRect();
  const pct = Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width));
  mpVolumeFill.style.width = `${pct * 100}%`;
  mpVolumeThumb.style.left = `${pct * 100}%`;
  return pct;
}

// ===== 进度条拖动（Seek）=====

export function updateSeekable(seekable: boolean) {
  if (isSeekable === seekable) return;
  setIsSeekable(seekable);
  progressBar.classList.toggle("no-seek", !seekable);
  mpProgressBar.classList.toggle("no-seek", !seekable);
}

function updateProgressFromMouse(e: MouseEvent) {
  const rect = progressBar.getBoundingClientRect();
  const pct = Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width));
  progressFill.style.width = `${pct * 100}%`;
  progressThumb.style.left = `${pct * 100}%`;
  return pct;
}

function updateMpProgressFromMouse(e: MouseEvent) {
  const rect = mpProgressBar.getBoundingClientRect();
  const pct = Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width));
  mpProgressFill.style.width = `${pct * 100}%`;
  mpProgressThumb.style.left = `${pct * 100}%`;
  mpTimeCurrent.textContent = formatTime(pct * currentDurationMs);
  return pct;
}

export function initMusicControls() {

  // ===== Tauri 事件监听 =====

  listen<boolean>("playback-state", (event) => {
    setIsPlaying(event.payload);
    updatePlayIcon();
  });

  listen<{ title: string; artist: string; genre?: string; thumbnail?: string | null; duration_ms?: number; seekable?: boolean }>("media-changed", (event) => {
    setIsMusicPlaying(true);
    setCurrentSongTitle(event.payload.title);
    setCurrentArtistName(event.payload.artist);
    logi("SMTC", `genre='${event.payload.genre ?? ""}' title='${event.payload.title}' artist='${event.payload.artist}'`);
    lyricTextInner.textContent = "♪";
    lyricMeta.textContent = `${event.payload.artist} - ${event.payload.title}`;
    mpLyricText.textContent = "♪";
    resetMpLyricFlipState();
    lyricMeta.style.fontSize = "";
    lyricMeta.style.color = "";

    // 同步面板信息
    musicPanelSong.textContent = event.payload.title;
    musicPanelArtist.textContent = event.payload.artist;

    // 更新封面
    if (event.payload.thumbnail) {
      setCurrentThumbnailUrl(event.payload.thumbnail);
      vinylCover.style.backgroundImage = `url(${event.payload.thumbnail})`;
      musicPanelCoverImg.style.backgroundImage = `url(${event.payload.thumbnail})`;
    } else {
      setCurrentThumbnailUrl("");
      vinylCover.style.backgroundImage = "";
      musicPanelCoverImg.style.backgroundImage = "";
    }

    // 更新时长
    if (event.payload.duration_ms) {
      setCurrentDurationMs(event.payload.duration_ms);
      mpTimeTotal.textContent = formatTime(event.payload.duration_ms);
    } else {
      setCurrentDurationMs(0);
      mpTimeTotal.textContent = "0:00";
    }

    // 更新 seekable 状态
    updateSeekable(event.payload.seekable ?? true);

    // 重置进度条
    progressFill.style.width = "0%";
    progressThumb.style.left = "0%";
    mpProgressFill.style.width = "0%";
    mpProgressThumb.style.left = "0%";
    mpTimeCurrent.textContent = "0:00";

    updateSwitcherUI();
  });

  listen<{ title: string; artist: string }>("media-paused", () => {
    setIsMusicPlaying(true);
  });

  // 异步封面加载完成
  listen<{ thumbnail: string }>("media-thumbnail", (event) => {
    if (event.payload.thumbnail) {
      setCurrentThumbnailUrl(event.payload.thumbnail);
      vinylCover.style.backgroundImage = `url(${event.payload.thumbnail})`;
      musicPanelCoverImg.style.backgroundImage = `url(${event.payload.thumbnail})`;
    }
  });

  // ===== 收起态播放控制 =====

  btnPrev.addEventListener("click", (e) => {
    e.stopPropagation();
    void invoke("media_prev");
  });

  btnPlay.addEventListener("click", (e) => {
    e.stopPropagation();
    void invoke("media_play_pause");
  });

  btnNext.addEventListener("click", (e) => {
    e.stopPropagation();
    void invoke("media_next");
  });

  // ===== 面板播放控制 =====

  mpPrev.addEventListener("click", (e) => {
    e.stopPropagation();
    void invoke("media_prev");
  });

  mpPlay.addEventListener("click", (e) => {
    e.stopPropagation();
    void invoke("media_play_pause");
  });

  mpNext.addEventListener("click", (e) => {
    e.stopPropagation();
    void invoke("media_next");
  });

  // ===== 面板音量滑块事件 =====

  mpVolumeBar.addEventListener("mousedown", (e: MouseEvent) => {
    e.stopPropagation();
    setIsMpVolSeeking(true);
    mpVolumeBar.classList.add("seeking");
    const pct = updateMpVolumeFromMouse(e);
    void invoke("media_set_volume", { volume: pct }).catch(() => {});
  });

  document.addEventListener("mousemove", (e: MouseEvent) => {
    if (!isMpVolSeeking) return;
    const pct = updateMpVolumeFromMouse(e);
    // 节流：拖动时每50ms更新一次音量
    if (!volThrottleTimer) {
      setVolThrottleTimer(window.setTimeout(() => {
        setVolThrottleTimer(null);
      }, 50));
      void invoke("media_set_volume", { volume: pct }).catch(() => {});
    }
  });

  document.addEventListener("mouseup", (e: MouseEvent) => {
    if (!isMpVolSeeking) return;
    setIsMpVolSeeking(false);
    mpVolumeBar.classList.remove("seeking");
    const pct = updateMpVolumeFromMouse(e);
    void invoke("media_set_volume", { volume: pct }).catch((err: unknown) => {
      logw(TAG, "Set volume failed:", err);
    });
  });

  // ===== 进度条拖动（收起态） =====

  progressBar.addEventListener("mousedown", (e: MouseEvent) => {
    if (currentDurationMs <= 0 || !isSeekable) return;
    e.stopPropagation();
    setIsSeeking(true);
    progressBar.classList.add("seeking");
    updateProgressFromMouse(e);
  });

  document.addEventListener("mousemove", (e: MouseEvent) => {
    if (!isSeeking) return;
    updateProgressFromMouse(e);
  });

  document.addEventListener("mouseup", (e: MouseEvent) => {
    if (!isSeeking) return;
    setIsSeeking(false);
    progressBar.classList.remove("seeking");
    const pct = updateProgressFromMouse(e);
    const seekMs = Math.round(pct * currentDurationMs);
    void invoke("media_seek", { positionMs: seekMs }).catch((err: unknown) => {
      logw(TAG, "Seek failed:", err);
    });
  });

  // ===== 面板进度条拖动（Seek）=====

  mpProgressBar.addEventListener("mousedown", (e: MouseEvent) => {
    if (currentDurationMs <= 0 || !isSeekable) return;
    e.stopPropagation();
    setIsMpSeeking(true);
    mpProgressBar.classList.add("seeking");
    updateMpProgressFromMouse(e);
  });

  document.addEventListener("mousemove", (e: MouseEvent) => {
    if (!isMpSeeking) return;
    updateMpProgressFromMouse(e);
  });

  document.addEventListener("mouseup", (e: MouseEvent) => {
    if (!isMpSeeking) return;
    setIsMpSeeking(false);
    mpProgressBar.classList.remove("seeking");
    const pct = updateMpProgressFromMouse(e);
    const seekMs = Math.round(pct * currentDurationMs);
    void invoke("media_seek", { positionMs: seekMs }).catch((err: unknown) => {
      logw(TAG, "Seek failed:", err);
    });
  });

}
