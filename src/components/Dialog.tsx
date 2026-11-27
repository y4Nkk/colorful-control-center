import { createContext, useContext, useId, useLayoutEffect, useRef, type KeyboardEvent, type ReactNode } from "react";
import { getTabStops } from "../lib/tab-stops";
import { Button } from "./Button";
import { ScrollArea } from "./ScrollArea";

export const WorkspaceActivity = createContext(true);

function isBackdropPoint(dialog: HTMLDialogElement, event: Pick<MouseEvent, "target" | "clientX" | "clientY">) {
  if (event.target !== dialog) return false;
  const { left, right, top, bottom } = dialog.getBoundingClientRect();
  return event.clientX < left || event.clientX >= right || event.clientY < top || event.clientY >= bottom;
}

function sameTabStop(first: HTMLElement, second: Element) {
  return first === second || (first instanceof HTMLInputElement && second instanceof HTMLInputElement
    && first.type === "radio" && second.type === "radio" && !!first.name && first.name === second.name && first.form === second.form);
}

function wrapTabFocus(event: KeyboardEvent<HTMLDialogElement>, heading: HTMLHeadingElement | null) {
  const { currentTarget: dialog, target, shiftKey } = event;
  if (event.key !== "Tab" || event.defaultPrevented || !(target instanceof Element) || target.closest("dialog") !== dialog) return;
  const stops = getTabStops(dialog);
  const first = stops[0], last = stops[stops.length - 1];
  const boundary = shiftKey ? first : last;
  if (boundary && !sameTabStop(boundary, target) && !(shiftKey && target === heading)) return;
  const next = shiftKey ? last : first;
  const checked = next instanceof HTMLInputElement && next.type === "radio"
    ? stops.find(stop => stop instanceof HTMLInputElement && stop.checked && sameTabStop(next, stop)) : undefined;
  event.preventDefault();
  (checked ?? next ?? heading)?.focus();
}

export function Dialog({ title, open, busy, onClose, children, actions, width = "standard", layout, dismissOnBackdrop = false }: {
  title: string; open: boolean; busy?: boolean; onClose: () => void;
  children?: ReactNode; actions?: ReactNode; width?: "standard" | "wide" | "narrow";
  layout?: "confirmation"; dismissOnBackdrop?: boolean;
}) {
  const panel = useRef<HTMLDialogElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const backdropPressed = useRef(false);
  const active = useContext(WorkspaceActivity);
  const titleId = useId();

  useLayoutEffect(() => {
    const node = panel.current!;
    if (open && active && !node.open) {
      node.showModal();
      heading.current?.focus({ preventScroll: true });
    } else if ((!open || !active) && node.open) {
      node.close();
      if (!active) onClose();
    }
  }, [open, active, onClose]);

  useLayoutEffect(() => {
    const node = panel.current;
    if (!node || !open || !active || busy || !dismissOnBackdrop) return;
    // Check the child overlays before the select's document listener can dismiss its menu.
    const pointerDown = (event: PointerEvent) => {
      backdropPressed.current = event.isPrimary && event.button === 0 && isBackdropPoint(node, event)
        && !node.querySelector('[aria-haspopup="listbox"][aria-expanded="true"], :popover-open');
    };
    window.addEventListener("pointerdown", pointerDown, true);
    return () => {
      window.removeEventListener("pointerdown", pointerDown, true);
      backdropPressed.current = false;
    };
  }, [open, active, busy, dismissOnBackdrop]);

  return <dialog ref={panel} className="ccc-dialog" data-width={width} data-layout={layout}
    aria-labelledby={titleId} aria-busy={busy || undefined}
    onKeyDown={event => wrapTabFocus(event, heading.current)}
    onCancel={event => {
      if (event.target !== event.currentTarget) return;
      event.preventDefault();
      if (!busy) onClose();
    }}
    onClick={event => {
      const pressed = backdropPressed.current;
      backdropPressed.current = false;
      if (dismissOnBackdrop && !busy && pressed && event.detail > 0 && isBackdropPoint(event.currentTarget, event)) onClose();
    }}>
    {open && active && <div className="ccc-dialog-frame">
      <header className="ccc-dialog-header">
        <h2 id={titleId} ref={heading} tabIndex={-1}>{title}</h2>
        {layout !== "confirmation" && <Button icon="close" variant="ghost" size="icon"
          aria-label="关闭弹窗" disabled={busy} onClick={onClose} />}
      </header>
      {children && <ScrollArea className="ccc-dialog-body" aria-label={title}>{children}</ScrollArea>}
      {actions && <footer className="ccc-dialog-footer">{actions}</footer>}
    </div>}
  </dialog>;
}
