import type { RefObject } from "react";
import { Button } from "./Button";
import { Tooltip } from "./Tooltip";

export function BackToTopButton({ viewportRef }: { viewportRef: RefObject<HTMLDivElement | null> }) {
  function returnToTop() {
    const viewport = viewportRef.current;
    if (!viewport) return;
    const reduceMotion = document.documentElement.dataset.motion === "reduce"
      || window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    viewport.focus({ preventScroll: true });
    viewport.scrollTo({ top: 0, behavior: reduceMotion ? "instant" : "smooth" });
  }

  return <Tooltip content="返回顶部">
    <Button className="ccc-back-to-top" variant="secondary" size="icon" icon="back-to-top"
      aria-label="返回顶部" onClick={returnToTop} />
  </Tooltip>;
}
