import type { HTMLAttributes, Ref } from "react";
import { Surface, type SurfaceVariant } from "./Surface";

interface ScrollAreaProps extends HTMLAttributes<HTMLElement> {
  as?: "div" | "main" | "section";
  surface?: SurfaceVariant;
  viewportRef?: Ref<HTMLDivElement>;
}

/** 外框负责布局与裁切，viewport 负责原生滚动和键盘焦点。 */
export function ScrollArea({ as: Tag = "div", surface = "plain", className, tabIndex = 0,
  onScroll, viewportRef, children, ...props }: ScrollAreaProps) {
  return (
    <Surface {...props} as={Tag} variant={surface} className={["ccc-scroll-area", className].filter(Boolean).join(" ")}>
      <div ref={viewportRef} className="ccc-scroll-viewport" tabIndex={tabIndex}
        aria-label={props["aria-label"]} onScroll={onScroll}>
        {children}
      </div>
    </Surface>
  );
}
