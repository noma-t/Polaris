import { useCallback, useEffect, useState } from "react";
import { DEFAULT_UI_STATE, loadUiState, saveUiState, type UiState } from "../lib/settings";

/** split handle のドラッグ等、連続した変更をまとめて保存するための待ち時間 */
const SAVE_DEBOUNCE_MS = 500;

/**
 * 開いている画面・Sidebar の開閉・Groups/Friends の幅・ソートなどの UI 状態を起動時に読み込み、変更を保存する。
 * 読み込み完了前は保存しない (既定値で上書きしないため)。
 */
export function useUiState() {
  const [uiState, setUiState] = useState<UiState>(DEFAULT_UI_STATE);
  const [isUiStateLoaded, setIsUiStateLoaded] = useState(false);

  useEffect(() => {
    let isCancelled = false;
    loadUiState()
      .catch(() => DEFAULT_UI_STATE)
      .then((loaded) => {
        if (isCancelled) return;
        setUiState(loaded);
        setIsUiStateLoaded(true);
      });
    return () => {
      isCancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!isUiStateLoaded) return;
    const timer = setTimeout(() => {
      saveUiState(uiState).catch(() => {
        // 次回の変更時に再度保存される
      });
    }, SAVE_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [isUiStateLoaded, uiState]);

  const updateUiState = useCallback((patch: Partial<UiState>) => setUiState((s) => ({ ...s, ...patch })), []);

  return { uiState, isUiStateLoaded, updateUiState };
}
