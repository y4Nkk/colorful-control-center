import { useRef, useState } from "react";

export function useFanEdit<T>(source: object, current: T, onCommit: (value: T) => void) {
  type Preview = { source: object; value: T };
  const [preview, setPreview] = useState<Preview | null>(null);
  const pending = useRef<Preview | null>(null);

  const cancel = () => { pending.current = null; setPreview(null); };
  const update = (change: (value: T) => T) => {
    const previous = pending.current;
    const next = { source, value: change(previous?.source === source ? previous.value : current) };
    pending.current = next;
    setPreview(next);
  };
  const commit = (change?: (value: T) => T) => {
    const previous = pending.current;
    cancel();
    if (previous && previous.source !== source) return;
    if (change) onCommit(change(previous?.value ?? current));
    else if (previous) onCommit(previous.value);
  };

  return { value: preview?.source === source ? preview.value : current, update, commit, cancel };
}
