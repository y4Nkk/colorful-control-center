import { useEffect } from "react";

/** scroll 不冒泡；在根捕获以统一覆盖工作区与其他原生滚动面。 */
export function useScrollActivityMarker(): void {
  useEffect(() => {
    const idleDelay = Number.parseFloat(getComputedStyle(document.documentElement)
      .getPropertyValue("--ccc-scrollbar-idle-delay"));
    const idleTimers = new Map<Element, number>();
    const onScroll = (event: Event) => {
      const surface = event.target === document ? document.scrollingElement : event.target;
      if (!(surface instanceof Element)) return;
      surface.setAttribute("data-scrolling", "");
      const pending = idleTimers.get(surface);
      if (pending !== undefined) window.clearTimeout(pending);
      idleTimers.set(surface, window.setTimeout(() => {
        idleTimers.delete(surface);
        surface.removeAttribute("data-scrolling");
      }, idleDelay));
    };
    window.addEventListener("scroll", onScroll, { capture: true, passive: true });
    return () => {
      window.removeEventListener("scroll", onScroll, { capture: true });
      for (const [surface, timeout] of idleTimers) {
        window.clearTimeout(timeout);
        surface.removeAttribute("data-scrolling");
      }
      idleTimers.clear();
    };
  }, []);
}
