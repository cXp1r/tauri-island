import { invoke } from "@tauri-apps/api/core";
var logLevel = 1;
function n2t(level: number) {
    if(level == 0) {
        return "DEBUG";
    }else if(level == 1) {
        return "INFO";
    }else if(level == 2){
        return "WARN";
    }
}
export function initLogLevel() {
    invoke<number>("get_log_level_num").then(l => {
        
        console.log(`[Logger][DEBUG][${Math.floor(Date.now() / 1000)}]`, "log level:", n2t(l));
        logLevel = l;
    });
}
function log(level: number, tag: string, ...args: any[]) {
  if (logLevel > level) return;

  const err = new Error();
  const stack = err.stack;
  if (!stack) return;

  const lines = stack.split("\n");
  const targetLine = lines[3] || "";
  const match = targetLine.match(
    /at\s+(.+?)\s+\((?:.*\/)?([^\/?#]+)\?[^:]*:(\d+):(\d+)\)/
  );
  let atInfo = "";
  if (match) {
    atInfo = `|| at ${match[1]}() ${match[2]}:${match[3]}:${match[4]}`;
  }

  console.log(
    `[${tag}][${n2t(level)}][${Math.floor(Date.now() / 1000)}]`,
    ...args,
    atInfo
  );
}

export const loge = (tag: string, ...args: any[]) => log(3, tag, ...args);
export const logd = (tag: string, ...args: any[]) => log(0, tag, ...args);
export const logw = (tag: string, ...args: any[]) => log(2, tag, ...args);
export const logi = (tag: string, ...args: any[]) => log(1, tag, ...args);
