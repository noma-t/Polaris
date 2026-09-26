import { useCallback, useEffect, useState } from "react";
import { loadUserSettings, saveUserSettings } from "../lib/settings";

type IdSet = Record<string, boolean>;

/** 連続した操作をまとめて保存するための待ち時間 */
const SAVE_DEBOUNCE_MS = 300;

const toIdSet = (ids: string[]): IdSet => Object.fromEntries(ids.map((id) => [id, true]));
const toIds = (set: IdSet) => Object.keys(set).filter((id) => set[id]);
const toggleId = (set: IdSet, id: string): IdSet => ({ ...set, [id]: !set[id] });

/**
 * pinned friend・表示する group・折りたたんだ group を userId ごとに読み込み、変更を保存する。
 * 読み込み完了前は保存しない (空の状態で上書きしないため)。
 */
export function useUserSettings(userId: string | null) {
  const [pinnedFriendIds, setPinnedFriendIds] = useState<IdSet>({});
  const [shownGroupIds, setShownGroupIds] = useState<IdSet>({});
  const [collapsedGroupIds, setCollapsedGroupIds] = useState<IdSet>({});
  const [loadedUserId, setLoadedUserId] = useState<string | null>(null);

  useEffect(() => {
    setPinnedFriendIds({});
    setShownGroupIds({});
    setCollapsedGroupIds({});
    setLoadedUserId(null);
    if (!userId) return;

    let isCancelled = false;
    loadUserSettings(userId)
      .then((settings) => {
        if (isCancelled) return;
        setPinnedFriendIds(toIdSet(settings.pinnedFriendIds));
        setShownGroupIds(toIdSet(settings.shownGroupIds));
        setCollapsedGroupIds(toIdSet(settings.collapsedGroupIds));
      })
      .catch(() => {
        // 読み込めない場合は空の設定から始める
      })
      .finally(() => {
        if (!isCancelled) setLoadedUserId(userId);
      });
    return () => {
      isCancelled = true;
    };
  }, [userId]);

  useEffect(() => {
    if (!userId || loadedUserId !== userId) return;
    const timer = setTimeout(() => {
      saveUserSettings(userId, {
        pinnedFriendIds: toIds(pinnedFriendIds),
        shownGroupIds: toIds(shownGroupIds),
        collapsedGroupIds: toIds(collapsedGroupIds),
      }).catch(() => {
        // 次回の変更時に再度保存される
      });
    }, SAVE_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [userId, loadedUserId, pinnedFriendIds, shownGroupIds, collapsedGroupIds]);

  const togglePinnedFriend = useCallback((friendId: string) => setPinnedFriendIds((s) => toggleId(s, friendId)), []);

  /** 表示に切り替えた group は展開した状態から始める */
  const toggleShownGroup = useCallback((groupId: string) => {
    setShownGroupIds((s) => toggleId(s, groupId));
    setCollapsedGroupIds((s) => (s[groupId] ? { ...s, [groupId]: false } : s));
  }, []);

  const toggleGroupCollapsed = useCallback((groupId: string) => setCollapsedGroupIds((s) => toggleId(s, groupId)), []);

  return {
    pinnedFriendIds,
    shownGroupIds,
    collapsedGroupIds,
    togglePinnedFriend,
    toggleShownGroup,
    toggleGroupCollapsed,
  };
}
