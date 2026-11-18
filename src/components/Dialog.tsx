import { createContext, useContext, useId, useLayoutEffect, useRef, type ReactNode } from "react";
import { Button } from "./Button";
import { ScrollArea } from "./ScrollArea";

export const WorkspaceActivity = createContext(true);

export function Dialog({ title, open, busy, onClose, children, actions, wide = false }: {
  title: string; open: boolean; busy?: boolean; onClose: () => void;
  children?: ReactNode; actions?: ReactNode; wide?: boolean;
}) {
  const panel = useRef<HTMLDialogElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const active = useContext(WorkspaceActivity);
  const titleId = useId();
  useLayoutEffect(() => {
    const node = panel.current!;
    if (open && active && !node.open) {
      node.showModal();
      heading.current?.focus({ preventScroll: true });
    } else if ((!open || !active) && node.open) node.close();
  }, [open, active]);
  return <dialog ref={panel} className="ccc-dialog" data-width={wide ? "wide" : undefined}
    aria-labelledby={titleId} aria-busy={busy || undefined}
    onClose={() => { if (!panel.current?.open) onClose(); }}
    onCancel={event => { if (busy) event.preventDefault(); }}>
    {open && active && <div className="ccc-dialog-frame">
      <header className="ccc-dialog-header">
        <h2 id={titleId} ref={heading} tabIndex={-1}>{title}</h2>
        <Button icon="close" variant="ghost" size="icon-compact" aria-label="关闭弹窗" disabled={busy} onClick={onClose} />
      </header>
      {children && <ScrollArea className="ccc-dialog-body" aria-label={title}>{children}</ScrollArea>}
      {actions && <footer className="ccc-dialog-footer">{actions}</footer>}
    </div>}
  </dialog>;
}
