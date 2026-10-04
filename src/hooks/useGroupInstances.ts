import { useCallback, useEffect, useRef, useState } from "react";
import { describeAuthError, isAuthError } from "../lib/auth";
import { describeErrorForLog, diagnosticLog } from "../lib/diagnosticLog";
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
/** 自動更新のタイマー (1 秒周期) の間隔がこれ以上空いたら、タイマーが凍結・間引きされたとしてログに残す */
const TIMER_TICK_GAP_NOTICE_MS = 5_000;
/** 取得が終わらないままこれ以上経ったら、止まっているとしてログに残す */
const FETCH_STUCK_NOTICE_MS = 15_000;

/** fetchedAt から次の自動更新時刻を決める。時計がずれていても [最低限空ける時間, 取得間隔] 後に収める */
const nextAutoRefreshAt = (fetchedAt: number, now: number) =>
  now + Math.min(AUTO_REFRESH_INTERVAL_MS, Math.max(AUTO_REFRESH_MIN_DELAY_MS, fetchedAt + AUTO_REFRESH_INTERVAL_MS - now));

/** ログ用: ms を秒 (小数 1 桁) にする */
const seconds = (ms: number) => (ms / 1000).toFixed(1);

/** 自動更新タイマーの 1 回ごとの判定。waiting / due 以外は異常として、変化したときだけログに残す */
type AutoRefreshTickState = "waiting" | "due" | "hidden" | "unscheduled" | "stuck";

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
  /** 実行中の取得を始めた時刻 (ログ用) */
  const fetchStartedAtRef = useRef(0);
  const nextAutoRef = useRef(0);
  /** 手動更新で最新のデータを取得した時刻 (取得開始時)。これより古い fetchedAt の全体取得の結果は反映しない。0 なら制限なし */
  const manualRefreshedAtRef = useRef(0);
  /** reset のたびに進め、サインアウト前に始まった取得の結果を捨てる */
  const sessionRef = useRef(0);
  const onErrorRef = useRef(onError);
  onErrorRef.current = onError;

  /** `groupId` を指定するとそのグループだけ取得して差し替える。省略時は全グループ分を取得する */
  const fetchInstances = useCallback(async (shouldReportError: boolean, groupId?: string) => {
    // 取得中の呼び出しは、毎秒の自動更新タイマーからも来るので、ここではログに残さない (タイマー側が状態の変化として残す)
    if (isFetchingRef.current) return;
    isFetchingRef.current = true;
    fetchStartedAtRef.current = Date.now();
    const session = sessionRef.current;
    const scope = groupId ?? "all groups";
    /** 取得に失敗したときは完了時刻を基準にする */
    let fetchedAt: number | null = null;
    setIsLoading(true);
    diagnosticLog(`group instances: fetch (${scope}) started`);
    try {
      if (groupId === undefined) {
        const list = await getGroupInstances();
        if (session !== sessionRef.current) {
          diagnosticLog(`group instances: fetch (${scope}) result discarded; the session changed while fetching`);
          return;
        }
        fetchedAt = list.fetchedAt;
        // 手動更新より古いキャッシュが返ってきたときは、新しいデータを上書きしないよう捨てて再試行を待つ
        if (fetchedAt < manualRefreshedAtRef.current) {
          diagnosticLog(
            `group instances: fetch (${scope}) result discarded; fetchedAt is ${seconds(manualRefreshedAtRef.current - fetchedAt)}s older than the last manual refresh`,
          );
          return;
        }
        manualRefreshedAtRef.current = 0;
        setInstancesByGroup(groupByGroupId(list.instances));
        setHasLoadFailed(false);
        diagnosticLog(
          `group instances: fetch (${scope}) ok, ${list.instances.length} instances, fetchedAt age ${seconds(Date.now() - fetchedAt)}s, took ${seconds(Date.now() - fetchStartedAtRef.current)}s`,
        );
      } else {
        const startedAt = Date.now();
        const instances = await getInstancesOfGroup(groupId);
        if (session !== sessionRef.current) {
          diagnosticLog(`group instances: fetch (${scope}) result discarded; the session changed while fetching`);
          return;
        }
        manualRefreshedAtRef.current = startedAt;
        setInstancesByGroup((prev) => prev && { ...prev, [groupId]: instances });
        diagnosticLog(`group instances: fetch (${scope}) ok, ${instances.length} instances, took ${seconds(Date.now() - startedAt)}s`);
      }
      setUpdatedAt(Date.now());
    } catch (error) {
      if (session !== sessionRef.current) {
        diagnosticLog(`group instances: fetch (${scope}) failed after the session changed: ${describeErrorForLog(error)}`);
        return;
      }
      diagnosticLog(
        `group instances: fetch (${scope}) failed after ${seconds(Date.now() - fetchStartedAtRef.current)}s: ${describeErrorForLog(error)} (reported to the user: ${shouldReportError})`,
      );
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
          diagnosticLog(`auto-refresh: next in ${seconds(nextAutoRef.current - now)}s`);
        }
      }
    }
  }, []);

  // 初回取得: サインイン中に初めてグループが展開されたとき
  useEffect(() => {
    if (isSignedIn && hasOpenGroup && instancesByGroup === null && !hasLoadFailed) {
      diagnosticLog("group instances: initial fetch triggered");
      void fetchInstances(true);
    }
  }, [isSignedIn, hasOpenGroup, instancesByGroup, hasLoadFailed, fetchInstances]);

  // 自動更新: グループを展開中かつアプリ表示中のみ
  useEffect(() => {
    if (!isSignedIn || !hasOpenGroup) {
      diagnosticLog(`auto-refresh: timer off (signedIn=${isSignedIn} hasOpenGroup=${hasOpenGroup})`);
      return;
    }
    const nextIn = nextAutoRef.current ? seconds(nextAutoRef.current - Date.now()) : "unscheduled";
    diagnosticLog(`auto-refresh: timer on (next in ${nextIn}s, visibility=${document.visibilityState})`);
    let lastTickAt = Date.now();
    let lastState: AutoRefreshTickState = "waiting";
    const id = setInterval(() => {
      const now = Date.now();
      const gapMs = now - lastTickAt;
      lastTickAt = now;
      if (gapMs >= TIMER_TICK_GAP_NOTICE_MS) {
        diagnosticLog(`auto-refresh: timer tick was delayed ${seconds(gapMs)}s (timer throttled, or the app/PC was suspended?)`);
      }

      const isDue = nextAutoRef.current !== 0 && now >= nextAutoRef.current;
      let state: AutoRefreshTickState;
      if (document.visibilityState !== "visible") state = "hidden";
      else if (nextAutoRef.current === 0) state = "unscheduled";
      else if (!isDue) state = "waiting";
      else if (isFetchingRef.current && now - fetchStartedAtRef.current >= FETCH_STUCK_NOTICE_MS) state = "stuck";
      else state = "due";

      // 通常の waiting / due は毎周期の動きなので残さない。異常な状態に入ったときと、そこから戻ったときだけ残す
      const isNormal = (s: AutoRefreshTickState) => s === "waiting" || s === "due";
      if (state !== lastState && !(isNormal(state) && isNormal(lastState))) {
        const detail =
          state === "stuck"
            ? `previous fetch has been running for ${seconds(now - fetchStartedAtRef.current)}s`
            : `next in ${seconds(nextAutoRef.current - now)}s, visibility=${document.visibilityState}`;
        diagnosticLog(`auto-refresh: ${lastState} -> ${state} (${detail})`);
      }
      lastState = state;

      if (state === "due" && !isFetchingRef.current) {
        diagnosticLog(`auto-refresh: due, overdue by ${seconds(now - nextAutoRef.current)}s; fetching`);
        void fetchInstances(false);
      }
    }, 1000);
    return () => {
      clearInterval(id);
      diagnosticLog("auto-refresh: timer cleared");
    };
  }, [isSignedIn, hasOpenGroup, fetchInstances]);

  /** 押されたグループだけ再取得する。全グループ分が未取得なら全体取得にフォールバックする */
  const refreshGroup = useCallback(
    (groupId: string) => {
      setCooldownUntil(Date.now() + REFRESH_COOLDOWN_SEC * 1000);
      const target = instancesByGroup === null ? undefined : groupId;
      diagnosticLog(
        `manual refresh: ${groupId}, fetching ${target ?? "all groups (nothing loaded yet)"}${isFetchingRef.current ? "; skipped because a fetch is already running" : ""}`,
      );
      void fetchInstances(true, target);
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
    diagnosticLog(`group instances: reset (fetching=${isFetchingRef.current})`);
    sessionRef.current += 1;
    isFetchingRef.current = false;
    nextAutoRef.current = 0;
    manualRefreshedAtRef.current = 0;
    setCooldownUntil(0);
    setInstancesByGroup(null);
    setIsLoading(false);
    setHasLoadFailed(false);
    setUpdatedAt(null);
  }, []);

  return { sort, cooldownUntil, instancesByGroup, isLoading, hasLoadFailed, updatedAt, refreshGroup, selectSort, reset };
}

export type GroupInstancesState = ReturnType<typeof useGroupInstances>;
