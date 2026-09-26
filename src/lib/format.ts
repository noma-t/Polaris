const pad2 = (n: number) => String(n).padStart(2, "0");

/** epoch ms を HH:mm (withSeconds なら HH:mm:ss) に整形する */
export function formatClock(time: number, withSeconds = false): string {
  const d = new Date(time);
  const hm = `${pad2(d.getHours())}:${pad2(d.getMinutes())}`;
  return withSeconds ? `${hm}:${pad2(d.getSeconds())}` : hm;
}

/** epoch ms を YYYY-MM-DD HH:mm に整形する */
export function formatDateTime(time: number): string {
  const d = new Date(time);
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())} ${formatClock(time)}`;
}
