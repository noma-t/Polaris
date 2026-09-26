import { getVersion } from "@tauri-apps/api/app";
import { useEffect, useRef, useState } from "react";

/** 前回起動時の version を保存する localStorage key */
const LAST_LAUNCHED_VERSION_KEY = "polaris.lastLaunchedVersion";

/**
 * tauri.conf.json の version を返す。
 * 前回起動時と version が変わっていれば (= 更新直後の起動) onUpdated を 1 度だけ呼ぶ。
 */
export function useAppVersion(onUpdated: (version: string) => void) {
  const [version, setVersion] = useState("");
  const onUpdatedRef = useRef(onUpdated);
  onUpdatedRef.current = onUpdated;

  useEffect(() => {
    let isCancelled = false;
    getVersion()
      .then((current) => {
        if (isCancelled) return;
        setVersion(current);
        let lastVersion: string | null = null;
        try {
          lastVersion = localStorage.getItem(LAST_LAUNCHED_VERSION_KEY);
          localStorage.setItem(LAST_LAUNCHED_VERSION_KEY, current);
        } catch {
          // storage が使えない環境では更新通知を出さないだけ
        }
        if (lastVersion && lastVersion !== current) onUpdatedRef.current(current);
      })
      .catch(() => {});
    return () => {
      isCancelled = true;
    };
  }, []);

  return version;
}
