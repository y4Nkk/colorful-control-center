/** 硬件写入的统一两步确认；执行中阻止重复触发。 */
import { Button } from "./Button";
import type { IconName } from "./Icon";
import { useEffect, useRef, useState } from "react";
import { toast } from "./use-toast";

export function ConfirmButton({ label, confirmLabel, onConfirm, disabled = false, busy = false, variant = "primary", icon = "apply" }: {
  label: string;
  confirmLabel: string;
  onConfirm: () => void | Promise<void>;
  disabled?: boolean;
  busy?: boolean;
  variant?: "primary" | "secondary";
  icon?: IconName;
}) {
  const [arming, setArming] = useState(false);
  const [pending, setPending] = useState(false);
  const executing = useRef(false);
  useEffect(() => { setArming(false); }, [label, confirmLabel, disabled]);
  useEffect(() => {
    if (!arming) return;
    const timer = setTimeout(() => setArming(false), 3000);
    return () => clearTimeout(timer);
  }, [arming]);
  const confirm = async () => {
    if (disabled || busy || executing.current) return;
    if (!arming) { setArming(true); return; }
    setArming(false);
    executing.current = true;
    setPending(true);
    try { await onConfirm(); }
    catch (error) { toast({ kind: "error", title: "操作失败", description: String(error) }); }
    finally { executing.current = false; setPending(false); }
  };
  return (
    <Button variant={variant} icon={busy || pending ? "loading" : arming ? "confirm" : icon} tone={arming ? "danger" : "normal"} type="button" disabled={disabled || busy || pending} data-confirming={arming}
      aria-busy={busy || pending}
      onClick={() => { void confirm(); }}>
      {busy || pending ? "执行中…" : arming ? confirmLabel : label}
    </Button>
  );
}
