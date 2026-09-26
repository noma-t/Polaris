import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export const getGameRunning = () => invoke<boolean>("game_is_running");

export const onGameRunningChanged = (handler: (isRunning: boolean) => void): Promise<UnlistenFn> =>
  listen<boolean>("game://running-changed", (event) => handler(event.payload));
