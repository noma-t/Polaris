import { useCallback, useEffect, useRef, useState } from "react";

export const TOAST_DURATION_MS = 3200;
const TOAST_EXIT_MS = 260;

export type ToastVariant = "success" | "error";

export interface ToastState {
  message: string | null;
  variant: ToastVariant;
  visible: boolean;
  shownAt: number;
}

export function useToast() {
  const [toast, setToast] = useState<ToastState>({ message: null, variant: "success", visible: false, shownAt: 0 });
  const hideTimer = useRef<number>(undefined);
  const removeTimer = useRef<number>(undefined);
  const enterFrame = useRef<number>(undefined);

  const clearTimers = () => {
    clearTimeout(hideTimer.current);
    clearTimeout(removeTimer.current);
    cancelAnimationFrame(enterFrame.current ?? 0);
  };

  const hideToast = useCallback(() => {
    clearTimeout(hideTimer.current);
    setToast((t) => ({ ...t, visible: false }));
    removeTimer.current = window.setTimeout(() => setToast((t) => ({ ...t, message: null })), TOAST_EXIT_MS);
  }, []);

  const showToast = useCallback(
    (message: string, variant: ToastVariant = "success") => {
      clearTimers();
      setToast({ message, variant, visible: false, shownAt: Date.now() });
      // 初期位置を描画してから visible にし、enter transition を発火させる
      enterFrame.current = requestAnimationFrame(() => {
        enterFrame.current = requestAnimationFrame(() => setToast((t) => ({ ...t, visible: true })));
      });
      hideTimer.current = window.setTimeout(hideToast, TOAST_DURATION_MS);
    },
    [hideToast],
  );

  useEffect(() => clearTimers, []);

  return { toast, showToast, hideToast };
}
