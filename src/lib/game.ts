import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface GameStatus {
  isRunning: boolean;
  /** 設定された launch.exe が存在するか */
  isLauncherFound: boolean;
}

export const getGameStatus = () => invoke<GameStatus>("game_get_status");

export const onGameStatusChanged = (handler: (status: GameStatus) => void): Promise<UnlistenFn> =>
  listen<GameStatus>("game://status-changed", (event) => handler(event.payload));

/** `wrld_…:…` 形式の location を起動中の VRChat で開く */
export const openInstanceInGame = (location: string) => invoke<void>("game_open_instance", { location });

/** Open ボタンを無効化する理由。開ける状態なら null */
export function openInstanceDisabledReason(status: GameStatus): string | null {
  if (!status.isRunning) return "VRChatが開かれていません";
  if (!status.isLauncherFound) return "launch.exeが存在しません";
  return null;
}
