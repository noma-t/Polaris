import { useEffect, useState } from "react";
import { getGameRunning, onGameRunningChanged } from "../lib/game";

/**
 * VRChat が起動しているかを返す。
 * listen を登録してから現在値を取得し、登録前の emit を取りこぼさないようにする。
 */
export function useGameRunning() {
  const [isRunning, setIsRunning] = useState(false);

  useEffect(() => {
    let isCancelled = false;
    let hasReceivedEvent = false;
    const subscription = onGameRunningChanged((next) => {
      if (isCancelled) return;
      hasReceivedEvent = true;
      setIsRunning(next);
    });
    subscription
      .then(() => getGameRunning())
      .then((current) => {
        // 取得中に emit された値のほうが新しい
        if (!isCancelled && !hasReceivedEvent) setIsRunning(current);
      })
      .catch(() => {
        // 取得に失敗しても以降の emit で更新される
      });

    return () => {
      isCancelled = true;
      subscription.then((unlisten) => unlisten());
    };
  }, []);

  return isRunning;
}
