import { useCallback, useEffect, useId, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent, type RefObject } from "react";

export interface SelectOption { value: string; label: string; description?: string; disabled?: boolean; style?: CSSProperties }
export interface SelectProps {
  id?: string;
  label: string;
  value: string;
  options: readonly SelectOption[];
  size?: "default" | "compact";
  disabled?: boolean;
  placeholder?: string;
  searchLabel?: string;
  loading?: boolean;
  error?: string | null;
  onRetry?: () => void;
  onChange: (value: string) => void;
}

function focusBeside(trigger: HTMLButtonElement | null, menu: HTMLElement | null, backwards: boolean) {
  const elements = [...document.querySelectorAll<HTMLElement>('button,input,a[href],textarea,[tabindex]')]
    .filter(el => el.tabIndex >= 0 && !el.matches(":disabled") && el.checkVisibility() && !menu?.contains(el));
  const index = elements.findIndex(el => el === trigger);
  elements[index + (backwards ? -1 : 1)]?.focus();
}

function useSelectDismiss(open: boolean, trigger: RefObject<HTMLButtonElement | null>,
  menu: RefObject<HTMLDivElement | null>, close: (restore?: boolean) => void, disabled?: boolean) {
  useEffect(() => {
    if (!open) return;
    if (disabled) { close(false); return; }
    const outside = (event: Event) => {
      const target = event.target as Node;
      if (!trigger.current?.contains(target) && !menu.current?.contains(target)) close(false);
    };
    const blur = () => close(false);
    document.addEventListener("pointerdown", outside, true);
    document.addEventListener("focusin", outside);
    window.addEventListener("blur", blur);
    return () => {
      document.removeEventListener("pointerdown", outside, true);
      document.removeEventListener("focusin", outside);
      window.removeEventListener("blur", blur);
    };
  }, [open, trigger, menu, close, disabled]);
}

export function useSelect(props: SelectProps) {
  const { options, value, disabled, searchLabel } = props;
  const [open, setOpen] = useState(false), [query, setQuery] = useState(""), [active, setActive] = useState<string | null>(null);
  const trigger = useRef<HTMLButtonElement>(null), menu = useRef<HTMLDivElement>(null);
  const typed = useRef({ text: "", at: 0 });
  const listId = useId();
  const choices = useMemo(() => options.filter(option => !searchLabel || option.label.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase())), [options, query, searchLabel]);
  const enabled = choices.filter(option => !option.disabled);
  const current = enabled.find(option => option.value === active) ?? enabled.find(option => option.value === value) ?? enabled[0];
  const activeId = current ? `${listId}-${choices.indexOf(current)}` : undefined;
  const close = useCallback((restore = true) => {
    setOpen(false); setQuery(""); setActive(null); typed.current = { text: "", at: 0 };
    if (restore) trigger.current?.focus();
  }, []);
  const dismiss = useCallback(() => close(false), [close]);
  useSelectDismiss(open, trigger, menu, close, disabled);
  const choose = (next: string) => {
    if (disabled || !options.some(option => option.value === next && !option.disabled)) return;
    close(); if (next !== value) props.onChange(next);
  };
  const keyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (disabled || event.nativeEvent.isComposing) return;
    const editing = event.target instanceof HTMLInputElement;
    const index = enabled.findIndex(option => option === current);
    if (event.key === "Escape" && open) { event.preventDefault(); event.stopPropagation(); close(); }
    else if (event.key === "Tab" && open) { event.preventDefault(); close(false); focusBeside(trigger.current, menu.current, event.shiftKey); }
    else if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault(); setOpen(true);
      const next = !open ? enabled.find(option => option.value === value) ?? (event.key === "ArrowUp" ? enabled[enabled.length - 1] : enabled[0])
        : enabled[Math.max(0, Math.min(enabled.length - 1, index + (event.key === "ArrowDown" ? 1 : -1)))];
      setActive(next?.value ?? null);
    } else if (!editing && (event.key === "Home" || event.key === "End")) {
      event.preventDefault(); setOpen(true); setActive((event.key === "Home" ? enabled[0] : enabled[enabled.length - 1])?.value ?? null);
    } else if (open && (event.key === "Enter" || (!editing && event.key === " "))) {
      event.preventDefault(); if (current) choose(current.value);
    } else if (!searchLabel && event.key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey && event.key !== " ") {
      event.preventDefault(); setOpen(true);
      const text = (Date.now() - typed.current.at < 700 ? typed.current.text : "") + event.key.toLocaleLowerCase();
      typed.current = { text, at: Date.now() };
      const next = enabled.find(option => option.label.toLocaleLowerCase().startsWith(text));
      if (next) setActive(next.value);
    }
  };
  return { open, query, setQuery, choices, current, activeId, listId, trigger, menu, choose, close, dismiss, keyDown,
    setActive, toggle: () => { if (!disabled) { if (open) close(false); else setOpen(true); } } };
}
