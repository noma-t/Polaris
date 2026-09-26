import { useCallback, useEffect, useRef, useState } from "react";
import { DEFAULT_APP_SETTINGS, loadAppSettings, saveAppSettings, type AppSettings } from "../lib/settings";

/**
 * アカウントに依らない設定を起動時に 1 度読み込み、部分的な変更を保存する。
 * 読み込み完了までは appSettings が null。保存に失敗した場合は reject し、表示中の値は変更前に戻す。
 */
export function useAppSettings() {
  const [appSettings, setAppSettings] = useState<AppSettings | null>(null);
  /** 連続した変更を取りこぼさないよう、最新の値を同期的に参照する */
  const latestRef = useRef<AppSettings | null>(null);

  useEffect(() => {
    let isCancelled = false;
    loadAppSettings()
      .catch(() => DEFAULT_APP_SETTINGS)
      .then((settings) => {
        if (isCancelled) return;
        latestRef.current = settings;
        setAppSettings(settings);
      });
    return () => {
      isCancelled = true;
    };
  }, []);

  const updateAppSettings = useCallback(async (patch: Partial<AppSettings>) => {
    const previous = latestRef.current;
    if (!previous) return;
    const next = { ...previous, ...patch };
    latestRef.current = next;
    setAppSettings(next);
    try {
      await saveAppSettings(next);
    } catch (err) {
      latestRef.current = previous;
      setAppSettings(previous);
      throw err;
    }
  }, []);

  return { appSettings, updateAppSettings };
}
