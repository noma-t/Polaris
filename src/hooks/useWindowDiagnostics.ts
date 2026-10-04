import { useEffect } from "react";
import { diagnosticLog } from "../lib/diagnosticLog";

/**
 * WebView 側から見えるウィンドウ・ページ・ネットワークの状態変化を診断ログに記録する。
 * 自動更新が止まったとき、表示状態 (visibilityState) やタイマーの凍結が原因かを後から追えるようにする。
 */
export function useWindowDiagnostics() {
  useEffect(() => {
    diagnosticLog(`page: loaded visibility=${document.visibilityState} focused=${document.hasFocus()} online=${navigator.onLine}`);

    const logVisibility = () => diagnosticLog(`page: visibility=${document.visibilityState} focused=${document.hasFocus()}`);
    const listeners: [Window | Document, string, () => void][] = [
      [document, "visibilitychange", logVisibility],
      [window, "focus", () => diagnosticLog("page: window focus")],
      [window, "blur", () => diagnosticLog("page: window blur")],
      [window, "online", () => diagnosticLog("page: browser went online")],
      [window, "offline", () => diagnosticLog("page: browser went offline")],
      // Page Lifecycle API: 非表示のタブをブラウザが凍結・再開したときに発火する
      [document, "freeze", () => diagnosticLog("page: frozen by the browser")],
      [document, "resume", () => diagnosticLog("page: resumed after freeze")],
      [window, "pagehide", () => diagnosticLog("page: pagehide")],
      [window, "pageshow", () => diagnosticLog("page: pageshow")],
    ];
    for (const [target, type, listener] of listeners) target.addEventListener(type, listener);
    return () => {
      for (const [target, type, listener] of listeners) target.removeEventListener(type, listener);
    };
  }, []);
}
