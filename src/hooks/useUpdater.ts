import { useCallback, useEffect, useRef, useState } from "react";
import { checkForUpdate, type AvailableUpdate } from "../lib/updater";

export type UpdateStatus =
  | "idle"
  | "checking"
  | "latest"
  | "available"
  | "downloading"
  | "installing"
  | "check-failed"
  | "update-failed";

interface UseUpdaterOptions {
  /** false の間は自動チェックを行わない (設定の読み込み待ち) */
  isReady: boolean;
  /** true ならダミーの update を検出する */
  simulate: boolean;
  /** ダミーの update の install が完了したとき */
  onSimulatedInstallFinished: () => void;
}

/**
 * 起動時 (isReady になった時点) と simulate の切り替え時に update を自動チェックし、
 * 手動チェック・install の状態も管理する。
 */
export function useUpdater({ isReady, simulate, onSimulatedInstallFinished }: UseUpdaterOptions) {
  const [status, setStatus] = useState<UpdateStatus>("idle");
  const [update, setUpdate] = useState<AvailableUpdate | null>(null);
  /** download 進捗 (0〜1)。サイズ不明の場合は null */
  const [downloadProgress, setDownloadProgress] = useState<number | null>(null);
  /** 古いチェック結果で上書きしないよう、最後に開始したチェックを識別する */
  const checkIdRef = useRef(0);
  const onSimulatedInstallFinishedRef = useRef(onSimulatedInstallFinished);
  onSimulatedInstallFinishedRef.current = onSimulatedInstallFinished;

  const isBusy = status === "checking" || status === "downloading" || status === "installing";
  const isUpdateAvailable = update !== null && (status === "available" || status === "update-failed");

  const check = useCallback(async () => {
    const checkId = ++checkIdRef.current;
    setStatus("checking");
    try {
      const found = await checkForUpdate({ simulate });
      if (checkId !== checkIdRef.current) return;
      setUpdate(found);
      setStatus(found ? "available" : "latest");
    } catch {
      if (checkId !== checkIdRef.current) return;
      setUpdate(null);
      setStatus("check-failed");
    }
  }, [simulate]);

  useEffect(() => {
    if (isReady) void check();
  }, [isReady, check]);

  const install = useCallback(async (target: AvailableUpdate) => {
    setDownloadProgress(null);
    setStatus("downloading");
    try {
      await target.downloadAndInstall((ratio) => {
        setDownloadProgress(ratio);
        if (ratio === 1) setStatus("installing");
      });
      if (target.isSimulated) {
        setStatus("available");
        onSimulatedInstallFinishedRef.current();
      } else {
        setStatus("installing");
      }
    } catch {
      setStatus("update-failed");
    }
  }, []);

  return { status, update, downloadProgress, isBusy, isUpdateAvailable, check, install };
}

export type UpdaterState = ReturnType<typeof useUpdater>;
