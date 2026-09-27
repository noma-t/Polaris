import { useCallback, useEffect, useRef, useState } from "react";
import { getDummyInstanceDetail } from "../data/thumbnailFriends";
import { describeAuthError, isAuthError } from "../lib/auth";
import { getInstanceDetail, type InstanceDetail } from "../lib/social";
import { REFRESH_COOLDOWN_SEC } from "./useGroupInstances";

/**
 * フレンドがいるインスタンスの詳細を取得する。location が変わるたび (展開時・フレンドの移動時) に取得し直し、
 * 手動更新は取得中とクールダウン中は受け付けない。
 */
export function useInstanceDetail(location: string) {
  const [detail, setDetail] = useState<InstanceDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isCoolingDown, setIsCoolingDown] = useState(false);
  /** 取得のたびに進め、location の変更やアンマウントより前に始まった取得の結果を捨てる */
  const requestRef = useRef(0);
  const cooldownTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const load = useCallback(async () => {
    const request = ++requestRef.current;
    setIsLoading(true);
    try {
      // Thumbnail mode のダミーインスタンスは API を呼ばない
      const result = getDummyInstanceDetail(location) ?? (await getInstanceDetail(location));
      if (request !== requestRef.current) return;
      setDetail(result);
      setError(null);
    } catch (err) {
      if (request !== requestRef.current) return;
      // unauthorized は useSocial の session-expired 通知でサインアウトさせる
      const isUnauthorized = isAuthError(err) && err.kind === "unauthorized";
      setError(isUnauthorized ? null : describeAuthError(err));
    } finally {
      if (request === requestRef.current) setIsLoading(false);
    }
  }, [location]);

  useEffect(() => {
    setDetail(null);
    setError(null);
    void load();
    return () => {
      requestRef.current += 1;
    };
  }, [load]);

  useEffect(
    () => () => {
      if (cooldownTimerRef.current) clearTimeout(cooldownTimerRef.current);
    },
    [],
  );

  const refresh = useCallback(() => {
    if (isLoading || isCoolingDown) return;
    setIsCoolingDown(true);
    cooldownTimerRef.current = setTimeout(() => setIsCoolingDown(false), REFRESH_COOLDOWN_SEC * 1000);
    void load();
  }, [isLoading, isCoolingDown, load]);

  return { detail, error, isLoading, isCoolingDown, refresh };
}
