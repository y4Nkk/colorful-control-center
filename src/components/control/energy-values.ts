import type { OemEnergyState } from "../../api/client";

export function energyValue(value: OemEnergyState) {
  if (value.hardwarePState) return "节能由电脑自动管理";
  return value.energyStar === null ? "电脑节能等级未知" : `电脑节能等级 ${value.energyStar}`;
}
