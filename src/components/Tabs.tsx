import { useEffect, useId, useState, type KeyboardEvent, type ReactNode } from "react";

interface TabsProps<Value extends string | number> {
  label: string;
  value: Value;
  tabs: readonly { value: Value; label: string; content: ReactNode }[];
  onChange: (value: Value) => void;
  disabled?: boolean;
  busy?: boolean;
  divider?: boolean;
  keepAlive?: boolean;
}

function moveFocus(event: KeyboardEvent<HTMLDivElement>) {
  const { altKey, ctrlKey, metaKey, key, currentTarget, target } = event;
  if (altKey || ctrlKey || metaKey || !["ArrowLeft", "ArrowRight", "Home", "End"].includes(key)) return;
  const buttons = Array.from(currentTarget.querySelectorAll<HTMLButtonElement>("button"));
  const index = buttons.findIndex(button => button === target);
  if (index < 0) return;
  event.preventDefault();
  const last = buttons.length - 1;
  const next = key === "Home" ? 0 : key === "End" ? last
    : (index + (key === "ArrowRight" ? 1 : last)) % buttons.length;
  buttons[next].focus();
}

export function Tabs<Value extends string | number>({ label, value, tabs, onChange, disabled, busy, keepAlive, divider = true }: TabsProps<Value>) {
  const id = useId();
  const [focused, setFocused] = useState<Value | null>(null);
  const [visited, setVisited] = useState<ReadonlySet<Value>>(() => new Set());
  useEffect(() => {
    if (keepAlive) setVisited(previous => previous.has(value) ? previous : new Set([...previous, value]));
  }, [value, keepAlive]);
  const tabStop = focused !== null && tabs.some(tab => tab.value === focused) ? focused : value;
  return <>
    <div role="tablist" aria-label={label} className="ccc-tabs-list" data-no-divider={!divider || undefined}
      onKeyDown={moveFocus} onBlur={event => {
        if (!event.currentTarget.contains(event.relatedTarget)) setFocused(null);
      }}>
      {tabs.map((tab, index) => <button key={tab.value} className="ccc-tabs-tab" type="button" role="tab"
        id={`${id}-tab-${index}`} aria-controls={`${id}-panel-${index}`} aria-selected={tab.value === value}
        aria-disabled={disabled || busy || undefined} aria-busy={busy && !disabled || undefined} tabIndex={tab.value === tabStop ? 0 : -1}
        onFocus={() => setFocused(tab.value)} onClick={() => { if (!disabled && !busy && tab.value !== value) onChange(tab.value); }}>
        {tab.label}
      </button>)}
    </div>
    {tabs.map((tab, index) => {
      const selected = tab.value === value;
      return <div key={tab.value} role="tabpanel" className="ccc-tabs-panel" id={`${id}-panel-${index}`}
        aria-labelledby={`${id}-tab-${index}`} hidden={!selected} tabIndex={selected ? 0 : undefined}>
        {(selected || (keepAlive && visited.has(tab.value))) && tab.content}
      </div>;
    })}
  </>;
}
