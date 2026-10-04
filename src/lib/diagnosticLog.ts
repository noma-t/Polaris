import { invoke } from "@tauri-apps/api/core";
import { isAuthError } from "./auth";

/**
 * フロントエンドの判断を診断ログ (`auth.log`) に記録する。ログが無効な間は Rust 側で捨てられる。
 * 呼び出しは状態が変わったときだけにする (毎秒の処理から呼ばない)。ログの失敗でアプリの動作は止めない。
 */
export const diagnosticLog = (message: string) => {
  invoke("auth_log_write", { message }).catch(() => {});
};

/** ログ用に invoke の reject 値を短く表す。認証エラーは種別とメッセージ、それ以外は文字列化する */
export function describeErrorForLog(error: unknown): string {
  if (isAuthError(error)) return `${error.kind}: ${error.message}`;
  if (error instanceof Error) return `${error.name}: ${error.message}`;
  return String(error);
}
