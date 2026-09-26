import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";

/** download 進捗 (0〜1)。サイズ不明の場合は null */
export type ProgressHandler = (ratio: number | null) => void;

/** 検出された update。本物の update と、Developer 向けのダミー update を同じ形で扱う */
export interface AvailableUpdate {
  version: string;
  body?: string;
  /** true ならダミー。install しても何も変更されない */
  isSimulated: boolean;
  /**
   * download して install し、アプリを再起動する。
   * 本物の update では Windows の場合 install 開始時点でアプリが終了するため resolve しない。
   * ダミーの update は進捗を擬似的に進めた後 resolve する。
   */
  downloadAndInstall: (onProgress: ProgressHandler) => Promise<void>;
}

const SIMULATED_VERSION = "99.0.0";
const SIMULATED_DOWNLOAD_MS = 3000;
const SIMULATED_INSTALL_MS = 1500;
const SIMULATED_TICK_MS = 100;

const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/** 新しい version があれば返す。最新なら null。simulate が true なら通信せずにダミーを返す */
export async function checkForUpdate({ simulate }: { simulate: boolean }): Promise<AvailableUpdate | null> {
  if (simulate) return createSimulatedUpdate();
  const update = await check();
  return update && toAvailableUpdate(update);
}

function toAvailableUpdate(update: Update): AvailableUpdate {
  return {
    version: update.version,
    body: update.body,
    isSimulated: false,
    downloadAndInstall: async (onProgress) => {
      let totalBytes: number | null = null;
      let downloadedBytes = 0;
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          totalBytes = event.data.contentLength ?? null;
          onProgress(totalBytes ? 0 : null);
        } else if (event.event === "Progress") {
          downloadedBytes += event.data.chunkLength;
          onProgress(totalBytes ? Math.min(downloadedBytes / totalBytes, 1) : null);
        }
      });
      await relaunch();
    },
  };
}

function createSimulatedUpdate(): AvailableUpdate {
  return {
    version: SIMULATED_VERSION,
    body: "This is a simulated update for development.\nNothing will be downloaded or installed.",
    isSimulated: true,
    downloadAndInstall: async (onProgress) => {
      const steps = SIMULATED_DOWNLOAD_MS / SIMULATED_TICK_MS;
      for (let i = 0; i <= steps; i++) {
        onProgress(i / steps);
        await wait(SIMULATED_TICK_MS);
      }
      await wait(SIMULATED_INSTALL_MS);
    },
  };
}
