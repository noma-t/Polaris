import { useNow } from "../hooks/useNow";
import { TOAST_DURATION_MS, type ToastState } from "../hooks/useToast";
import { CheckIcon, ErrorIcon } from "./icons";

export function Toast({ toast, onClose }: { toast: ToastState; onClose: () => void }) {
  const now = useNow(100, !!toast.message);
  if (!toast.message) return null;
  const remaining = Math.max(0, 1 - (now - toast.shownAt) / TOAST_DURATION_MS);
  const isError = toast.variant === "error";
  return (
    <div className="toast-container">
      <button
        className={`toast ${toast.visible ? "is-visible" : ""} ${isError ? "is-error" : ""}`}
        onClick={onClose}
        role={isError ? "alert" : "status"}
      >
        {isError ? (
          <ErrorIcon size={22} className="toast-icon" />
        ) : (
          <CheckIcon size={22} className="toast-icon" />
        )}
        <span>{toast.message}</span>
        <span className="toast-progress" style={{ width: `${remaining * 100}%` }} />
      </button>
    </div>
  );
}
