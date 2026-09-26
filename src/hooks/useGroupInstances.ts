import { useCallback, useEffect, useRef, useState } from "react";

export type SortKey = "users" | "created";
export interface SortState {
  key: SortKey;
  usersDir: "desc" | "asc";
  createdDir: "new" | "old";
}

/** 手動更新の全体共通クールダウン (秒) */
export const REFRESH_COOLDOWN_SEC = 5;
const FIRST_LOAD_DELAY_MS = 800;
const AUTO_REFRESH_BASE_MS = 60_000;
const AUTO_REFRESH_JITTER_MS = 15_000;

/**
 * グループ欄の開閉・初回取得・自動更新・手動更新クールダウン・ソートの状態を管理する (モック)。
 * 画面切り替えで状態が失われないよう App 直下で使う。
 */
export function useGroupInstances({ rateLimited }: { rateLimited: boolean }) {
  const [openIds, setOpenIds] = useState<Record<string, boolean>>({});
  const [loadedIds, setLoadedIds] = useState<Record<string, boolean>>({});
  const [loadingIds, setLoadingIds] = useState<Record<string, boolean>>({});
  const [sort, setSort] = useState<SortState>({ key: "users", usersDir: "desc", createdDir: "new" });
  const [cooldownUntil, setCooldownUntil] = useState(0);
  const [updatedAt, setUpdatedAt] = useState<number | null>(null);
  const nextAutoRef = useRef(0);
  const openIdsRef = useRef(openIds);
  openIdsRef.current = openIds;

  // 自動更新: グループ欄を展開中かつアプリ表示中のみ
  useEffect(() => {
    const id = setInterval(() => {
      const now = Date.now();
      const anyOpen = Object.values(openIdsRef.current).some(Boolean);
      if (!anyOpen || rateLimited || document.visibilityState !== "visible") return;
      if (nextAutoRef.current && now >= nextAutoRef.current) {
        setUpdatedAt(now);
        nextAutoRef.current = now + AUTO_REFRESH_BASE_MS + Math.random() * AUTO_REFRESH_JITTER_MS;
      }
    }, 1000);
    return () => clearInterval(id);
  }, [rateLimited]);

  const toggleGroup = useCallback(
    (groupId: string) => {
      if (openIds[groupId]) {
        setOpenIds((s) => ({ ...s, [groupId]: false }));
        return;
      }
      const isFirstOpen = !loadedIds[groupId];
      setOpenIds((s) => ({ ...s, [groupId]: true }));
      if (!nextAutoRef.current) nextAutoRef.current = Date.now() + AUTO_REFRESH_BASE_MS;
      if (isFirstOpen) {
        setLoadingIds((s) => ({ ...s, [groupId]: true }));
        setTimeout(() => {
          setLoadingIds((s) => ({ ...s, [groupId]: false }));
          setLoadedIds((s) => ({ ...s, [groupId]: true }));
          setUpdatedAt(Date.now());
        }, FIRST_LOAD_DELAY_MS);
      }
    },
    [openIds, loadedIds],
  );

  const refreshGroup = useCallback(() => {
    const now = Date.now();
    setCooldownUntil(now + REFRESH_COOLDOWN_SEC * 1000);
    setUpdatedAt(now);
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
    setOpenIds({});
    setLoadedIds({});
    setLoadingIds({});
    setUpdatedAt(null);
    nextAutoRef.current = 0;
  }, []);

  return { openIds, loadingIds, sort, cooldownUntil, updatedAt, toggleGroup, refreshGroup, selectSort, reset };
}

export type GroupInstancesState = ReturnType<typeof useGroupInstances>;
