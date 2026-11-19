export function formatParameter(value: number | null | undefined, unit = "", digits = 0): string {
  if (value == null || !Number.isFinite(value)) return "未知";
  return `${Number(value.toFixed(digits))}${unit ? ` ${unit}` : ""}`;
}

// 容量界面沿用 Windows 的 1024 换算和单位名称。
const CAPACITY_SCALE = 1024;
const CAPACITY_UNITS = ["B", "KB", "MB", "GB", "TB", "PB"] as const;
export const BYTES_PER_MB = CAPACITY_SCALE ** 2;

export function capacityParts(bytes: number | null | undefined) {
  if (bytes == null || !Number.isFinite(bytes) || bytes < 0) return null;
  let value = bytes;
  let index = 0;
  while (value >= CAPACITY_SCALE && index < CAPACITY_UNITS.length - 1) {
    value /= CAPACITY_SCALE;
    index++;
  }
  const digits = index === 0 || value >= 100 ? 0 : value >= 10 ? 1 : 2;
  return { value: formatParameter(value, "", digits), unit: CAPACITY_UNITS[index] };
}

export function formatCapacity(bytes: number | null | undefined) {
  const parts = capacityParts(bytes);
  return parts ? `${parts.value} ${parts.unit}` : "未知";
}
