import { useContext, useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import type { RgbColor } from "../api/client";
import { hexOf } from "../lib/color";
import { Button } from "./Button";
import { ColorPickerEditor } from "./ColorPickerEditor";
import { ColorSwatch } from "./ColorSwatch";
import { WorkspaceActivity } from "./Dialog";

export function ColorPicker({ label, value, onChange, swatchOnly, disabled }: {
  label: string; value: RgbColor; onChange: (color: RgbColor) => void; swatchOnly?: boolean; disabled?: boolean;
}) {
  const id = useId();
  const active = useContext(WorkspaceActivity);
  const blocked = disabled || !active;
  const [open, setOpen] = useState(false);
  const trigger = useRef<HTMLButtonElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const hex = hexOf(value).toUpperCase();

  useLayoutEffect(() => { if (blocked) panel.current?.hidePopover(); }, [blocked]);
  useLayoutEffect(() => { if (open) heading.current?.focus({ preventScroll: true }); }, [open]);
  useEffect(() => {
    if (!open || !trigger.current) return;
    const visibility = new IntersectionObserver(([entry]) => {
      if (!entry.isIntersecting) panel.current?.hidePopover();
    });
    visibility.observe(trigger.current);
    return () => visibility.disconnect();
  }, [open]);

  return <>
    <Button ref={trigger} className="ccc-color-picker-trigger" variant="field" size={swatchOnly ? "icon" : "default"}
      disabled={blocked} aria-label={label} aria-description={hex} title={hex}
      aria-haspopup="dialog" aria-expanded={open} popoverTarget={id}>
      <ColorSwatch color={value} />{!swatchOnly && <span>{label}</span>}
    </Button>
    <div ref={panel} id={id} className="ccc-color-picker-panel" popover="auto" role="dialog" aria-labelledby={`${id}-title`}
      onToggle={event => setOpen(event.newState === "open")}>
      {open && <>
        <header className="ccc-color-picker-header">
          <h3 id={`${id}-title`} ref={heading} tabIndex={-1}>{label}</h3>
          <Button icon="close" variant="ghost" size="icon-compact" aria-label="关闭选色" popoverTarget={id} popoverTargetAction="hide" />
        </header>
        <ColorPickerEditor value={value} onChange={onChange} />
      </>}
    </div>
  </>;
}
