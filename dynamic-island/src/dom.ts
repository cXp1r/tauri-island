import type { ViewMode } from "./types";

export const capsule = document.getElementById("island-capsule") as HTMLDivElement;
export const currentViewContainer = document.getElementById("current-view") as HTMLDivElement;
export const viewHolder = document.getElementById("view-holder") as HTMLDivElement;


export const timeWrapper = document.getElementById("time-wrapper") as HTMLDivElement;

export const timeText = document.getElementById("time-text") as HTMLDivElement;

export const dateText = document.getElementById("date-text") as HTMLDivElement;

export const weatherText = document.getElementById("weather-text") as HTMLDivElement;



export const noticeArea = document.getElementById("notice-area") as HTMLDivElement;

export const noticeMsg = document.getElementById("notice-msg") as HTMLDivElement;

export const urlList = document.getElementById("url-list") as HTMLDivElement;

export const lyricArea = document.getElementById("lyric-area") as HTMLDivElement;



export const lyricText = document.getElementById("lyric-text") as HTMLDivElement;
export const lyricTextInner = document.getElementById("lyric-text-inner") as HTMLSpanElement;

export const lyricMeta = document.getElementById("lyric-meta") as HTMLDivElement;

export const vinylDisc = document.getElementById("vinyl-disc") as HTMLDivElement;

export const vinylCover = document.getElementById("vinyl-cover") as HTMLDivElement;


export const progressBar = document.getElementById("progress-bar") as HTMLDivElement;

export const progressFill = document.getElementById("progress-fill") as HTMLDivElement;

export const progressThumb = document.getElementById("progress-thumb") as HTMLDivElement;



// 音乐展开面板

export const musicPanelCoverImg = document.getElementById("music-panel-cover-img") as HTMLDivElement;

export const musicPanelSong = document.getElementById("music-panel-song") as HTMLDivElement;

export const musicPanelArtist = document.getElementById("music-panel-artist") as HTMLDivElement;

export const mpProgressBar = document.getElementById("mp-progress-bar") as HTMLDivElement;

export const mpProgressFill = document.getElementById("mp-progress-fill") as HTMLDivElement;

export const mpProgressThumb = document.getElementById("mp-progress-thumb") as HTMLDivElement;

export const mpTimeCurrent = document.getElementById("mp-time-current") as HTMLSpanElement;

export const mpTimeTotal = document.getElementById("mp-time-total") as HTMLSpanElement;

export const mpPrev = document.getElementById("mp-prev") as HTMLButtonElement;

export const mpPlay = document.getElementById("mp-play") as HTMLButtonElement;

export const mpNext = document.getElementById("mp-next") as HTMLButtonElement;

export const mpIconPlay = mpPlay.querySelector(".mp-icon-play") as SVGElement;

export const mpIconPause = mpPlay.querySelector(".mp-icon-pause") as SVGElement;

export const mpVolumeBar = document.getElementById("mp-volume-bar") as HTMLDivElement;

export const mpVolumeFill = document.getElementById("mp-volume-fill") as HTMLDivElement;

export const mpVolumeThumb = document.getElementById("mp-volume-thumb") as HTMLDivElement;

export const mpLyricText = document.getElementById("mp-lyric-text") as HTMLDivElement;



export const agentArea = document.getElementById("agent-area") as HTMLDivElement;

export const agentMessages = document.getElementById("agent-messages") as HTMLDivElement;

export const agentInput = document.getElementById("agent-input") as HTMLInputElement;

export const agentSendBtn = document.getElementById("agent-send-btn") as HTMLButtonElement;

export const agentStopBtn = document.getElementById("agent-stop-btn") as HTMLButtonElement;

export const agentModelName = document.getElementById("agent-model-name") as HTMLDivElement;

export const agentStatusLabel = document.getElementById("agent-status-label") as HTMLDivElement;

export const agentClearBtn = document.getElementById("agent-clear-btn") as HTMLButtonElement;

export const agentConfirmDialog = document.getElementById("agent-confirm-dialog") as HTMLDivElement;

export const agentConfirmCancel = document.getElementById("agent-confirm-cancel") as HTMLButtonElement;

export const agentConfirmOk = document.getElementById("agent-confirm-ok") as HTMLButtonElement;

export const searchArea = document.getElementById("search-area") as HTMLDivElement;
export const searchInput = document.getElementById("search-input") as HTMLInputElement;
export const searchResults = document.getElementById("search-results") as HTMLDivElement;



export const btnPrev = document.getElementById("btn-prev") as HTMLButtonElement;

export const btnPlay = document.getElementById("btn-play") as HTMLButtonElement;

export const btnNext = document.getElementById("btn-next") as HTMLButtonElement;

export const iconPlay = document.getElementById("icon-play") as HTMLElement;

export const iconPause = document.getElementById("icon-pause") as HTMLElement;



export const viewSwitcher = document.getElementById("view-switcher") as HTMLDivElement;

export const viewDots = document.getElementById("view-dots") as HTMLDivElement;

export const privacyIndicators = document.getElementById("privacy-indicators") as HTMLDivElement;

export const privacyMic = document.getElementById("privacy-mic") as HTMLDivElement;

export const privacyCamera = document.getElementById("privacy-camera") as HTMLDivElement;

export const collapsedIndicator = document.getElementById("collapsed-indicator") as HTMLDivElement;

export const viewElements: Record<ViewMode, HTMLElement> = {
  time: timeWrapper,
  lyric: lyricArea,
  agent: agentArea,
  search: searchArea,
};
