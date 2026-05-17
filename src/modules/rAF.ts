import { invoke } from '@tauri-apps/api/core'

const ease = (p1x: number, p1y: number, p2x: number, p2y: number) => {
  const calcX = (t: number) => 3 * p1x * t * (1-t)**2 + 3 * p2x * t**2 * (1-t) + t**3
  const calcY = (t: number) => 3 * p1y * t * (1-t)**2 + 3 * p2y * t**2 * (1-t) + t**3
  const solveT = (x: number) => {
    let lo = 0, hi = 1, t = x
    for (let i = 0; i < 64; i++) {
      const cx = calcX(t)
      if (Math.abs(cx - x) < 1e-9) break
      cx < x ? lo = t : hi = t
      t = (lo + hi) / 2
    }
    return t
  }
  return (x: number) => x <= 0 ? 0 : x >= 1 ? 1 : calcY(solveT(x))
}

const easing = ease(0.25, 1, 0.5, 1)
const { port1, port2 } = new MessageChannel()


let raf: number
let targetW: number = 0
let targetH: number = 0
const el = document.getElementById('island-capsule')!
let rect = el.getBoundingClientRect()
let fromW = Math.round(rect.width)
let fromH = Math.round(rect.height)
port1.onmessage = ({ data }: MessageEvent<{ w: number; h: number; lw: number; t: number; e: number }>) => {
  void invoke('resize_raf', {
    width: data.w,
    height: data.h + 10,
    lwidth: data.lw,
    ewidth: targetW,   // 用模块级变量
    reposition: 1,
    d: data.t,
    e: data.e,
  })
}
export function animateCapsule(toW: number, toH: number): void {
  if (toW === targetW && toH === targetH) return
  targetW = toW
  targetH = toH
  
  cancelAnimationFrame(raf)

  // 从当前实际样式读取起点，而不是依赖外部 fromW/fromH
  const startW = parseFloat(el.style.width) || fromW
  const startH = parseFloat(el.style.height) || fromH

  const start = performance.now()
  let lw = startW
  //let lh = startH
  function frame(now: number): void {
    
    const t = Math.min((now - start) / 350, 1)
    const e = easing(t)
    const w = (Math.round(startW + (toW - startW) * e) + 1) & ~1
    const h = (Math.round(startH + (toH - startH) * e) + 1) & ~1

    el.style.width  = w + 'px'
    el.style.height = h + 'px'
console.log(w);
    port2.postMessage({ w, h, lw, t, e })

    if (t < 1) raf = requestAnimationFrame(frame)
    lw = w
  }

  raf = requestAnimationFrame(frame)
}



// 在 initrAF 里预热，页面加载时就触发一次
export function initrAF() {
  // 预热 Tauri IPC
  void invoke('resize_raf', {
    width: 0, height: 0, lwidth: 0,
    ewidth: 0, reposition: 0, d: 0, e: 0,
  })

  // 预热 MessageChannel
  port2.postMessage({ w: 0, h: 0, lw: 0, t: 0, e: 0 })
    const observer = new MutationObserver((mutations) => {
        for (const mutation of mutations) {
            if (mutation.attributeName === 'class') {
              if (!el.classList.contains("sadb-expanded")){
                let [toW, toH] = [140, 50];
                if (el.classList.value == "") {
                } else if (el.classList.contains("music-expanded")) {
                  [toW, toH] = [380, 420];
                } else if (el.classList.contains("agent-expanded")) {
                  [toW, toH] = [640, 620];
                } else if (el.classList.contains("sadb-idle")) {
                  [toW, toH] = [380, 420];
                } else if (el.classList.contains("email-expanded")) {
                  const style = getComputedStyle(document.documentElement);
                  [toW, toH] = [parseInt(style.getPropertyValue('--email-view-w')), parseInt(style.getPropertyValue('--email-view-h')),];
                } else if (el.classList.contains("expanded")) {
                  [toW, toH] = [330, 74];
                } else if (el.classList.contains("lyric-collapsed")) {
                  [toW, toH] = [340, 50];
                }
                animateCapsule(toW, toH);
              }
            }
        }
        })
    observer.observe(el, {
        attributes: true,
        attributeFilter: ['class']  // 只监听 class，不监听其他属性
    })
}