import { useContext, useRef, useState, type ReactNode } from "react";
import { Button } from "./Button";
import { WorkspaceActivity } from "./Dialog";
import { RestoreSettings } from "./RestoreSettings";
import { toast } from "./use-toast";

type Result = boolean | void;
type Commit = () => Result | Promise<Result>;
type ActionOptions = {
  label: string;
  applyLabel?: string;
  revision: string;
  disabled?: boolean;
  blocked?: boolean;
  restore?: boolean;
  children?: ReactNode;
  onConfirm: Commit;
};

export function ConfigurationAction({ restore, ...props }: ActionOptions) {
  if (restore) return props.disabled ? null : <RestoreSettings label={props.label} blocked={props.blocked}
    options={[{ label: props.label, revision: props.revision, preview: props.children }]} onRestore={props.onConfirm} />;
  return <ConfigurationApply {...props} />;
}

function ConfigurationApply({ label, applyLabel = "应用更改", disabled, blocked, children, onConfirm }: ActionOptions) {
  const active = useContext(WorkspaceActivity);
  const [busy, setBusy] = useState(false);
  const running = useRef(false);
  const unavailable = disabled || blocked;
  const submit = async () => {
    if (!active || unavailable || running.current) return;
    running.current = true;
    setBusy(true);
    try { await onConfirm(); }
    catch (error) { toast({ kind: "error", title: `${label}操作失败`, description: String(error) }); }
    finally { running.current = false; setBusy(false); }
  };
  if (!active || (disabled && !busy)) return null;
  return <section className="ccc-module-action ccc-review-section" aria-label={`${label} · ${applyLabel}`} data-module-action={label} aria-busy={busy}>
    {children}
    <div className="ccc-actions"><Button icon={busy ? "loading" : "check"} data-configuration-apply="" aria-busy={busy || undefined}
      disabled={busy || unavailable} onClick={() => void submit()}>{applyLabel}</Button></div>
  </section>;
}
