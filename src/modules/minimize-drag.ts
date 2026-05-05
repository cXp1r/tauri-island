import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import {
  capsule,
  collapsedIndicator,
  emailDragHandle,
} from "../dom";
import {
  isMinimized, setIsMinimized,
  isMinimizeAnimating, setIsMinimizeAnimating,
  currentView,
  isDragging, setIsDragging,
  dragStarted, setDragStarted,
  lastX, setLastX,
  lastY, setLastY,
  mouseDownX, setMouseDownX,
  mouseDownY, setMouseDownY,
  DRAG_THRESHOLD,
} from "../state";

// ===== 收起/展开功能 =====

export function applyIndicatorColor(color: string) {

  collapsedIndicator.style.background = `linear-gradient(90deg, ${color}dd, ${color}, ${color}dd)`;

  collapsedIndicator.style.boxShadow = `0 0 8px ${color}80`;

}



export function minimizeIsland() {

  if (isMinimized || isMinimizeAnimating) return;

  setIsMinimized(true);

  setIsMinimizeAnimating(true);



  capsule.classList.add("minimizing");



  setTimeout(() => {

    capsule.classList.remove("minimizing");

    capsule.classList.add("minimized");

    document.body.classList.add("minimized");

    void invoke("set_minimized", { minimized: true });

    setIsMinimizeAnimating(false);

  }, 300);

}



export function expandFromMinimized() {

  if (!isMinimized || isMinimizeAnimating) return;

  setIsMinimized(false);

  setIsMinimizeAnimating(true);



  void invoke("set_minimized", { minimized: false });



  document.body.classList.remove("minimized");



  capsule.classList.remove("minimized");

  capsule.classList.add("expanding");



  setTimeout(() => {

    capsule.classList.remove("expanding");

    setIsMinimizeAnimating(false);

  }, 300);

}



// ===== 右键菜单 =====

export function showContextMenu() {

  // 使用后端显示系统菜单

  void invoke("show_context_menu");

}



function beginWindowDrag(screenX: number, screenY: number) {

  setIsDragging(true);

  setDragStarted(false);

  setLastX(screenX);

  setLastY(screenY);

  setMouseDownX(screenX);

  setMouseDownY(screenY);

}



function moveWindowDrag(screenX: number, screenY: number) {

  if (!isDragging) return;

  const dx = screenX - lastX;

  const dy = screenY - lastY;

  if (!dragStarted) {

    const totalDx = Math.abs(screenX - mouseDownX);

    const totalDy = Math.abs(screenY - mouseDownY);

    if (totalDx < DRAG_THRESHOLD && totalDy < DRAG_THRESHOLD) return;

    setDragStarted(true);

    void invoke("start_drag");

  }

  setLastX(screenX);

  setLastY(screenY);

  if (dx !== 0 || dy !== 0) {

    void invoke("drag_move", { dx, dy });

  }

}



function endWindowDrag() {

  if (!isDragging) return;

  setIsDragging(false);

  if (dragStarted) {

    void invoke("end_drag");

    window.setTimeout(() => { setDragStarted(false); }, 100);

  }

}



export function initMinimizeDrag() {

  // 监听菜单动作

  listen<string>("context-menu-action", (event) => {

    const action = event.payload;

    if (action === "minimize") {

      minimizeIsland();

    } else if (action === "settings") {

      // 延迟执行，确保菜单完全关闭后再打开设置窗口

      setTimeout(() => {

        void invoke("open_settings");

      }, 100);

    }

  });


  listen<string>("indicator-color-changed", (event) => {

    applyIndicatorColor(event.payload);

  });



  capsule.addEventListener("mousedown", (e: MouseEvent) => {

    // 右键不触发拖动

    if (e.button !== 0) return;

    const target = e.target as HTMLElement;

    if (target.closest("#email-drag-handle") || target.closest(".url-item") || target.closest("#notice-area") || target.closest(".media-btn") || target.closest(".view-dot")) {

      return;

    }

    // Agent 展开态下，排除输入框和按钮，但允许拖动状态栏和消息区域

    if (currentView === "agent" && capsule.classList.contains("agent-expanded")) {

      if (target.closest("#agent-input") || target.closest("#agent-send-btn") || target.closest("#agent-stop-btn") || target.closest("#agent-clear-btn")) {

        return;

      }

    }



    beginWindowDrag(e.screenX, e.screenY);

  });



  emailDragHandle.addEventListener("pointerdown", (e: PointerEvent) => {

    if (currentView !== "email" || e.button !== 0) return;

    e.preventDefault();

    e.stopPropagation();

    emailDragHandle.setPointerCapture(e.pointerId);

    beginWindowDrag(e.screenX, e.screenY);

  });

  emailDragHandle.addEventListener("pointermove", (e: PointerEvent) => {

    if (currentView !== "email" || !emailDragHandle.hasPointerCapture(e.pointerId)) return;

    e.preventDefault();

    moveWindowDrag(e.screenX, e.screenY);

  });

  emailDragHandle.addEventListener("pointerup", (e: PointerEvent) => {

    if (emailDragHandle.hasPointerCapture(e.pointerId)) {

      emailDragHandle.releasePointerCapture(e.pointerId);

    }

    endWindowDrag();

  });

  emailDragHandle.addEventListener("pointercancel", (e: PointerEvent) => {

    if (emailDragHandle.hasPointerCapture(e.pointerId)) {

      emailDragHandle.releasePointerCapture(e.pointerId);

    }

    endWindowDrag();

  });

  emailDragHandle.addEventListener("lostpointercapture", () => {

    endWindowDrag();

  });



  // 绿条点击展开

  collapsedIndicator.addEventListener("click", (e: MouseEvent) => {

    e.stopPropagation();

    expandFromMinimized();

  });



  document.addEventListener("mousemove", (e: MouseEvent) => {

    if (!isDragging) return;

    moveWindowDrag(e.screenX, e.screenY);

  });


  document.addEventListener("mouseup", () => {

    if (!isDragging) return;

    endWindowDrag();

  });

}
