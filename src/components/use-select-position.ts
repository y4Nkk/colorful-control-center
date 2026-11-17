import { useLayoutEffect, useState, type RefObject } from "react";
import { selectLayout } from "../lib/select-layout";

type ElementRef = RefObject<HTMLElement | null>;

export function useSelectPosition(trigger: ElementRef, menu: ElementRef, list: ElementRef, dismiss: () => void) {
  const [position, setPosition] = useState<ReturnType<typeof selectLayout> | null>(null);
  useLayoutEffect(() => {
    const anchor = trigger.current, popup = menu.current, options = list.current;
    if (!anchor || !popup || !options) return;
    let frame = 0;
    const update = () => {
      frame = 0;
      const box = anchor.getBoundingClientRect();
      // Portal 直接挂 body；原生 WebView 缩放已包含在视口坐标中。
      const scale = Number(getComputedStyle(document.documentElement).zoom) * Number(getComputedStyle(document.body).zoom);
      const styles = getComputedStyle(popup);
      const read = (name: string) => parseFloat(styles.getPropertyValue(name)) * scale;
      const viewport = popup.querySelector<HTMLElement>(".ccc-scroll-viewport")!;
      const naturalHeight = (popup.offsetHeight - viewport.clientHeight
        + Math.min(options.scrollHeight, read("--ccc-select-list-height") / scale)) * scale;
      const next = selectLayout({ anchor: box, viewport: { width: innerWidth, height: innerHeight }, naturalHeight,
        minWidth: read("--ccc-select-min-width"), maxWidth: read("--ccc-select-max-width"),
        gap: read("--ccc-select-gap"), edge: read("--ccc-select-edge"), scale });
      setPosition(previous => previous && Object.keys(next).every(key => previous[key as keyof typeof next] === next[key as keyof typeof next]) ? previous : next);
    };
    const schedule = () => { if (!frame) frame = requestAnimationFrame(update); };
    update();
    const resize = new ResizeObserver(schedule);
    resize.observe(anchor); resize.observe(popup); resize.observe(options);
    const visibility = new IntersectionObserver(entries => { if (!entries[0].isIntersecting) dismiss(); });
    visibility.observe(anchor);
    window.addEventListener("resize", schedule);
    document.addEventListener("scroll", schedule, true);
    return () => {
      cancelAnimationFrame(frame); resize.disconnect(); visibility.disconnect();
      window.removeEventListener("resize", schedule);
      document.removeEventListener("scroll", schedule, true);
    };
  }, [trigger, menu, list, dismiss]);
  return position;
}
