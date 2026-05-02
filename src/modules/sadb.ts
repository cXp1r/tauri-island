import { invoke } from "@tauri-apps/api/core";
import { Channel } from "@tauri-apps/api/core";
import {
  capsule,
  sadbArea,
  sadbCanvas, sadbBtnStart, sadbBtnStop, sadbStatus,
  sadbDeviceName, sadbResolution, sadbFps,
} from "../dom";
import { setSkipResizeSync } from "../state";

type PacketEvent =
  | { type: "meta"; device_name: string; codec: string; width: number; height: number }
  | { type: "packet"; pts: number; key_frame: boolean; config: boolean; data: string }
  | { type: "audio_packet"; pts: number; config: boolean; data: string }
  | { type: "error"; message: string }
  | { type: "closed" }
  | { type: "clipboard"; text: string };

type SadbAudioData = {
  timestamp: number;
  numberOfChannels: number;
  numberOfFrames: number;
  sampleRate: number;
  copyTo(destination: Float32Array, options: { planeIndex: number; format: "f32-planar" }): void;
  close(): void;
};

type AudioDecoder = {
  state: string;
  configure(config: unknown): void;
  decode(chunk: unknown): void;
  close(): void;
};

declare const AudioDecoder: {
  new(init: { output: (audioData: SadbAudioData) => void; error: (error: Error) => void }): AudioDecoder;
};

declare const EncodedAudioChunk: {
  new(init: { type: "key" | "delta"; timestamp: number; data: ArrayBufferView | ArrayBuffer }): unknown;
};

// ── H.264 helpers ──

function base64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

function splitAnnexB(data: Uint8Array): Uint8Array[] {
  const nalus: Uint8Array[] = [];
  let naluStart = -1;
  let i = 0;
  while (i < data.length) {
    const is4 = i + 3 < data.length &&
      data[i] === 0 && data[i + 1] === 0 && data[i + 2] === 0 && data[i + 3] === 1;
    const is3 = !is4 && i + 2 < data.length &&
      data[i] === 0 && data[i + 1] === 0 && data[i + 2] === 1;
    if (is4 || is3) {
      if (naluStart >= 0 && i > naluStart) {
        nalus.push(data.slice(naluStart, i));
      }
      naluStart = i + (is4 ? 4 : 3);
      i = naluStart;
    } else {
      i++;
    }
  }
  if (naluStart >= 0 && naluStart < data.length) {
    nalus.push(data.slice(naluStart));
  }
  return nalus;
}

function annexBToAVCC(data: Uint8Array): Uint8Array {
  const nalus = splitAnnexB(data);
  let total = 0;
  for (const n of nalus) total += 4 + n.length;
  const out = new Uint8Array(total);
  let pos = 0;
  for (const n of nalus) {
    const len = n.length;
    out[pos++] = (len >>> 24) & 0xff;
    out[pos++] = (len >>> 16) & 0xff;
    out[pos++] = (len >>> 8) & 0xff;
    out[pos++] = len & 0xff;
    out.set(n, pos);
    pos += len;
  }
  return out;
}

function buildAVCDecoderConfig(sps: Uint8Array, pps: Uint8Array): ArrayBuffer {
  const buf = new Uint8Array(11 + sps.length + pps.length);
  let i = 0;
  buf[i++] = 1;
  buf[i++] = sps[1];
  buf[i++] = sps[2];
  buf[i++] = sps[3];
  buf[i++] = 0xff;
  buf[i++] = 0xe1;
  buf[i++] = (sps.length >> 8) & 0xff;
  buf[i++] = sps.length & 0xff;
  buf.set(sps, i); i += sps.length;
  buf[i++] = 1;
  buf[i++] = (pps.length >> 8) & 0xff;
  buf[i++] = pps.length & 0xff;
  buf.set(pps, i);
  return buf.buffer;
}

// ── State ──

const ctx = sadbCanvas.getContext("2d")!;

