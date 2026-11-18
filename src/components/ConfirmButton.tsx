import { Button } from "./Button";
import { Dialog } from "./Dialog";
import type { IconName } from "./Icon";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { toast } from "./use-toast";

export function ConfirmButton({ label, confirmLabel, onConfirm, disabled = false, busy = false, variant = "secondary", icon = "apply", children }: {
  label: string;
  confirmLabel: string;
  onConfirm: () => void | boolean | Promise<void | boolean>;
  disabled?: boolean;
  busy?: boolean;
  variant?: "primary" | "secondary";
  icon?: IconName;
  children?: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const [pending, setPending] = useState(false);
  const executing = useRef(false);
  useEffect(() => { setOpen(false); }, [label, confirmLabel, disabled, busy]);
  const confirm = async () => {
    if (!open || disabled || busy || executing.current) return;
    executing.current = true;
    setPending(true);
    setOpen(false);
    try { await onConfirm(); }
    catch (error) { toast({ kind: "error", title: "操作失败", description: String(error) }); }
    finally { executing.current = false; setPending(false); }
  };
  const close = () => setOpen(false);
  return <>
    <Button variant={variant} icon={busy || pending ? "loading" : icon} disabled={disabled || busy || pending} aria-haspopup="dialog"
      aria-busy={busy || pending || undefined} onClick={() => setOpen(true)}>{label}</Button>
    <Dialog title={label} open={open} onClose={close}
      actions={<><Button variant="secondary" onClick={close}>取消</Button>
        <Button icon={icon} disabled={disabled || busy} onClick={() => void confirm()}>{confirmLabel}</Button></>}>
      {children}
    </Dialog>
  </>;
}
