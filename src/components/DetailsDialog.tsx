import { useState, type ReactNode } from "react";
import { Button } from "./Button";
import { Dialog } from "./Dialog";
import type { IconName } from "./Icon";

export function DetailsDialog({ title, label = title, icon = "info", className, wide, editor, busy, children }: {
  title: string; label?: string; icon?: IconName | null; className?: string; wide?: boolean;
  editor?: boolean; busy?: boolean; children: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const close = () => setOpen(false);
  const information = icon === "info";
  return <>
    <Button className={className} icon={icon ?? undefined} variant={editor ? "secondary" : "ghost"}
      size={information ? "icon-compact" : "compact"} aria-haspopup="dialog" aria-label={information ? label : undefined}
      title={information ? title : undefined} onClick={() => setOpen(true)}>{!information && label}</Button>
    <Dialog title={title} width={wide ? "wide" : "standard"} open={open} busy={busy} onClose={close} dismissOnBackdrop={!editor}
      actions={editor && <Button disabled={busy} onClick={close}>完成编辑</Button>}>{children}</Dialog>
  </>;
}