let decoder: VideoDecoder | null = null;
let pendingW = 0;
let pendingH = 0;
let frameCounter = 0;
let lastFpsTick = performance.now();
let deviceW = 0;
let deviceH = 0;
let streaming = false;
let mouseButtons = 0;
let clipboardPollInterval: ReturnType<typeof setInterval> | null = null;
let currentSerial: string | null = null;
const SADB_INIT_CAP_W = 280; // 流启动时的基准宽度
const SADB_MIN_SCALE = 0.6;  // 最小缩放（约 168px 宽）
const SADB_MAX_SCALE = 3.0;  // 最大缩放（约 840px 宽）

// 按手机 AR 计算出的基准尺寸，sadbScale 乘上去就是实际尺寸
let initCapW = SADB_INIT_CAP_W;
let initCapH = SADB_INIT_CAP_W;
let sadbScale = 1.0;

// Audio decoder state
let audioCtx: AudioContext | null = null;
let audioDecoder: AudioDecoder | null = null;
let audioBaseTime = 0;
let audioBasePts = 0;

// Clipboard sync state (timestamp tracking avoids echo loops)
let pcClipboard: { text: string; timestamp: number } | null = null;
let phoneClipboard: { text: string; timestamp: number } | null = null;
let lastSyncedText: string | null = null;

// object-fit: contain draw rect for mouse coordinate mapping
let drawRect = { x: 0, y: 0, w: 0, h: 0 };

function cssPx(value: string) {
  const n = parseFloat(value);
  return Number.isFinite(n) ? n : 0;
}

function getSadbChromeSize() {
  const areaStyle = getComputedStyle(sadbArea);
  const statusBar = document.getElementById("sadb-status-bar") as HTMLDivElement | null;
  const controls = document.getElementById("sadb-controls") as HTMLDivElement | null;
  const gap = cssPx(areaStyle.rowGap || areaStyle.gap);
  const paddingX = cssPx(areaStyle.paddingLeft) + cssPx(areaStyle.paddingRight);
  const paddingY = cssPx(areaStyle.paddingTop) + cssPx(areaStyle.paddingBottom);
  const statusH = statusBar?.offsetHeight || 16;
  const controlsH = controls?.offsetHeight || 24;
  return {
    x: paddingX,
    y: paddingY + statusH + controlsH + gap * 2,
  };
}

function getScaledSize() {
  const scale = Math.max(SADB_MIN_SCALE, Math.min(SADB_MAX_SCALE, sadbScale));
  return {
    capW: Math.round(initCapW * scale),
    capH: Math.round(initCapH * scale),
  };
}

function updateDrawRect() {
  const cw = sadbCanvas.width;
  const ch = sadbCanvas.height;
  if (!cw || !ch) return;
  const rect = sadbCanvas.getBoundingClientRect();
  const canvasAspect = cw / ch;
  const rectAspect = rect.width / rect.height;
  if (canvasAspect > rectAspect) {
    drawRect.w = rect.width;
    drawRect.h = rect.width / canvasAspect;
    drawRect.x = 0;
    drawRect.y = (rect.height - drawRect.h) / 2;
  } else {
    drawRect.h = rect.height;
    drawRect.w = rect.height * canvasAspect;
    drawRect.x = (rect.width - drawRect.w) / 2;
    drawRect.y = 0;
  }
}

async function autoFitWindow() {
  if (!deviceW || !deviceH) return;
  const phoneAR = deviceW / deviceH;
  const chrome = getSadbChromeSize();
  initCapW = SADB_INIT_CAP_W;
  initCapH = Math.round((initCapW - chrome.x) / phoneAR + chrome.y);
  sadbScale = 1.0;
  const { capW, capH } = getScaledSize();
  capsule.style.width = `${capW}px`;
  capsule.style.height = `${capH}px`;
  requestAnimationFrame(updateDrawRect);
  const bodyPad = parseFloat(getComputedStyle(document.body).paddingTop) || 5;
  invoke("sync_window_size", { width: capW, height: capH + bodyPad + 5, reposition: true }).catch(() => {});
}

function setStatus(s: string) {
  sadbStatus.textContent = s;
}

function tickFps() {
  const now = performance.now();
  const dt = now - lastFpsTick;
  if (dt >= 1000) {
    const fps = Math.round((frameCounter * 1000) / dt);
    sadbFps.textContent = `${fps} fps`;
    frameCounter = 0;
    lastFpsTick = now;
  }
}

