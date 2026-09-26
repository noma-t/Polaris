const pad2 = (n: number) => String(n).padStart(2, "0");

/** epoch ms を HH:mm (withSeconds なら HH:mm:ss) に整形する */
export function formatClock(time: number, withSeconds = false): string {
  const d = new Date(time);
  const hm = `${pad2(d.getHours())}:${pad2(d.getMinutes())}`;
  return withSeconds ? `${hm}:${pad2(d.getSeconds())}` : hm;
}
