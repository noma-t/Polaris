import { useEffect, useState } from "react";
import { getGameStatus, onGameStatusChanged, type GameStatus } from "../lib/game";

const INITIAL_STATUS: GameStatus = { isRunning: false, isLauncherFound: false };

/**
 * VRChat が起動しているか・launch.exe が存在するかを返す。
 * listen を登録してから現在値を取得し、登録前の emit を取りこぼさないようにする。
 */
export function useGameStatus() {
  const [status, setStatus] = useState<GameStatus>(INITIAL_STATUS);

  useEffect(() => {
    let isCancelled = false;
    let hasReceivedEvent = false;
    const subscription = onGameStatusChanged((next) => {
      if (isCancelled) return;
      hasReceivedEvent = true;
      setStatus(next);
    });
    subscription
      .then(() => getGameStatus())
      .then((current) => {
        // 取得中に emit された値のほうが新しい
        if (!isCancelled && !hasReceivedEvent) setStatus(current);
      })
      .catch(() => {
        // 取得に失敗しても以降の emit で更新される
      });

    return () => {
      isCancelled = true;
      subscription.then((unlisten) => unlisten());
    };
  }, []);

  return status;
}
