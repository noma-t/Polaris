import { formatClock } from "../lib/format";

export function StatusBar({ updatedAt, isGameRunning }: { updatedAt: number | null; isGameRunning: boolean }) {
  return (
    <footer className="status-bar">
      <span
        className={`status-bar-game${isGameRunning ? " is-running" : ""}`}
        title={isGameRunning ? "VRChat is running" : "VRChat is not running"}
      >
        <span className="status-dot status-bar-game-dot" />
        Game
      </span>
      <span>
        Updated: <span className="status-bar-time">{updatedAt ? formatClock(updatedAt, true) : "--:--:--"}</span>
      </span>
    </footer>
  );
}
