import { useGameRunning } from "../hooks/useGameRunning";
import { formatClock } from "../lib/format";

export function StatusBar({ updatedAt }: { updatedAt: number | null }) {
  const isGameRunning = useGameRunning();

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
