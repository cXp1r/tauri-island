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

let raf: number
let targetW: number = 0
let targetH: number = 0
const el = document.getElementById('island-capsule')!
let rect = el.getBoundingClientRect()
let fromW = Math.round(rect.width)
let fromH = Math.round(rect.height)

export function animateCapsule(toW: number, toH: number): void {
  if (toW === targetW && toH === targetH) return
  targetW = toW
  targetH = toH

  cancelAnimationFrame(raf)

  // 从当前实际样式读取起点，而不是依赖外部 fromW/fromH
  const currentW = parseFloat(el.style.width) || fromW
  const currentH = parseFloat(el.style.height) || fromH
  const startW = currentW
  const startH = currentH

  const start = performance.now()
  let lw = startW
  let lh = startH

  function frame(now: number): void {
    const t = Math.min((now - start) / 350, 1)
    const e = easing(t)
    const w = Math.round(startW + (toW - startW) * e)
    const h = Math.round(startH + (toH - startH) * e)

    el.style.width  = w + 'px'
    el.style.height = h + 'px'

    invoke('resize_raf', { width: w, height: h + 10, lwidth: lw, lheight: lh + 10, reposition: true })
    lw = w
    lh = h

    if (t < 1) {
      raf = requestAnimationFrame(frame)
    } else {
      fromW = toW  // ← 动画结束后再更新
      fromH = toH
    }
  }

  raf = requestAnimationFrame(frame)
}

export function animateCapsuleWithoutRaf(toW: number, toH: number): void {
  if (toW === targetW && toH === targetH) return
  targetW = toW
  targetH = toH

  clearInterval(raf)

  const currentW = parseFloat(el.style.width) || fromW
  const currentH = parseFloat(el.style.height) || fromH
  const startW = currentW
  const startH = currentH

  const start = performance.now()
  let lw = startW
  let lh = startH

  raf = window.setInterval(() => {
    const t = Math.min((performance.now() - start) / 500, 1)
    const e = easing(t)
    const w = Math.round(startW + (toW - startW) * e)
    const h = Math.round(startH + (toH - startH) * e)

    el.style.width  = w + 'px'
    el.style.height = h + 'px'

    invoke('resize_raf', { width: w, height: h + 10, lwidth: lw, lheight: lh + 10, reposition: true })
    lw = w
    lh = h

    if (t >= 1) {
      clearInterval(raf)
      fromW = toW
      fromH = toH
    }
  }, 1000 / 100) // 约 60fps~
}

export function initrAF() {
  
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
                } else if (el.classList.contains("expanded")) {
                  [toW, toH] = [330, 74];
                } else if (el.classList.contains("lyric-collapsed")) {
                  [toW, toH] = [340, 50];
                } else if (el.classList.contains("sadb-idle")) {
                  [toW, toH] = [380, 420];
                } else if (el.classList.contains("email-expanded")) {
                  const style = getComputedStyle(document.documentElement);
                  [toW, toH] = [parseInt(style.getPropertyValue('--email-view-w')), parseInt(style.getPropertyValue('--email-view-h')),];
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