// ── Video ──

function renderFrame(frame: VideoFrame) {
  if (sadbCanvas.width !== frame.displayWidth || sadbCanvas.height !== frame.displayHeight) {
    sadbCanvas.width = frame.displayWidth;
    sadbCanvas.height = frame.displayHeight;
  }
  ctx.drawImage(frame, 0, 0, sadbCanvas.width, sadbCanvas.height);
  frame.close();
  frameCounter++;
  tickFps();
}

function initDecoder(codec: string, width: number, height: number) {
  if (decoder) { try { decoder.close(); } catch { /* ignore */ } }
  pendingW = width;
  pendingH = height;
  decoder = new VideoDecoder({
    output: renderFrame,
    error: (e) => {
      console.error("VideoDecoder error:", e);
      setStatus(`解码器错误: ${e.message}`);
    },
  });
  sadbCanvas.width = width;
  sadbCanvas.height = height;
  sadbResolution.textContent = `${width}x${height} (${codec})`;
}

function applyConfigPacket(data: Uint8Array) {
  if (!decoder) return;
  const nalus = splitAnnexB(data);
  const sps = nalus.find(n => n.length > 0 && (n[0] & 0x1f) === 7);
  const pps = nalus.find(n => n.length > 0 && (n[0] & 0x1f) === 8);
  if (!sps || !pps) return;
  const profile = sps[1].toString(16).padStart(2, "0");
  const compat = sps[2].toString(16).padStart(2, "0");
  const level = sps[3].toString(16).padStart(2, "0");
  const codecStr = `avc1.${profile}${compat}${level}`;
  const description = buildAVCDecoderConfig(sps, pps);
  decoder.configure({
    codec: codecStr,
    codedWidth: pendingW,
    codedHeight: pendingH,
    description,
    optimizeForLatency: true,
  });
}

// ── Audio (Opus via WebCodecs AudioDecoder) ──

function initAudioDecoder(configData: Uint8Array) {
  if (typeof AudioDecoder === "undefined") {
    console.warn("AudioDecoder not available in this browser");
    return;
  }
  if (configData.length < 19) {
    console.warn("Opus config packet too short:", configData.length);
    return;
  }
  const channelCount = configData[9];
  const sampleRate = new DataView(
    configData.buffer, configData.byteOffset + 12, 4
  ).getUint32(0, true);

  audioCtx = new AudioContext({ sampleRate });
  audioBaseTime = 0;
  audioBasePts = 0;

  audioDecoder = new AudioDecoder({
    output: (audioData) => {
      if (!audioCtx) return;
      if (audioBaseTime === 0) {
        audioBasePts = audioData.timestamp;
        audioBaseTime = audioCtx.currentTime + 0.05;
      }
      const buf = audioCtx.createBuffer(
        audioData.numberOfChannels,
        audioData.numberOfFrames,
        audioData.sampleRate,
      );
      for (let ch = 0; ch < audioData.numberOfChannels; ch++) {
        audioData.copyTo(buf.getChannelData(ch), { planeIndex: ch, format: "f32-planar" });
      }
      audioData.close();

      const source = audioCtx.createBufferSource();
      source.buffer = buf;
      source.connect(audioCtx.destination);
      const t = audioBaseTime + (audioData.timestamp - audioBasePts) / 1_000_000;
      source.start(Math.max(t, audioCtx.currentTime));
    },
    error: (e) => console.error("AudioDecoder error:", e),
  });

  audioDecoder.configure({
    codec: "opus",
    sampleRate,
    numberOfChannels: channelCount,
    description: configData,
  });
  console.log(`AudioDecoder configured: opus ${sampleRate}Hz ${channelCount}ch`);
}

function decodeAudio(pts: number, data: Uint8Array) {
  if (!audioDecoder || audioDecoder.state !== "configured") return;
  const chunk = new EncodedAudioChunk({
    type: "key",
    timestamp: pts,
    data,
  });
  audioDecoder.decode(chunk);
}

// ── Event handler ──

