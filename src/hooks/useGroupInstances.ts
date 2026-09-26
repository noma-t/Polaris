import { useCallback, useEffect, useRef, useState } from "react";
import { describeAuthError, isAuthError } from "../lib/auth";
import { getGroupInstances, getInstancesOfGroup, type GroupInstance } from "../lib/social";

export type SortKey = "users" | "created";
export interface SortState {
  key: SortKey;
  usersDir: "desc" | "asc";
  createdDir: "new" | "old";
}

/** 手動更新の全体共通クールダウン (秒) */
export const REFRESH_COOLDOWN_SEC = 3;
const AUTO_REFRESH_BASE_MS = 60_000;
const AUTO_REFRESH_JITTER_MS = 15_000;

interface UseGroupInstancesOptions {
  isSignedIn: boolean;
  /** 表示中かつ展開中のグループが 1 つ以上ある */
  hasOpenGroup: boolean;
  onError: (message: string) => void;
}

const groupByGroupId = (instances: GroupInstance[]) => {
  const byGroup: Record<string, GroupInstance[]> = {};
  for (const instance of instances) (byGroup[instance.groupId] ??= []).push(instance);
  return byGroup;
};

/**
 * グループインスタンスの取得・自動更新・手動更新クールダウン・ソートの状態を管理する (開閉状態は useUserSettings が保存する)。
 * 全グループ分を 1 リクエストで取得し、展開中のグループがありアプリ表示中の間だけ自動更新する。
 * 画面切り替えで状態が失われないよう App 直下で使う。
 */
export function useGroupInstances({ isSignedIn, hasOpenGroup, onError }: UseGroupInstancesOptions) {
  const [sort, setSort] = useState<SortState>({ key: "users", usersDir: "desc", createdDir: "new" });
  const [cooldownUntil, setCooldownUntil] = useState(0);
  /** groupId → インスタンス。未取得なら null */
  const [instancesByGroup, setInstancesByGroup] = useState<Record<string, GroupInstance[]> | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  /** 未取得のまま取得に失敗した */
  const [hasLoadFailed, setHasLoadFailed] = useState(false);
  const [updatedAt, setUpdatedAt] = useState<number | null>(null);
  const isFetchingRef = useRef(false);
  const nextAutoRef = useRef(0);
  /** reset のたびに進め、サインアウト前に始まった取得の結果を捨てる */
  const sessionRef = useRef(0);
  const onErrorRef = useRef(onError);
  onErrorRef.current = onError;

  /** `groupId` を指定するとそのグループだけ取得して差し替える。省略時は全グループ分を取得する */
  const fetchInstances = useCallback(async (shouldReportError: boolean, groupId?: string) => {
    if (isFetchingRef.current) return;
    isFetchingRef.current = true;
    const session = sessionRef.current;
    setIsLoading(true);
    try {
      if (groupId === undefined) {
        const instances = await getGroupInstances();
        if (session !== sessionRef.current) return;
        setInstancesByGroup(groupByGroupId(instances));
        setHasLoadFailed(false);
      } else {
        const instances = await getInstancesOfGroup(groupId);
        if (session !== sessionRef.current) return;
        setInstancesByGroup((prev) => prev && { ...prev, [groupId]: instances });
      }
      setUpdatedAt(Date.now());
    } catch (error) {
      if (session !== sessionRef.current) return;
      // 1 グループの取得失敗では取得済みの表示を残す
      if (groupId === undefined) setHasLoadFailed(true);
      // unauthorized は useSocial の session-expired 通知でサインアウトさせる
      const isUnauthorized = isAuthError(error) && error.kind === "unauthorized";
      if (shouldReportError && !isUnauthorized) onErrorRef.current(describeAuthError(error));
    } finally {
      if (session === sessionRef.current) {
        isFetchingRef.current = false;
        setIsLoading(false);
        // 1 グループの取得ではほかのグループが古いままなので、自動更新の予定は延ばさない
        if (groupId === undefined) {
          nextAutoRef.current = Date.now() + AUTO_REFRESH_BASE_MS + Math.random() * AUTO_REFRESH_JITTER_MS;
        }
      }
    }
  }, []);

  // 初回取得: サインイン中に初めてグループが展開されたとき
  useEffect(() => {
    if (isSignedIn && hasOpenGroup && instancesByGroup === null && !hasLoadFailed) void fetchInstances(true);
  }, [isSignedIn, hasOpenGroup, instancesByGroup, hasLoadFailed, fetchInstances]);

  // 自動更新: グループを展開中かつアプリ表示中のみ
  useEffect(() => {
    if (!isSignedIn || !hasOpenGroup) return;
    const id = setInterval(() => {
      if (document.visibilityState !== "visible") return;
      if (nextAutoRef.current && Date.now() >= nextAutoRef.current) void fetchInstances(false);
    }, 1000);
    return () => clearInterval(id);
  }, [isSignedIn, hasOpenGroup, fetchInstances]);

  /** 押されたグループだけ再取得する。全グループ分が未取得なら全体取得にフォールバックする */
  const refreshGroup = useCallback(
    (groupId: string) => {
      setCooldownUntil(Date.now() + REFRESH_COOLDOWN_SEC * 1000);
      void fetchInstances(true, instancesByGroup === null ? undefined : groupId);
    },
    [fetchInstances, instancesByGroup],
  );

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
    sessionRef.current += 1;
    isFetchingRef.current = false;
    nextAutoRef.current = 0;
    setCooldownUntil(0);
    setInstancesByGroup(null);
    setIsLoading(false);
    setHasLoadFailed(false);
    setUpdatedAt(null);
  }, []);

  return { sort, cooldownUntil, instancesByGroup, isLoading, hasLoadFailed, updatedAt, refreshGroup, selectSort, reset };
}

export type GroupInstancesState = ReturnType<typeof useGroupInstances>;
