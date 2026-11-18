import { useState, type ReactNode } from "react";
import { Button } from "./Button";
import { Dialog } from "./Dialog";
import type { IconName } from "./Icon";

export function DetailsDialog({ title, label = title, icon = "info", className, wide, editor, busy, children }: {
  title: string; label?: string; icon?: IconName; className?: string; wide?: boolean;
  editor?: boolean; busy?: boolean; children: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const close = () => setOpen(false);
  return <>
    <Button className={className} icon={icon} variant={editor ? "secondary" : "ghost"} size="compact" aria-haspopup="dialog"
      onClick={() => setOpen(true)}>{label}</Button>
    <Dialog title={title} wide={wide} open={open} busy={busy} onClose={close}
      actions={editor && <Button disabled={busy} onClick={close}>完成编辑</Button>}>{children}</Dialog>
  </>;
}
