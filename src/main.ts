import { currentView } from "./state";
import { hidePrivacyPopup, initPrivacy } from "./modules/privacy";
import { initNoticeUrl } from "./modules/notice-url";
import { initWeather } from "./modules/weather";
import { initViewSwitcher, showOnlyView, syncCurrentView } from "./modules/view-switcher";
import { initLyricRenderer } from "./modules/lyric-renderer";
import { initMusicControls } from "./modules/music-controls";
import { initMinimizeDrag } from "./modules/minimize-drag";
import { initCapsuleInteraction } from "./modules/capsule-interaction";
import { initAgent } from "./modules/agent";
import { initResizeObserver } from "./modules/resize-observer";
import { initSearch } from "./modules/search";
import { initSadb } from "./modules/sadb";
import { initEmailResize } from "./modules/email-resize";
import { initEmailView } from "./modules/email-view";
import { initNoticeQueue } from "./modules/notice-queue";

initNoticeUrl();
initNoticeQueue();
initWeather();
initPrivacy();
initViewSwitcher();
initLyricRenderer();
initMusicControls();
initMinimizeDrag();
initCapsuleInteraction();
initAgent();
initSearch();
initSadb();
initEmailView();
initEmailResize();
initResizeObserver();

showOnlyView("time");
hidePrivacyPopup();
void syncCurrentView(currentView);

