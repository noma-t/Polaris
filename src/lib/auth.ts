import { invoke } from "@tauri-apps/api/core";

export interface CurrentUser {
  id: string;
  displayName: string;
}

export type TwoFactorMethod = "totp" | "emailOtp";

export type LoginResult =
  | { kind: "signedIn"; user: CurrentUser }
  | { kind: "requiresTwoFactor"; methods: TwoFactorMethod[] };

export type AuthErrorKind = "invalidCredentials" | "invalidCode" | "rateLimited" | "network" | "unexpected";

/** Rust 側 AuthError のシリアライズ形式 */
export interface AuthError {
  kind: AuthErrorKind;
  message: string;
}

export function isAuthError(value: unknown): value is AuthError {
  return typeof value === "object" && value !== null && "kind" in value && "message" in value;
}

/** invoke の reject 値を UI 表示用の文言に変換する */
export function describeAuthError(error: unknown): string {
  if (isAuthError(error)) return error.message;
  if (typeof error === "string") return error;
  return "Something went wrong. Please try again.";
}

export const login = (username: string, password: string) => invoke<LoginResult>("auth_login", { username, password });

export const verifyTwoFactor = (method: TwoFactorMethod, code: string) =>
  invoke<CurrentUser>("auth_verify_two_factor", { method, code });

export const restoreSession = () => invoke<CurrentUser | null>("auth_restore_session");

export const logout = () => invoke<void>("auth_logout");
