import { useRef, useState } from "react";

export function useFanEdit<T>(source: string, current: T, onCommit: (value: T) => void | Promise<void>) {
  type Preview = { source: string; value: T };
  const [preview, setPreview] = useState<Preview | null>(null);
  const gesture = useRef<Preview | null>(null);

  const cancel = () => {
    const previous = gesture.current;
    gesture.current = null;
    setPreview(shown => shown === previous ? null : shown);
  };
  const update = (change: (value: T) => T) => {
    const previous = gesture.current;
    const next = { source, value: change(previous?.source === source ? previous.value : current) };
    gesture.current = next;
    setPreview(next);
  };
  const commit = (change?: (value: T) => T) => {
    const previous = gesture.current;
    if (previous && previous.source !== source) { cancel(); return; }
    if (!previous && !change) return;
    const value = previous?.value ?? current;
    const submitted = { source, value: change ? change(value) : value };
    gesture.current = null;
    setPreview(submitted);
    const settle = () => setPreview(shown => shown === submitted ? null : shown);
    void Promise.resolve(onCommit(submitted.value)).then(settle, settle);
  };

  return { value: preview?.source === source ? preview.value : current, update, commit, cancel };
}
