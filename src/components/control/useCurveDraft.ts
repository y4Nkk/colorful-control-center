import { useRef } from "react";

export function useCurveDraft<T>(draft: T, onChange: (value: T) => void) {
  const gesture = useRef<{ initial: T; value: T } | null>(null);
  const update = (change: (value: T) => T) => {
    const before = gesture.current;
    const value = change(before?.value ?? draft);
    gesture.current = { initial: before?.initial ?? draft, value };
    onChange(value);
  };
  const commit = (change?: (value: T) => T) => {
    const value = gesture.current?.value ?? draft;
    gesture.current = null;
    if (change) onChange(change(value));
  };
  const cancel = () => {
    const before = gesture.current;
    gesture.current = null;
    if (before) onChange(before.initial);
  };
  return { update, commit, cancel };
}
