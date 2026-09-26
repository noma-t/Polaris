import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";

export type { Update };

/** 新しい version があれば Update を返す。最新なら null */
export const checkForUpdate = () => check();

/**
 * 更新を download して install し、アプリを再起動する。
 * Windows では install 開始時点でアプリが終了し、installer が再起動を行うため relaunch には到達しない。
 */
export async function downloadAndInstallUpdate(update: Update, onProgress: (ratio: number | null) => void) {
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
}