function handleEvent(evt: PacketEvent) {
  switch (evt.type) {
    case "meta":
      sadbDeviceName.textContent = evt.device_name;
      deviceW = evt.width;
      deviceH = evt.height;
      initDecoder(evt.codec, evt.width, evt.height);
      setStatus("镜像中");
      // 待机面板 → 镜像展开（CSS + 后端 flag；尺寸由 autoFitWindow 设置）
      capsule.classList.remove("sadb-idle");
      capsule.classList.add("sadb-expanded");
      invoke("set_sadb_expanded", { expanded: true }).catch(() => {});
      updateDrawRect();
      autoFitWindow();
      break;
    case "packet": {
      if (!decoder) return;
      const raw = base64ToBytes(evt.data);
      if (evt.config) {
        applyConfigPacket(raw);
        return;
      }
      if (decoder.state !== "configured") return;
      const avcc = annexBToAVCC(raw);
      const chunk = new EncodedVideoChunk({
        type: evt.key_frame ? "key" : "delta",
        timestamp: evt.pts,
        data: avcc,
      });
      try { decoder.decode(chunk); } catch (e) { console.error("decode error:", e); }
      break;
    }
    case "audio_packet": {
      const raw = base64ToBytes(evt.data);
      if (evt.config) {
        initAudioDecoder(raw);
        return;
      }
      decodeAudio(evt.pts, raw);
      break;
    }
    case "error":
      setStatus(`错误: ${evt.message}`);
      stopStream();
      break;
    case "closed":
      setStatus("已断开");
      stopStream();
      break;
    case "clipboard":
      phoneClipboard = { text: evt.text, timestamp: Date.now() };
      if (evt.text && evt.text !== lastSyncedText) {
        navigator.clipboard.writeText(evt.text)
          .then(() => { lastSyncedText = evt.text; })
          .catch(() => {});
      }
      break;
  }
}

// ── Start / Stop ──

async function startStream() {
  // 杀掉后端已有 session（后端保证干净状态）
  try { await invoke("sadb_stop_mirroring"); } catch { /* ignore */ }

  sadbBtnStart.disabled = true;
  sadbBtnStop.disabled = false;
  setStatus("连接中...");

  const settings = await invoke<any>("get_settings");
  const savedIp = settings.sadb_ip || "";
  const savedPort = settings.sadb_port || 5555;
  let serial: string | null = null;

  // Step 1: Try USB (no serial)
  try {
    const channel = new Channel<PacketEvent>();
    channel.onmessage = handleEvent;
    await invoke("sadb_start_mirroring", {
      channel,
      bitrate: 4_000_000,
      serial: null,
    });
    clipboardPollInterval = setInterval(pollPCClipboard, 1000);
    currentSerial = null;
    streaming = true;
    return;
  } catch (e) {
    console.error("USB mirroring failed:", e);
    setStatus(`USB失败: ${e}`);
  }

  // Step 2: Try WiFi with saved IP
  if (savedIp) {
    serial = `${savedIp}:${savedPort}`;
    setStatus(`连接WiFi设备 ${serial}...`);
    try {
      await invoke("sadb_connect_device", { serial });
    } catch (e) {
      console.error("WiFi connect failed:", e);
      setStatus(`WiFi连接失败: ${e}`);
      sadbBtnStart.disabled = false;
      sadbBtnStop.disabled = true;
      return;
    }
    try {
      const channel = new Channel<PacketEvent>();
      channel.onmessage = handleEvent;
      await invoke("sadb_start_mirroring", {
        channel,
        bitrate: 4_000_000,
        serial,
      });
      clipboardPollInterval = setInterval(pollPCClipboard, 1000);
      currentSerial = serial;
      streaming = true;
      return;
    } catch (e) {
      console.error("WiFi mirroring failed:", e);
      setStatus(`镜像失败: ${e}`);
      sadbBtnStart.disabled = false;
      sadbBtnStop.disabled = true;
    }
  } else {
    setStatus("未发现USB设备，请在设置中配置WiFi IP");
    sadbBtnStart.disabled = false;
    sadbBtnStop.disabled = true;
  }
}

