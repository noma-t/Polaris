import { useCallback, useEffect, useRef, useState } from "react";
import { diagnosticLog } from "../lib/diagnosticLog";
import { checkForUpdate, setTrayUpdateBadge, type AvailableUpdate } from "../lib/updater";

/** 起動し続けている間に update を再チェックする間隔 */
const PERIODIC_CHECK_INTERVAL_MS = 6 * 60 * 60 * 1000;
/**
 * 再チェックの時期かを判定する頻度。固定間隔のタイマーではなく前回チェックからの経過時間で判定するため、
 * スリープやトレイ格納中のタイマーのスロットリングを挟んでも、期限を過ぎていれば次の判定で実行される
 */
const PERIODIC_CHECK_TICK_MS = 10 * 60 * 1000;

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
 * 起動時 (isReady になった時点)・simulate の切り替え時・前回チェックから一定時間経過時に update を自動チェックし、
 * 手動チェック・install の状態も管理する。
 */
export function useUpdater({ isReady, simulate, onSimulatedInstallFinished }: UseUpdaterOptions) {
  const [status, setStatus] = useState<UpdateStatus>("idle");
  const [update, setUpdate] = useState<AvailableUpdate | null>(null);
  /** download 進捗 (0〜1)。サイズ不明の場合は null */
  const [downloadProgress, setDownloadProgress] = useState<number | null>(null);
  /** 最後にチェックが成功した日時 (epoch ms)。未チェック・失敗のみの場合は null */
  const [lastCheckedAt, setLastCheckedAt] = useState<number | null>(null);
  /** 古いチェック結果で上書きしないよう、最後に開始したチェックを識別する */
  const checkIdRef = useRef(0);
  /** 最後にチェックを開始した日時 (epoch ms)。失敗したチェックも含め、定期チェックの間隔の起点にする */
  const lastAttemptedAtRef = useRef<number | null>(null);
  const statusRef = useRef(status);
  statusRef.current = status;
  const onSimulatedInstallFinishedRef = useRef(onSimulatedInstallFinished);
  onSimulatedInstallFinishedRef.current = onSimulatedInstallFinished;

  const isBusy = status === "checking" || status === "downloading" || status === "installing";
  const isUpdateAvailable = update !== null && (status === "available" || status === "update-failed");

  const check = useCallback(async () => {
    const checkId = ++checkIdRef.current;
    lastAttemptedAtRef.current = Date.now();
    setStatus("checking");
    try {
      const found = await checkForUpdate({ simulate });
      if (checkId !== checkIdRef.current) return;
      setUpdate(found);
      setStatus(found ? "available" : "latest");
      setLastCheckedAt(Date.now());
    } catch {
      if (checkId !== checkIdRef.current) return;
      setUpdate(null);
      setStatus("check-failed");
    }
  }, [simulate]);

  useEffect(() => {
    if (isReady) void check();
  }, [isReady, check]);

  useEffect(() => {
    if (!isReady) return;
    const id = setInterval(() => {
      const lastAttemptedAt = lastAttemptedAtRef.current;
      if (lastAttemptedAt !== null && Date.now() - lastAttemptedAt < PERIODIC_CHECK_INTERVAL_MS) return;
      // download・install 中に状態を checking へ戻さないよう、操作中は次の判定に回す
      const currentStatus = statusRef.current;
      if (currentStatus === "checking" || currentStatus === "downloading" || currentStatus === "installing") return;
      diagnosticLog("updater: periodic check");
      void check();
    }, PERIODIC_CHECK_TICK_MS);
    return () => clearInterval(id);
  }, [isReady, check]);

  // 再チェック中・download 中も直前の検出結果を保つため、status ではなく update の有無で判定する
  const badgeVersion = update?.version ?? null;
  useEffect(() => {
    setTrayUpdateBadge(badgeVersion).catch(() => {});
  }, [badgeVersion]);

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

  return { status, update, downloadProgress, lastCheckedAt, isBusy, isUpdateAvailable, check, install };
}

export type UpdaterState = ReturnType<typeof useUpdater>;
