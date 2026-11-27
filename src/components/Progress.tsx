/** 进度条唯一原语：提供 value 与 max 时按比例填充，否则循环。 */
export function Progress({ label, value, max, className }: {
  label: string; value?: number; max?: number; className?: string;
}) {
  const known = value !== undefined && Number.isFinite(value) && max !== undefined && Number.isFinite(max) && max > 0;
  const current = known ? Math.min(Math.max(value, 0), max) : undefined;
  return <div className={["ccc-progress", className].filter(Boolean).join(" ")} role="progressbar" aria-label={label}
    aria-valuemin={known ? 0 : undefined} aria-valuemax={known ? max : undefined} aria-valuenow={current}>
    <span style={current !== undefined && known ? { inlineSize: `${(current / max) * 100}%` } : undefined} />
  </div>;
}
