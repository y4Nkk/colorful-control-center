import { Button } from "./Button";
import { Dialog } from "./Dialog";
import type { IconName } from "./Icon";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { toast } from "./use-toast";

export function ConfirmButton({ label, confirmLabel, onConfirm, onOpen, revision, disabled = false, confirmDisabled = false,
  busy = false, variant = "secondary", tone, size, icon = "apply", children }: {
  label: string;
  confirmLabel: string;
  onConfirm: () => void | boolean | Promise<void | boolean>;
  onOpen?: () => void;
  revision?: string;
  disabled?: boolean;
  confirmDisabled?: boolean;
  busy?: boolean;
  variant?: "primary" | "secondary";
  tone?: "danger";
  size?: "compact";
  icon?: IconName;
  children?: ReactNode;
}) {
  const [opened, setOpened] = useState<{ revision?: string } | null>(null);
  const open = opened !== null && opened.revision === revision;
  const [pending, setPending] = useState(false);
  const executing = useRef(false);
  useEffect(() => { setOpened(null); }, [label, confirmLabel, disabled, busy, revision]);
  const confirm = async () => {
    if (!open || disabled || confirmDisabled || busy || executing.current) return;
    executing.current = true;
    setPending(true);
    setOpened(null);
    try { await onConfirm(); }
    catch (error) { toast({ kind: "error", title: "操作失败", description: String(error) }); }
    finally { executing.current = false; setPending(false); }
  };
  const close = () => setOpened(null);
  return <>
    <Button variant={variant} tone={tone} size={size} icon={icon} busy={pending} disabled={disabled || busy}
      aria-haspopup="dialog" onClick={() => { onOpen?.(); setOpened({ revision }); }}>{label}</Button>
    <Dialog title={label} width="narrow" open={open} onClose={close}
      actions={<><Button variant="secondary" onClick={close}>取消</Button>
        <Button icon={icon} tone={tone} disabled={disabled || confirmDisabled || busy} onClick={() => void confirm()}>{confirmLabel}</Button></>}>
      {children}
    </Dialog>
  </>;
}
