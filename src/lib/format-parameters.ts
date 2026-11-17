export function formatParameter(value: number | null | undefined, unit = "", digits = 0): string {
  if (value == null || !Number.isFinite(value)) return "未知";
  return `${Number(value.toFixed(digits))}${unit ? ` ${unit}` : ""}`;
}

export const formatCapacity = (bytes: number | null | undefined) =>
  formatParameter(bytes == null ? null : bytes / 1024 ** 3, "GiB", 1);
