import { useCallback, useState } from "react";

export type SortKey = "users" | "created";
export interface SortState {
  key: SortKey;
  usersDir: "desc" | "asc";
  createdDir: "new" | "old";
}

/** 手動更新の全体共通クールダウン (秒) */
export const REFRESH_COOLDOWN_SEC = 5;

/**
 * グループ欄の手動更新クールダウン・ソートの状態を管理する (開閉状態は useUserSettings が保存する)。
 * インスタンス取得は未実装のため、取得・自動更新の処理はまだ持たない。
 * 画面切り替えで状態が失われないよう App 直下で使う。
 */
export function useGroupInstances() {
  const [sort, setSort] = useState<SortState>({ key: "users", usersDir: "desc", createdDir: "new" });
  const [cooldownUntil, setCooldownUntil] = useState(0);

  const refreshGroup = useCallback(() => {
    setCooldownUntil(Date.now() + REFRESH_COOLDOWN_SEC * 1000);
  }, []);

  /** 同じキーなら方向を反転、別キーならキーだけ切り替える */
  const selectSort = useCallback((key: SortKey) => {
    setSort((s) => {
      if (s.key !== key) return { ...s, key };
      return key === "users"
        ? { ...s, usersDir: s.usersDir === "desc" ? "asc" : "desc" }
        : { ...s, createdDir: s.createdDir === "new" ? "old" : "new" };
    });
  }, []);

  const reset = useCallback(() => {
    setCooldownUntil(0);
  }, []);

  return { sort, cooldownUntil, refreshGroup, selectSort, reset };
}

export type GroupInstancesState = ReturnType<typeof useGroupInstances>;
