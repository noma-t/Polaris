import { useEffect, useState, type RefObject } from "react";

/** ResizeObserver で要素の border-box height を監視する */
export function useElementHeight(ref: RefObject<HTMLElement | null>, fallback: number): number {
  const [height, setHeight] = useState(fallback);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const observer = new ResizeObserver((entries) => setHeight(entries[0].borderBoxSize[0].blockSize));
    observer.observe(el);
    return () => observer.disconnect();
  }, [ref]);
  return height;
}