function stopStream() {
  sadbBtnStart.disabled = false;
  sadbBtnStop.disabled = true;
  deviceW = 0;
  deviceH = 0;
  mouseButtons = 0;
  streaming = false;
  if (decoder) { try { decoder.close(); } catch { /* ignore */ } decoder = null; }
  if (audioDecoder) { try { audioDecoder.close(); } catch { /* ignore */ } audioDecoder = null; }
  if (audioCtx) { try { audioCtx.close(); } catch { /* ignore */ } audioCtx = null; }
  audioBaseTime = 0;
  audioBasePts = 0;
  if (clipboardPollInterval) { clearInterval(clipboardPollInterval); clipboardPollInterval = null; }
  pcClipboard = null;
  phoneClipboard = null;
  lastSyncedText = null;
  sadbScale = 1.0;
  // 只有当前仍在 sadb 视图时才改 capsule 样式和触发 idle 动画
  // 若用户已切换到其他视图，仅做后端清理，不污染其他视图的尺寸
  const inSadbView = capsule.classList.contains("sadb-expanded") || capsule.classList.contains("sadb-idle");
  if (inSadbView) {
    capsule.classList.remove("sadb-expanded");
    capsule.classList.add("sadb-idle");
    capsule.style.width = "";
    capsule.style.height = "";
    invoke("set_sadb_expanded", { expanded: false }).catch(() => {});
    invoke("sadb_set_idle", { idle: true }).catch(() => {});
  }
  invoke("sadb_stop_mirroring").catch(console.error).finally(() => {
    if (currentSerial) {
      invoke("sadb_disconnect_device", { serial: currentSerial }).catch(() => {});
      currentSerial = null;
    }
  });
}

async function pollPCClipboard() {
  try {
    const text = await navigator.clipboard.readText();
    if (text && text !== lastSyncedText) {
      lastSyncedText = text;
      await invoke("sadb_set_clipboard", { text, paste: false });
    }
  } catch { /* ignore */ }
}

// ── Mouse input forwarding ──

function toDeviceCoords(e: MouseEvent): [number, number] {
  const rect = sadbCanvas.getBoundingClientRect();
  const rx = (e.clientX - rect.left - drawRect.x) / drawRect.w;
  const ry = (e.clientY - rect.top - drawRect.y) / drawRect.h;
  return [
    Math.round(Math.max(0, Math.min(sadbCanvas.width - 1, rx * sadbCanvas.width))),
    Math.round(Math.max(0, Math.min(sadbCanvas.height - 1, ry * sadbCanvas.height))),
  ];
}

sadbCanvas.addEventListener("mousedown", (e) => {
  if (!deviceW) return;
  e.preventDefault();
  e.stopPropagation();
  mouseButtons |= (1 << e.button);
  const [x, y] = toDeviceCoords(e);
  invoke("sadb_send_touch_event", { x, y, screenWidth: deviceW, screenHeight: deviceH, action: 0, buttons: mouseButtons }).catch(() => {});
});

sadbCanvas.addEventListener("mousemove", (e) => {
  if (!deviceW || mouseButtons === 0) return;
  const [x, y] = toDeviceCoords(e);
  invoke("sadb_send_touch_event", { x, y, screenWidth: deviceW, screenHeight: deviceH, action: 2, buttons: mouseButtons }).catch(() => {});
});

sadbCanvas.addEventListener("mouseup", (e) => {
  if (!deviceW) return;
  e.preventDefault();
  const [x, y] = toDeviceCoords(e);
  invoke("sadb_send_touch_event", { x, y, screenWidth: deviceW, screenHeight: deviceH, action: 1, buttons: mouseButtons }).catch(() => {});
  mouseButtons &= ~(1 << e.button);
});

sadbCanvas.addEventListener("mouseleave", () => {
  if (mouseButtons !== 0 && deviceW) {
    invoke("sadb_send_touch_event", { x: 0, y: 0, screenWidth: deviceW, screenHeight: deviceH, action: 1, buttons: mouseButtons }).catch(() => {});
    mouseButtons = 0;
  }
});

sadbCanvas.addEventListener("contextmenu", (e) => e.preventDefault());

