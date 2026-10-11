import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { describeAuthError, isAuthError } from "../lib/auth";
import { describeErrorForLog, diagnosticLog } from "../lib/diagnosticLog";
import {
  getGroupInstances,
  onGroupInstancesUpdated,
  refreshGroupInstances,
  setOpenGroups,
  type GroupInstance,
  type GroupInstancesSnapshot,
} from "../lib/social";

export type SortKey = "users" | "created";
export interface SortState {
  key: SortKey;
  usersDir: "desc" | "asc";
  createdDir: "new" | "old";
}

/** 手動更新の全体共通クールダウン (秒) */
export const REFRESH_COOLDOWN_SEC = 3;

interface UseGroupInstancesOptions {
  isSignedIn: boolean;
  /** 表示中かつ展開中のグループ ID。Rust 側の定期取得は、これが空でない間 (かバックグラウンド実行中) だけ動く */
  openGroupIds: string[];
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
 * Rust 側が保持するグループインスタンス一覧の snapshot を購読し、手動更新とそのクールダウン・並び順を扱う
 * (取得と定期取得は Rust 側、開閉状態は useUserSettings、ソートは useUiState が保存する)。
 * 画面切り替えで状態が失われないよう App 直下で使う。
 */
export function useGroupInstances({ isSignedIn, openGroupIds, sort, onSortChange, onError }: UseGroupInstancesOptions) {
  const [cooldownUntil, setCooldownUntil] = useState(0);
  const [snapshot, setSnapshot] = useState<GroupInstancesSnapshot | null>(null);
  const onErrorRef = useRef(onError);
  onErrorRef.current = onError;

  // listen を登録してから現在値を取得し、登録前の emit を取りこぼさないようにする
  useEffect(() => {
    setSnapshot(null);
    setCooldownUntil(0);
    if (!isSignedIn) return;

    let isCancelled = false;
    // 初期取得の値が、その後に届いた新しい emit を上書きしないよう revision で比べる
    const apply = (next: GroupInstancesSnapshot) => {
      if (isCancelled) return;
      setSnapshot((prev) => (prev && prev.revision >= next.revision ? prev : next));
    };
    const subscription = onGroupInstancesUpdated(apply);
    subscription
      .then(() => getGroupInstances())
      .then(apply)
      .catch(() => {
        // 取得に失敗しても以降の emit で更新される
      });

    return () => {
      isCancelled = true;
      subscription.then((unlisten) => unlisten());
    };
  }, [isSignedIn]);

  const openGroupKey = openGroupIds.join(",");
  useEffect(() => {
    if (!isSignedIn) return;
    const groupIds = openGroupKey ? openGroupKey.split(",") : [];
    setOpenGroups(groupIds).catch((error) => {
      diagnosticLog(`group instances: failed to send open groups: ${describeErrorForLog(error)}`);
    });
  }, [isSignedIn, openGroupKey]);

  /** 押されたグループだけ再取得させる。全グループ分が未取得なら Rust 側が全体取得にフォールバックする */
  const refreshGroup = useCallback((groupId: string) => {
    setCooldownUntil(Date.now() + REFRESH_COOLDOWN_SEC * 1000);
    refreshGroupInstances(groupId).catch((error) => {
      // unauthorized は useSocial の session-expired 通知でサインアウトさせる
      const isUnauthorized = isAuthError(error) && error.kind === "unauthorized";
      if (!isUnauthorized) onErrorRef.current(describeAuthError(error));
    });
  }, []);

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

  const instances = snapshot?.instances ?? null;
  /** groupId → インスタンス。未取得なら null */
  const instancesByGroup = useMemo(() => instances && groupByGroupId(instances), [instances]);

  return {
    sort,
    cooldownUntil,
    instancesByGroup,
    isLoading: snapshot?.isLoading ?? false,
    hasLoadFailed: snapshot?.hasLoadFailed ?? false,
    updatedAt: snapshot?.updatedAt ?? null,
    refreshGroup,
    selectSort,
  };
}

export type GroupInstancesState = ReturnType<typeof useGroupInstances>;
