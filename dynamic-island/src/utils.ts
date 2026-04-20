export function formatTime(ms: number): string {

  const totalSec = Math.floor(ms / 1000);

  const m = Math.floor(totalSec / 60);

  const s = totalSec % 60;

  return `${m}:${s.toString().padStart(2, "0")}`;

}

export function truncateUrl(url: string, max: number): string {

  if (url.length <= max) return url;

  return `${url.substring(0, max - 1)}…`;

}
