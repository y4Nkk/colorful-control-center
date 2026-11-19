export function preserveEditedFields<T extends object>(before: T, draft: T, current: T): T {
  const next = { ...current };
  for (const field of Object.keys(current) as (keyof T)[]) {
    if (JSON.stringify(draft[field]) !== JSON.stringify(before[field])) next[field] = draft[field];
  }
  return next;
}
