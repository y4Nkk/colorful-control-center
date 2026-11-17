import { useEffect, useRef } from "react";
import { toast } from "../components/use-toast";
import type { HotkeyNavigationRequest } from "../api/client";

export function useHotkeyNavigation(active: boolean, request: HotkeyNavigationRequest | null, acknowledge: (id: number) => Promise<boolean>, kind: "keyboard" | "display") {
  const anchor = useRef<HTMLDivElement>(null), handled = useRef<number | null>(null);
  useEffect(() => {
    if (!active || !request?.ready || (request.kind === "keyboard") !== (kind === "keyboard") || handled.current === request.id) return;
    const { id } = request;
    const frame = requestAnimationFrame(() => {
      const target = anchor.current;
      if (!target || handled.current === id) return;
      try {
        target.scrollIntoView({ block: "start", behavior: "auto" });
        target.focus({ preventScroll: true });
      } catch (error) {
        toast({ kind: "error", title: "无法定位设备页面", description: String(error) });
        return;
      }
      handled.current = id;
      void acknowledge(id).then(ok => { if (!ok && handled.current === id) handled.current = null; });
    });
    return () => cancelAnimationFrame(frame);
  }, [active, request, acknowledge, kind]);
  return anchor;
}
