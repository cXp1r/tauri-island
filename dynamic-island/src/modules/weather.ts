import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { WeatherResult } from "../types";
import { timeWrapper, timeText, dateText, weatherText } from "../dom";
import { showNotice } from "./notice-url";

function formatDateLabel(now: Date): string {

  const weekdays = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"];

  const mm = `${now.getMonth() + 1}`;

  const dd = `${now.getDate()}`;

  return `${weekdays[now.getDay()]} ${mm}/${dd}`;

}



function updateTimeAndDate() {

  const now = new Date();

  timeText.innerText = now.toLocaleTimeString("zh-CN", { hour12: false });

  dateText.innerText = formatDateLabel(now);

}



// ===== 天气功能（后端后台线程推送）=====

async function refreshWeather(force = false) {

  if (force) {

    weatherText.textContent = "获取中...";

    void invoke("refresh_weather");

    return;

  }

  // 非强制刷新：尝试读取缓存

  try {

    const result = await invoke<WeatherResult>("get_weather");

    if (result.city) {

      weatherText.textContent = `${result.city} ${result.desc} ${result.temp}°C`;

    } else {

      weatherText.textContent = `${result.desc} ${result.temp}°C`;

    }

  } catch {

    // 缓存尚未就绪，后台线程会自动推送

    if (weatherText.textContent === "") {

      weatherText.textContent = "获取中...";

    }

  }

}



export function initWeather() {

  // 点击天气文本刷新

  weatherText.style.cursor = "pointer";

  weatherText.title = "点击刷新天气";

  weatherText.addEventListener("click", (e) => {

    e.stopPropagation();

    void refreshWeather(true);

  });



  timeWrapper.addEventListener("mouseenter", () => {

    updateTimeAndDate();

  });



  setInterval(updateTimeAndDate, 1000);

  updateTimeAndDate();



  // 启动时尝试读取缓存（后台线程会自动获取并推送）

  void refreshWeather();



  // 监听后端天气更新推送

  listen<{ desc: string; temp: number; city: string }>("weather-updated", (event) => {

    const r = event.payload;

    if (r.city) {

      weatherText.textContent = `${r.city} ${r.desc} ${r.temp}°C`;

    } else {

      weatherText.textContent = `${r.desc} ${r.temp}°C`;

    }

  });



  listen<{ error: string }>("weather-error", () => {

    if (weatherText.textContent === "获取中...") {

      weatherText.textContent = "天气暂不可用";

    }

  });



  // 监听设置页天气城市变更

  listen("weather-city-changed", () => {

    weatherText.textContent = "获取中...";

    // 后端已自动触发 force refresh，等待 weather-updated 事件即可

  });



  // 监听启动时自动检查更新结果

  listen<{ has_update: boolean; latest_version: string }>("update-available", (event) => {

    if (event.payload.has_update) {

      showNotice(`发现新版本 v${event.payload.latest_version}，请前往设置更新`);

    }

  });

}
