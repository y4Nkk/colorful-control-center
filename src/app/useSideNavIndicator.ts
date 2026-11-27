import { useLayoutEffect, useRef, type RefObject } from "react";
import type { PageId } from "./navigation";

interface Frame { top: number; bottom: number }

export function useSideNavIndicator(list: RefObject<HTMLUListElement | null>, current: PageId | null) {
  const indicator = useRef<HTMLLIElement>(null);
  const painted = useRef<Frame | null>(null);

  useLayoutEffect(() => {
    const node = indicator.current;
    const row = list.current?.querySelector('[aria-current="page"]')?.closest("li");
    if (!current || !node || !row) { painted.current = null; return; }

    const style = getComputedStyle(node);
    const height = Number.parseFloat(style.getPropertyValue("--ccc-rail-indicator-height"));
    const duration = Number(style.getPropertyValue("--ccc-motion-rail-indicator-ms"));
    // Fixed-height rows use list-relative layout coordinates, independent of scrolling and zoom.
    const top = row.offsetTop + (row.offsetHeight - height) / 2;
    const target = { top, bottom: top + height };
    const paint = (frame: Frame) => {
      painted.current = frame;
      node.style.top = `${frame.top}px`;
      node.style.height = `${frame.bottom - frame.top}px`;
    };
    const from = painted.current;
    if (!from || !duration || document.hidden || (from.top === target.top && from.bottom === target.bottom)) {
      paint(target);
      return;
    }

    const reducedMotion = matchMedia("(prefers-reduced-motion: reduce)");
    const cleanup = () => {
      cancel();
      preferences.disconnect();
      reducedMotion.removeEventListener("change", checkMotion);
      document.removeEventListener("visibilitychange", settle);
      window.removeEventListener("blur", settle);
    };
    const settle = () => { cleanup(); paint(target); };
    const checkMotion = () => {
      if (reducedMotion.matches || document.documentElement.dataset.motion === "reduce") settle();
    };
    const cancel = animateTravel(from, target, duration, paint, cleanup);
    const preferences = new MutationObserver(checkMotion);
    preferences.observe(document.documentElement, { attributes: true, attributeFilter: ["data-motion"] });
    reducedMotion.addEventListener("change", checkMotion);
    document.addEventListener("visibilitychange", settle);
    window.addEventListener("blur", settle);
    return cleanup;
  }, [current, list]);

  return indicator;
}

function animateTravel(from: Frame, to: Frame, duration: number, paint: (frame: Frame) => void, finish: () => void) {
  const start = performance.now();
  const down = to.top + to.bottom >= from.top + from.bottom;
  const step = (now: number) => {
    const progress = Math.min(1, (now - start) / duration);
    const lead = 1 - (1 - Math.min(1, progress / 0.6)) ** 3;
    const tail = Math.max(0, (progress - 0.35) / 0.65);
    const trail = tail < 0.5 ? 4 * tail ** 3 : 1 - (-2 * tail + 2) ** 3 / 2;
    paint({
      top: from.top + (to.top - from.top) * (down ? trail : lead),
      bottom: from.bottom + (to.bottom - from.bottom) * (down ? lead : trail),
    });
    if (progress < 1) request = requestAnimationFrame(step);
    else finish();
  };
  let request = requestAnimationFrame(step);
  return () => cancelAnimationFrame(request);
}
