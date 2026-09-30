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
/** fetchedAt から次の自動更新までの間隔 (Rust 側のバックグラウンド取得と揃える) */
const AUTO_REFRESH_INTERVAL_MS = 120_000;
/** 端末と VRChat の時計のずれで fetchedAt + 間隔 が過ぎていても、連続取得しないよう最低限空ける時間 */
const AUTO_REFRESH_MIN_DELAY_MS = 10_000;

/** fetchedAt から次の自動更新時刻を決める。時計がずれていても [最低限空ける時間, 取得間隔] 後に収める */
const nextAutoRefreshAt = (fetchedAt: number, now: number) =>
  now + Math.min(AUTO_REFRESH_INTERVAL_MS, Math.max(AUTO_REFRESH_MIN_DELAY_MS, fetchedAt + AUTO_REFRESH_INTERVAL_MS - now));

interface UseGroupInstancesOptions {
  isSignedIn: boolean;
  /** 表示中かつ展開中のグループが 1 つ以上ある */
  hasOpenGroup: boolean;
  /** 保存済みの並び順 (useUiState が保存する) */
  sort: SortState;
  onSortChange: (sort: SortState) => void;
  onError: (message: string) => void;
}

const groupByGroupId = (instances: GroupInstance[]) => {
  const byGroup: Record<string, GroupInstance[]> = {};
  for (const instance of instances) (byGroup[instance.groupId] ??= []).push(instance);
  return byGroup;
};

/**
 * グループインスタンスの取得・自動更新・手動更新クールダウンを管理する (開閉状態は useUserSettings、ソートは useUiState が保存する)。
 * 全グループ分を 1 リクエストで取得し、展開中のグループがありアプリ表示中の間だけ自動更新する。手動更新は押されたグループだけを取得する。
 * 画面切り替えで状態が失われないよう App 直下で使う。
 */
export function useGroupInstances({ isSignedIn, hasOpenGroup, sort, onSortChange, onError }: UseGroupInstancesOptions) {
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
    /** 取得に失敗したときは完了時刻を基準にする */
    let fetchedAt: number | null = null;
    setIsLoading(true);
    try {
      if (groupId === undefined) {
        const list = await getGroupInstances();
        if (session !== sessionRef.current) return;
        fetchedAt = list.fetchedAt;
        setInstancesByGroup(groupByGroupId(list.instances));
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
          const now = Date.now();
          nextAutoRef.current = nextAutoRefreshAt(fetchedAt ?? now, now);
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
  const selectSort = useCallback(
    (key: SortKey) => {
      if (sort.key !== key) return onSortChange({ ...sort, key });
      onSortChange(
        key === "users"
          ? { ...sort, usersDir: sort.usersDir === "desc" ? "asc" : "desc" }
          : { ...sort, createdDir: sort.createdDir === "new" ? "old" : "new" },
      );
    },
    [sort, onSortChange],
  );

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
