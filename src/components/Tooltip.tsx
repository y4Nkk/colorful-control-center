import * as RadixTooltip from "@radix-ui/react-tooltip";
import { useCallback, useContext, useEffect, useRef, useState, type ReactElement, type ReactNode } from "react";
import { WorkspaceActivity } from "./Dialog";

export function TooltipProvider({ children }: { children: ReactNode }) {
  return <RadixTooltip.Provider delayDuration={150} disableHoverableContent>{children}</RadixTooltip.Provider>;
}

interface TooltipProps {
  content?: string;
  side?: RadixTooltip.TooltipContentProps["side"];
  children: ReactElement;
}

function TooltipLayer({ content, side, container }: Pick<TooltipProps, "content" | "side"> & { container: HTMLElement | null }) {
  const [ready, setReady] = useState(false);
  useEffect(() => {
    // Mount Radix's scroll listener after any scrolling caused by focus.
    const frame = requestAnimationFrame(() => setReady(true));
    return () => cancelAnimationFrame(frame);
  }, []);

  return ready && <RadixTooltip.Portal container={container}>
    <RadixTooltip.Content className="ccc-tooltip" side={side} sideOffset={6} collisionPadding={8}>
      {content}
    </RadixTooltip.Content>
  </RadixTooltip.Portal>;
}

export function Tooltip({ content, side = "top", children }: TooltipProps) {
  const available = useContext(WorkspaceActivity) && Boolean(content);
  const anchor = useRef<Element | null>(null);
  const setAnchor = useCallback((node: Element | null) => { anchor.current = node; }, []);
  const [popup, setPopup] = useState<{ container: HTMLElement | null } | null>(null);
  const open = available && popup !== null;

  useEffect(() => {
    if (!popup) return;
    const dismiss = () => setPopup(null);
    if (!available) { dismiss(); return; }
    window.addEventListener("blur", dismiss);
    return () => window.removeEventListener("blur", dismiss);
  }, [available, popup]);

  return <RadixTooltip.Root open={open} onOpenChange={visible => {
    // Modal dialogs and popovers render above the body, including body portals.
    setPopup(visible && available ? { container: anchor.current?.closest<HTMLElement>("dialog[open], :popover-open") ?? null } : null);
  }}>
    <RadixTooltip.Trigger asChild ref={setAnchor}>{children}</RadixTooltip.Trigger>
    {open && <TooltipLayer content={content} side={side} container={popup.container} />}
  </RadixTooltip.Root>;
}
