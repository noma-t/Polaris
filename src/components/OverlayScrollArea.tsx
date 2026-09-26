import { useEffect, useRef, type PointerEvent as ReactPointerEvent, type ReactNode } from "react";

const MIN_THUMB_PX = 36;
const HIDE_DELAY_MS = 900;

interface OverlayScrollAreaProps {
  children: ReactNode;
  /** スクロールする viewport 要素に付与する className */
  viewportClassName?: string;
}

/**
 * ネイティブ scrollbar を隠し、レイアウト幅を消費しないオーバーレイ scrollbar を重ねるスクロール領域。
 * thumb はスクロール中・track hover 中・drag 中のみ表示し、それ以外は auto-hide する。
 */
export function OverlayScrollArea({ children, viewportClassName }: OverlayScrollAreaProps) {
  const viewportRef = useRef<HTMLDivElement>(null);
  const trackRef = useRef<HTMLDivElement>(null);
  const thumbRef = useRef<HTMLDivElement>(null);
  const hideTimer = useRef<number>(undefined);
  const isHovering = useRef(false);
  const drag = useRef<{ startY: number; startScrollTop: number } | null>(null);

  const setVisible = (visible: boolean) => trackRef.current?.classList.toggle("is-visible", visible);

  const scheduleHide = () => {
    clearTimeout(hideTimer.current);
    hideTimer.current = window.setTimeout(() => {
      if (!isHovering.current && !drag.current) setVisible(false);
    }, HIDE_DELAY_MS);
  };

  const reveal = () => {
    setVisible(true);
    scheduleHide();
  };

  /** thumb の高さと位置を viewport のスクロール状態に同期する */
  const updateThumb = () => {
    const viewport = viewportRef.current;
    const track = trackRef.current;
    const thumb = thumbRef.current;
    if (!viewport || !track || !thumb) return;
    const { scrollHeight, clientHeight, scrollTop } = viewport;
    const isScrollable = scrollHeight > clientHeight + 1;
    track.classList.toggle("is-scrollable", isScrollable);
    if (!isScrollable) return;
    const trackHeight = track.clientHeight;
    const thumbHeight = Math.max(MIN_THUMB_PX, (clientHeight / scrollHeight) * trackHeight);
    const offset = ((trackHeight - thumbHeight) * scrollTop) / (scrollHeight - clientHeight);
    thumb.style.height = `${thumbHeight}px`;
    thumb.style.transform = `translateY(${offset}px)`;
  };

  useEffect(() => {
    const viewport = viewportRef.current;
    const track = trackRef.current;
    if (!viewport || !track) return;
    const onScroll = () => {
      updateThumb();
      reveal();
    };
    // viewport 自体・track・中身のサイズ変化のいずれでも scrollHeight / thumb 位置が変わるため、すべて監視する
    const resizeObserver = new ResizeObserver(() => updateThumb());
    const observeTargets = () => {
      resizeObserver.disconnect();
      resizeObserver.observe(viewport);
      resizeObserver.observe(track);
      for (const child of Array.from(viewport.children)) resizeObserver.observe(child);
    };
    const mutationObserver = new MutationObserver(() => {
      observeTargets();
      updateThumb();
    });
    observeTargets();
    mutationObserver.observe(viewport, { childList: true, subtree: true, characterData: true });
    viewport.addEventListener("scroll", onScroll, { passive: true });
    updateThumb();
    return () => {
      viewport.removeEventListener("scroll", onScroll);
      resizeObserver.disconnect();
      mutationObserver.disconnect();
      clearTimeout(hideTimer.current);
    };
  }, []);

  /** thumb なら drag 開始、track の空き部分ならクリック位置へジャンプしてそのまま drag を続ける */
  const onTrackPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    const viewport = viewportRef.current;
    const track = trackRef.current;
    const thumb = thumbRef.current;
    if (!viewport || !track || !thumb || e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.target !== thumb) {
      const maxOffset = track.clientHeight - thumb.offsetHeight;
      const y = e.clientY - track.getBoundingClientRect().top;
      if (maxOffset > 0) {
        viewport.scrollTop = ((y - thumb.offsetHeight / 2) / maxOffset) * (viewport.scrollHeight - viewport.clientHeight);
      }
    }
    track.setPointerCapture(e.pointerId);
    drag.current = { startY: e.clientY, startScrollTop: viewport.scrollTop };
    track.classList.add("is-dragging");
    setVisible(true);
  };

  const onTrackPointerMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    const viewport = viewportRef.current;
    const track = trackRef.current;
    const thumb = thumbRef.current;
    if (!drag.current || !viewport || !track || !thumb) return;
    const maxOffset = track.clientHeight - thumb.offsetHeight;
    if (maxOffset <= 0) return;
    const ratio = (viewport.scrollHeight - viewport.clientHeight) / maxOffset;
    viewport.scrollTop = drag.current.startScrollTop + (e.clientY - drag.current.startY) * ratio;
  };

  const onTrackPointerUp = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (!drag.current) return;
    e.currentTarget.releasePointerCapture(e.pointerId);
    drag.current = null;
    trackRef.current?.classList.remove("is-dragging");
    scheduleHide();
  };

  return (
    <div className="overlay-scroll-area">
      <div ref={viewportRef} className={`overlay-scroll-viewport ${viewportClassName ?? ""}`}>
        {children}
      </div>
      <div
        ref={trackRef}
        className="overlay-scrollbar-track"
        aria-hidden="true"
        onPointerDown={onTrackPointerDown}
        onPointerMove={onTrackPointerMove}
        onPointerUp={onTrackPointerUp}
        onPointerCancel={onTrackPointerUp}
        onPointerEnter={() => {
          isHovering.current = true;
          setVisible(true);
          clearTimeout(hideTimer.current);
        }}
        onPointerLeave={() => {
          isHovering.current = false;
          scheduleHide();
        }}
      >
        <div ref={thumbRef} className="overlay-scrollbar-thumb" />
      </div>
    </div>
  );
}
