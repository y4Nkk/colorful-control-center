import { useEffect, useId, useRef, useState } from "react";
import { Surface } from "./Surface";
import { Button } from "./Button";
import { useToast, type ToastKind } from "./use-toast";
import { Icon, type IconName } from "./Icon";
import type { BatteryColdWarning } from "../api/client";

const STATUS_ICONS: Record<ToastKind, IconName> = {
  info: "info", success: "check-circle", warning: "warning", error: "error",
};

function NotificationCenter({ toasts, dismiss, clear, pause, resume }: ReturnType<typeof useToast>) {
  const [open, setOpen] = useState(false);
  const [hovered, setHovered] = useState(false);
  const [focused, setFocused] = useState(false);
  const close = useRef<HTMLButtonElement>(null);
  const id = useId();
  const primary = toasts[0];

  useEffect(() => {
    const sync = () => {
      const paused = document.hidden || open || hovered || focused;
      for (const item of toasts) {
        if (paused || item.id !== primary.id) pause(item.id);
        else resume(item.id);
      }
    };
    sync();
    document.addEventListener("visibilitychange", sync);
    return () => {
      document.removeEventListener("visibilitychange", sync);
      toasts.forEach(item => pause(item.id));
    };
  }, [toasts, primary.id, open, hovered, focused, pause, resume]);

  return <>
    <Button variant="ghost" size="compact" className="ccc-notification-trigger" data-kind={primary.kind}
      aria-label={`通知（${toasts.length} 条）`} aria-describedby={`${id}-summary`} aria-haspopup="dialog"
      aria-expanded={open} popoverTarget={id}
      onPointerEnter={() => setHovered(true)} onPointerLeave={() => setHovered(false)}
      onFocus={() => setFocused(true)} onBlur={() => setFocused(false)}>
      <Icon name={STATUS_ICONS[primary.kind]} className="ccc-toast-status" />
      <span id={`${id}-summary`} className="ccc-notification-summary" role={primary.kind === "error" ? "alert" : "status"}>
        {primary.title}
      </span>
      <span className="ccc-notification-count">{toasts.length}</span>
    </Button>
    <Surface id={id} variant="card" className="ccc-notification-panel" popover="auto" role="dialog" aria-label="通知"
      onToggle={event => {
        const expanded = event.currentTarget.matches(":popover-open");
        setOpen(expanded);
        if (expanded) close.current?.focus({ preventScroll: true });
      }}>
      <header className="ccc-notification-heading">
        <h2>通知</h2>
        <Button variant="secondary" size="compact" onClick={clear}>清空通知</Button>
        <Button ref={close} variant="ghost" size="icon-compact" icon="close" aria-label="收起通知"
          popoverTarget={id} popoverTargetAction="hide" />
      </header>
      <div className="ccc-notification-list">
        {open && toasts.map(item => <div key={item.id} className="ccc-toast" data-kind={item.kind}>
          <Icon name={STATUS_ICONS[item.kind]} className="ccc-toast-status" />
          <div className="ccc-toast-body">
            <div className="ccc-toast-title">{item.title}</div>
            {item.description && <div className="ccc-toast-description">{item.description}</div>}
          </div>
          <Button variant="ghost" size="icon-compact" icon="close" aria-label="关闭通知" onClick={() => {
            close.current?.focus({ preventScroll: true });
            dismiss(item.id);
          }} />
        </div>)}
      </div>
    </Surface>
  </>;
}

export function Toaster({ coldWarning, dismissColdWarning }: { coldWarning: BatteryColdWarning | null; dismissColdWarning: () => void }) {
  const queue = useToast();
  return <>
    {queue.toasts.length > 0 && <NotificationCenter {...queue} />}
    {coldWarning && <Surface variant="card" className="ccc-toast ccc-cold-warning" data-kind="warning" role="alert">
      <Icon name="warning" className="ccc-toast-status" />
      <div className="ccc-toast-body"><div className="ccc-toast-title">电池温度较低</div>
        <div className="ccc-toast-description">电池温度 {coldWarning.temperatureC}°C，电量 {coldWarning.batteryPercent}%。请接入电源。</div></div>
      <Button variant="secondary" size="compact" onClick={dismissColdWarning}>知道了</Button>
    </Surface>}
  </>;
}