sadbCanvas.addEventListener("wheel", (e) => {
  if (!deviceW) return;
  e.preventDefault();
  const [x, y] = toDeviceCoords(e);
  const vscroll = -e.deltaY / 53;
  const hscroll = e.deltaX / 53;
  invoke("sadb_send_scroll_event", { x, y, screenWidth: deviceW, screenHeight: deviceH, hscroll: Math.max(-16, Math.min(16, hscroll)), vscroll: Math.max(-16, Math.min(16, vscroll)) }).catch(() => {});
}, { passive: false });

sadbCanvas.addEventListener("mousedown", () => {
  if (deviceW) imeInput.focus();
});

// ── Keyboard / text input forwarding ──

const imeInput = document.createElement("textarea");
imeInput.style.cssText = "position:fixed;left:-9999px;top:0;width:1px;height:1px;opacity:0;pointer-events:none;";
document.body.appendChild(imeInput);

imeInput.addEventListener("input", (e: Event) => {
  if (!deviceW) return;
  const ie = e as InputEvent;
  if (ie.isComposing) return;
  const text = imeInput.value;
  if (text) {
    invoke("sadb_inject_text", { text }).catch(() => {});
    imeInput.value = "";
  }
});

imeInput.addEventListener("keydown", (e: KeyboardEvent) => {
  if (!deviceW) return;
  const key = e.key;

  if (key === "Backspace") {
    e.preventDefault();
    invoke("sadb_send_keycode", { action: 0, keycode: 67, metastate: 0 }).catch(() => {});
    return;
  }
  if (key === "Enter") {
    e.preventDefault();
    invoke("sadb_send_keycode", { action: 0, keycode: 66, metastate: 0 }).catch(() => {});
    return;
  }
  if (e.ctrlKey || e.metaKey) {
    const AMETA_CTRL_LEFT_ON = 0x00002000;
    const ctrlDown = () =>
      invoke("sadb_send_keycode", { action: 0, keycode: 113, metastate: AMETA_CTRL_LEFT_ON }).catch(() => {});
    const ctrlUp = () =>
      invoke("sadb_send_keycode", { action: 1, keycode: 113, metastate: 0 }).catch(() => {});

    switch (e.code) {
      case "KeyA": { // Select All
        e.preventDefault();
        ctrlDown();
        invoke("sadb_send_keycode", { action: 0, keycode: 29, metastate: AMETA_CTRL_LEFT_ON }).catch(() => {});
        invoke("sadb_send_keycode", { action: 1, keycode: 29, metastate: AMETA_CTRL_LEFT_ON }).catch(() => {});
        ctrlUp();
        return;
      }
      case "KeyC": { // Copy
        e.preventDefault();
        ctrlDown();
        invoke("sadb_send_keycode", { action: 0, keycode: 31, metastate: AMETA_CTRL_LEFT_ON }).catch(() => {});
        invoke("sadb_send_keycode", { action: 1, keycode: 31, metastate: AMETA_CTRL_LEFT_ON }).catch(() => {});
        ctrlUp();
        return;
      }
      case "KeyX": { // Cut
        e.preventDefault();
        ctrlDown();
        invoke("sadb_send_keycode", { action: 0, keycode: 52, metastate: AMETA_CTRL_LEFT_ON }).catch(() => {});
        invoke("sadb_send_keycode", { action: 1, keycode: 52, metastate: AMETA_CTRL_LEFT_ON }).catch(() => {});
        ctrlUp();
        return;
      }
      case "KeyV": { // Paste: use whichever clipboard (PC or phone) is more recent
        e.preventDefault();
        navigator.clipboard.readText()
          .then(text => { if (text) pcClipboard = { text, timestamp: Date.now() }; })
          .catch(() => {})
          .finally(() => {
            const usePc = pcClipboard &&
              (!phoneClipboard || pcClipboard.timestamp >= phoneClipboard.timestamp);
            const pasteText = usePc ? pcClipboard!.text : phoneClipboard?.text;
            if (pasteText) {
              lastSyncedText = pasteText;
              invoke("sadb_set_clipboard", { text: pasteText, paste: true }).catch(() => {});
            }
          });
        return;
      }
    }
  }
});

