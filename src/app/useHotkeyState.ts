import { useCallback, useEffect, useRef, useState } from "react";
import { acknowledgeHotkeyNavigation, dismissBatteryColdWarning, onHotkeyStatus, readHotkeyStatus, type BatteryColdWarning, type HotkeyStatus, type HotkeyNavigationRequest } from "../api/client";
import { toast } from "../components/use-toast";

export function useHotkeyState(enabled: boolean) {
  const [warning, setWarning] = useState<BatteryColdWarning | null>(null);
  const [navigationRequest, setNavigationRequest] = useState<HotkeyNavigationRequest | null>(null);
  const latestNavigation = useRef<number | null>(null);
  useEffect(() => {
    if (!enabled) return;
    let disposed = false, received = false, unlisten: (() => void) | undefined;
    const accept = (status: HotkeyStatus) => {
      setWarning(status.coldWarning);
      latestNavigation.current = status.navigationRequest?.id ?? null;
      setNavigationRequest(status.navigationRequest);
    };
    void onHotkeyStatus(status => {
      if (!disposed) { received = true; accept(status); }
    }).then(async off => {
      if (disposed) { off(); return; }
      unlisten = off;
      const current = await readHotkeyStatus();
      if (!disposed && !received) accept(current);
    }).catch(error => { if (!disposed) toast({ kind: "error", title: "热键状态连接失败", description: String(error) }); });
    return () => { disposed = true; unlisten?.(); };
  }, [enabled]);
  const dismiss = async () => {
    if (!warning) return;
    const id = warning.id;
    try {
      await dismissBatteryColdWarning(id);
      setWarning(current => current?.id === id ? null : current);
    } catch (error) { toast({ kind: "error", title: "无法关闭低温提醒", description: String(error) }); }
  };
  const acknowledgeNavigation = useCallback(async (id: number): Promise<boolean> => {
    try {
      await acknowledgeHotkeyNavigation(id);
      if (latestNavigation.current === id) latestNavigation.current = null;
      setNavigationRequest(current => current?.id === id ? null : current);
      return true;
    } catch (error) {
      if (latestNavigation.current === id) toast({ kind: "error", title: "热键导航状态未确认", description: String(error) });
      return false;
    }
  }, []);
  return { warning, dismiss, navigationRequest, acknowledgeNavigation };
}
