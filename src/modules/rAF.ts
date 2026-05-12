import { cAFTimer, setcAFTimer } from "../state";
import { invoke } from "@tauri-apps/api/core";

const el = document.getElementById("island-capsule");
let animate_running = false;
function loop() {
      if(!el){
        return;
      }
      const rect = el.getBoundingClientRect();
      void invoke('resize_raf', {
        height: rect.height + 10,
        width: rect.width,
        reposition: true,
      });
      rafId = requestAnimationFrame(loop);
    }

export function initrAF(){

  if(el){
    
    el.addEventListener("transitionstart", (e) => {
      if (animate_running) return;
      animate_running = true;
      console.log("transition start:", e.propertyName);
      start_rAF();
    });
    el.addEventListener("transitionend", (e) => {
        if(e.propertyName == "width" || e.propertyName == "height"){
            stop_rAF(() => {
                animate_running = false;
                const hasExpanded = [...el.classList].some(cls =>
                    cls.endsWith("expanded")
                );
                if(!hasExpanded){
                    void invoke('snap_back_fast');
                } 
            })
        }
      
    });
  }
}

export function start_rAF(){
    rafId = requestAnimationFrame(loop);
}

let rafId: number | null = null;

export function stop_rAF(callback?: () => void) {
    if (cAFTimer !== null) {
        clearTimeout(cAFTimer);
        setcAFTimer(null);
    }

    setcAFTimer(window.setTimeout(() => {
        if (rafId !== null) {
            cancelAnimationFrame(rafId);
            rafId = null;
        }

        console.log("transition end");

        callback?.();
    }, 120));
}