// Paste: intercept browser paste and forward to device clipboard
imeInput.addEventListener("paste", (e) => {
  if (!deviceW) return;
  e.preventDefault();
  const text = (e as ClipboardEvent).clipboardData?.getData("text/plain") || "";
  if (text) {
    lastSyncedText = text;
    invoke("sadb_set_clipboard", { text, paste: true }).catch(() => {});
  }
});

// ── Buttons ──

sadbBtnStart.addEventListener("click", startStream);
sadbBtnStop.addEventListener("click", () => { setStatus("停止中..."); stopStream(); });

// ── Initial blank canvas ──

sadbCanvas.width = 320;
sadbCanvas.height = 480;
ctx.fillStyle = "#0a0a0a";
ctx.fillRect(0, 0, sadbCanvas.width, sadbCanvas.height);
ctx.fillStyle = "#555";
ctx.font = "12px system-ui";
ctx.textAlign = "center";
ctx.fillText("点击「开始」连接Android设备", sadbCanvas.width / 2, sadbCanvas.height / 2);

export function initSadb() {
  updateDrawRect();
  new ResizeObserver(() => updateDrawRect()).observe(sadbCanvas);

  // ── Resize handle ──
  let resizing = false;
  let resizeStartX = 0;
  let resizeStartScale = 1.0;

  const resizeHandle = document.getElementById("sadb-resize-handle") as HTMLDivElement;
  let syncPending = false;

  resizeHandle.addEventListener("mousedown", (e) => {
    if (!deviceW) return;
    e.preventDefault();
    e.stopPropagation();
    resizing = true;
    resizeStartX = e.screenX;
    resizeStartScale = sadbScale;
    setSkipResizeSync(true);
    capsule.style.transition = "none";
    console.log("[sadb-resize] mousedown: screenX=%d, scale=%.3f, initCapW=%d, initCapH=%d, capW=%d, capH=%d",
      e.screenX, sadbScale, initCapW, initCapH, capsule.offsetWidth, capsule.offsetHeight);
  });

  document.addEventListener("mousemove", (e) => {
    if (!resizing) return;
    const dx = e.screenX - resizeStartX;
    const prevScale = sadbScale;
    sadbScale = resizeStartScale + (2 * dx) / initCapW;
    const { capW, capH } = getScaledSize();
    capsule.style.width = `${capW}px`;
    capsule.style.height = `${capH}px`;
    requestAnimationFrame(updateDrawRect);
    console.log("[sadb-resize] move: dx=%d, scale %.3f→%.3f, cap %dx%d, offsetW=%d",
      dx, prevScale, sadbScale, capW, capH, capsule.offsetWidth);
    if (!syncPending) {
      syncPending = true;
      requestAnimationFrame(() => {
        syncPending = false;
        const { capW: sw, capH: sh } = getScaledSize();
        const bodyPad = parseFloat(getComputedStyle(document.body).paddingTop) || 5;
        console.log("[sadb-resize] sync_window_size: %dx%d", sw, sh + bodyPad + 5);
        invoke("sync_window_size", { width: sw, height: sh + bodyPad + 5, reposition: false }).catch(() => {});
      });
    }
  });

  document.addEventListener("mouseup", async () => {
    if (!resizing) return;
    resizing = false;
    capsule.style.transition = "";
    setSkipResizeSync(false);
    const { capW: fw, capH: fh } = getScaledSize();
    const bodyPad = parseFloat(getComputedStyle(document.body).paddingTop) || 5;
    console.log("[sadb-resize] mouseup: scale=%d/1000, sync %dx%d", Math.round(sadbScale * 1000), fw, fh + bodyPad + 5);
    try { await invoke("sync_window_size", { width: fw, height: fh + bodyPad + 5, reposition: false }); } catch { /* ignore */ }
  });
}

export function isSadbStreaming(): boolean {
  return streaming;
}

// 切回 sadb 视图时调用：恢复正确的 capsule 状态和窗口位置
export function onSadbViewEntered() {
  if (streaming) {
    capsule.classList.remove("sadb-idle");
    capsule.classList.add("sadb-expanded");
  } else {
    capsule.classList.remove("sadb-expanded");
    capsule.classList.add("sadb-idle");
    invoke("sadb_set_idle", { idle: true }).catch(() => {});
  }
}
