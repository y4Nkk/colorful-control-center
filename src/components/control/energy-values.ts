import type { OemEnergyState } from "../../api/client";

export function energyValue(value: OemEnergyState) {
  if (value.hardwarePState) return "节能由硬件管理";
  return value.energyStar === null ? "固件节能值未知" : `固件节能值 ${value.energyStar}`;
}
