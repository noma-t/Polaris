const pad2 = (n: number) => String(n).padStart(2, "0");

/** epoch ms を HH:mm (withSeconds なら HH:mm:ss) に整形する */
export function formatClock(time: number, withSeconds = false): string {
  const d = new Date(time);
  const hm = `${pad2(d.getHours())}:${pad2(d.getMinutes())}`;
  return withSeconds ? `${hm}:${pad2(d.getSeconds())}` : hm;
}

/** これ以上経過した記録は表示しない (最大表示は 23h59m) */
export const ELAPSED_LIMIT_MS = 24 * 60 * 60 * 1000;

/**
 * since から now までの経過時間を `12m` / `3h05m` に整形する。24 時間以上経っていたら null。
 * 時計のずれで since が未来でも `0m` として扱う
 */
export function formatElapsed(since: number, now: number): string | null {
  const elapsed = Math.max(0, now - since);
  if (elapsed >= ELAPSED_LIMIT_MS) return null;
  const totalMinutes = Math.floor(elapsed / 60_000);
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  return hours === 0 ? `${minutes}m` : `${hours}h${pad2(minutes)}m`;
}

/** epoch ms を YYYY-MM-DD HH:mm に整形する */
export function formatDateTime(time: number): string {
  const d = new Date(time);
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())} ${formatClock(time)}`;
}
