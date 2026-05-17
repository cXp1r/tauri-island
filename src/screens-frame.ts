// ────────────────────────────────────────────────
// Types
// ────────────────────────────────────────────────

interface ScreenItem {
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

interface Vec2 {
  x: number;
  y: number;
}

// ────────────────────────────────────────────────
// Constants
// ────────────────────────────────────────────────

const SCALE_MIN = 0.05;
const SCALE_MAX = 5;
const PADDING   = 20;

// ────────────────────────────────────────────────
// Default data
// ────────────────────────────────────────────────

const screenData: ScreenItem[] = [
  { id: 'screen-0', x: 2560,  y: 344, width: 1920, height: 1080 },
  { id: 'screen-1', x: 0,     y: 0,   width: 2560, height: 1440 },
  { id: 'screen-2', x: -1920, y: 352, width: 1920, height: 1080 },
];

// ────────────────────────────────────────────────
// State
// ────────────────────────────────────────────────

let offset: Vec2     = { x: 0, y: 0 };
let scale: number    = 1;
let selectedId: string | null = null;

// ────────────────────────────────────────────────
// DOM refs
// ────────────────────────────────────────────────

const frame          = document.getElementById('screens-frame')     as HTMLDivElement;
const canvas         = document.getElementById('grid-canvas')       as HTMLCanvasElement;
const ctx            = canvas.getContext('2d')!;
const world          = document.getElementById('world')             as HTMLDivElement;
const worldContainer = document.getElementById('world-container')   as HTMLDivElement;


// ────────────────────────────────────────────────
// Grid canvas
// ────────────────────────────────────────────────

function resizeCanvas(): void {
  canvas.width  = frame.clientWidth;
  canvas.height = frame.clientHeight;
  drawGrid();
}

function drawGrid(): void {
  const W = canvas.width;
  const H = canvas.height;
  ctx.clearRect(0, 0, W, H);

  const SMALL = 40 * scale;
  const LARGE = 200 * scale;

  const ox  = ((offset.x % SMALL) + SMALL) % SMALL;
  const oy  = ((offset.y % SMALL) + SMALL) % SMALL;
  const oxL = ((offset.x % LARGE) + LARGE) % LARGE;
  const oyL = ((offset.y % LARGE) + LARGE) % LARGE;

  ctx.lineWidth = 1;

  ctx.strokeStyle = 'rgba(26,111,255,0.07)';
  for (let x = ox - SMALL; x < W + SMALL; x += SMALL) {
    ctx.beginPath(); ctx.moveTo(x, 0); ctx.lineTo(x, H); ctx.stroke();
  }
  for (let y = oy - SMALL; y < H + SMALL; y += SMALL) {
    ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(W, y); ctx.stroke();
  }

  ctx.strokeStyle = 'rgba(26,111,255,0.14)';
  for (let x = oxL - LARGE; x < W + LARGE; x += LARGE) {
    ctx.beginPath(); ctx.moveTo(x, 0); ctx.lineTo(x, H); ctx.stroke();
  }
  for (let y = oyL - LARGE; y < H + LARGE; y += LARGE) {
    ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(W, y); ctx.stroke();
  }

  ctx.strokeStyle = 'rgba(26,111,255,0.3)';
  if (offset.x >= 0 && offset.x <= W) {
    ctx.beginPath(); ctx.moveTo(offset.x, 0); ctx.lineTo(offset.x, H); ctx.stroke();
  }
  if (offset.y >= 0 && offset.y <= H) {
    ctx.beginPath(); ctx.moveTo(0, offset.y); ctx.lineTo(W, offset.y); ctx.stroke();
  }
}

// ────────────────────────────────────────────────
// World / DOM rendering
// ────────────────────────────────────────────────

function renderScreens(data: ScreenItem[]): void {
  world.querySelectorAll('.screen-div').forEach(e => e.remove());

  data.forEach((s, i) => {
    const div = document.createElement('div');
    div.className = 'screen-div';
    div.id = s.id;
    div.innerHTML = `
      <span class="screen-label">SCREEN · ${i}</span>
      <span class="screen-dims">${s.width} × ${s.height}</span>
    `;
    div.style.left   = `${s.x}px`;
    div.style.top    = `${s.y}px`;
    div.style.width  = `${s.width}px`;
    div.style.height = `${s.height}px`;
    div.addEventListener('click', (e) => {
      e.stopPropagation();
      selectScreen(s.id, s);
    });
    world.appendChild(div);
  });
}

function applyOffset(): void {
  world.style.transform       = `translate(${offset.x}px, ${offset.y}px) scale(${scale})`;
  world.style.transformOrigin = '0 0';
  drawGrid();
}

function selectScreen(id: string, data: ScreenItem): void {
  if (selectedId) {
    document.getElementById(selectedId)?.classList.remove('selected');
  }
  selectedId = id;
  document.getElementById(id)?.classList.add('selected');
}

// ────────────────────────────────────────────────
// Drag to pan
// ────────────────────────────────────────────────

let isDragging  = false;
let dragStart:   Vec2 = { x: 0, y: 0 };
let offsetStart: Vec2 = { x: 0, y: 0 };

worldContainer.addEventListener('mousedown', (e: MouseEvent) => {
  if (e.button !== 0) return;
  isDragging  = true;
  dragStart   = { x: e.clientX, y: e.clientY };
  offsetStart = { ...offset };
  worldContainer.classList.add('dragging');
});

frame.addEventListener('mousemove', (e: MouseEvent) => {
  if (!isDragging) return;
  offset.x = offsetStart.x + (e.clientX - dragStart.x);
  offset.y = offsetStart.y + (e.clientY - dragStart.y);
  applyOffset();
});

const stopDrag = (): void => {
  isDragging = false;
  worldContainer.classList.remove('dragging');
};

frame.addEventListener('mouseup',    stopDrag);
frame.addEventListener('mouseleave', stopDrag);

// Deselect on background click
worldContainer.addEventListener('click', (e: MouseEvent) => {
  if (e.target === worldContainer || e.target === world) {
    if (selectedId) {
      document.getElementById(selectedId)?.classList.remove('selected');
      selectedId = null;
    }
  }
});

// ── Touch ──
worldContainer.addEventListener('touchstart', (e: TouchEvent) => {
  const t = e.touches[0];
  isDragging  = true;
  dragStart   = { x: t.clientX, y: t.clientY };
  offsetStart = { ...offset };
}, { passive: true });

frame.addEventListener('touchmove', (e: TouchEvent) => {
  if (!isDragging) return;
  const t = e.touches[0];
  offset.x = offsetStart.x + (t.clientX - dragStart.x);
  offset.y = offsetStart.y + (t.clientY - dragStart.y);
  applyOffset();
}, { passive: true });

frame.addEventListener('touchend', () => { isDragging = false; });

// ── Wheel zoom ──
worldContainer.addEventListener('wheel', (e: WheelEvent) => {
  e.preventDefault();
  const rect    = frame.getBoundingClientRect();
  const mouseX  = e.clientX - rect.left;
  const mouseY  = e.clientY - rect.top;
  const factor  = e.deltaY < 0 ? 1.1 : 0.9;
  const newScale = Math.min(SCALE_MAX, Math.max(SCALE_MIN, scale * factor));
  offset.x = mouseX - (mouseX - offset.x) * (newScale / scale);
  offset.y = mouseY - (mouseY - offset.y) * (newScale / scale);
  scale = newScale;
  applyOffset();
}, { passive: false });

// ────────────────────────────────────────────────
// Controls
// ────────────────────────────────────────────────

function centerAll(): void {
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  screenData.forEach(s => {
    minX = Math.min(minX, s.x);
    minY = Math.min(minY, s.y);
    maxX = Math.max(maxX, s.x + s.width);
    maxY = Math.max(maxY, s.y + s.height);
  });
  const totalW = maxX - minX;
  const totalH = maxY - minY;
  const W = frame.clientWidth;
  const H = frame.clientHeight;
  scale    = Math.min((W - PADDING * 2) / totalW, (H - PADDING * 2) / totalH, 1);
  offset.x = (W - totalW * scale) / 2 - minX * scale;
  offset.y = (H - totalH * scale) / 2 - minY * scale;
  applyOffset();
}

// Exposed to HTML onclick
(window as any).resetOffset = (): void => { centerAll(); };

// ────────────────────────────────────────────────
// Public API
// ────────────────────────────────────────────────

function setScreenData(data: Omit<ScreenItem, 'id'>[]): void {
  const items: ScreenItem[] = data.map((s, i) => ({
    ...s,
    id: `screen-${i}`,
  }));
  screenData.length = 0;
  items.forEach(s => screenData.push(s));
  renderScreens(screenData);
  centerAll();
  if (screenData.length > 0) selectScreen(screenData[0].id, screenData[0]);
}

(window as any).setScreenData = setScreenData;

// ────────────────────────────────────────────────
// ResizeObserver — debounced, skip if size unchanged
// ────────────────────────────────────────────────

let resizeTimer: ReturnType<typeof setTimeout> | null = null;

new ResizeObserver(() => {
  if (resizeTimer !== null) clearTimeout(resizeTimer);
  resizeTimer = setTimeout(() => {
    resizeTimer = null;
    const W = frame.clientWidth;
    const H = frame.clientHeight;
    if (canvas.width === W && canvas.height === H) return;
    canvas.width  = W;
    canvas.height = H;
    drawGrid();
  }, 50);
}).observe(frame);

// ────────────────────────────────────────────────
// Init — lazy, called once when screens page becomes visible
// ────────────────────────────────────────────────

let initialized = false;

function init(): void {
  if (initialized) return;
  if (frame.clientWidth === 0 || frame.clientHeight === 0) return;
  initialized = true;
  resizeCanvas();
  renderScreens(screenData);
  centerAll();
  if (screenData.length > 0) selectScreen(screenData[0].id, screenData[0]);
}

// Exposed globally so non-module scripts can call window.initScreensFrame()
(window as any).initScreensFrame = (): void => {
  requestAnimationFrame(() => init());
};