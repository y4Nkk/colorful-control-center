import { useContext, useRef, useState, type ReactNode } from "react";
import { Button } from "./Button";
import { ConfirmButton } from "./ConfirmButton";
import { WorkspaceActivity } from "./Dialog";
import { toast } from "./use-toast";

type Result = boolean | void;
type Commit = () => Result | Promise<Result>;
type ActionOptions = {
  label: string;
  applyLabel?: string;
  revision: string;
  disabled?: boolean;
  restore?: boolean;
  children?: ReactNode;
  onConfirm: Commit;
};

export function ConfigurationAction({ label, applyLabel = "应用更改", revision, disabled, restore, children, onConfirm }: ActionOptions) {
  const active = useContext(WorkspaceActivity);
  const [busy, setBusy] = useState(false);
  const running = useRef(false);
  const submit = async () => {
    if (!active || disabled || running.current) return;
    running.current = true;
    setBusy(true);
    try { await onConfirm(); }
    catch (error) { toast({ kind: "error", title: `${label}操作失败`, description: String(error) }); }
    finally { running.current = false; setBusy(false); }
  };
  if (!active || (disabled && !busy)) return null;
  if (restore) return <div className="ccc-actions" data-module-restore={label}>
    <ConfirmButton key={revision} label={`恢复${label}`} confirmLabel="确认恢复" icon="undo" size="compact"
      busy={busy} disabled={disabled} onConfirm={submit}>{children}</ConfirmButton>
  </div>;
  return <section className="ccc-module-action ccc-review-section" aria-label={`${label} · ${applyLabel}`} data-module-action={label} aria-busy={busy}>
    {children}
    <div className="ccc-actions"><Button icon={busy ? "loading" : "check"} data-configuration-apply=""
      disabled={busy || disabled} onClick={() => void submit()}>{applyLabel}</Button></div>
  </section>;
}
