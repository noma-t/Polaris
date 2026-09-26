import { formatClock } from "../lib/format";

export function StatusBar({ updatedAt }: { updatedAt: number | null }) {
  return (
    <footer className="status-bar">
      <span>
        Updated: <span className="status-bar-time">{updatedAt ? formatClock(updatedAt, true) : "--:--:--"}</span>
      </span>
    </footer>
  );
}
