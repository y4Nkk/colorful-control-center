import type { ProfileGroup } from "../../api/client";

export const PROFILE_GROUP_LABELS: Record<ProfileGroup, string> = {
  performance: "性能与散热",
  display: "显示与显卡",
  lighting: "灯光",
  power: "充电与供电",
  devices: "设备功能",
};

export const PROFILE_GROUPS = Object.keys(PROFILE_GROUP_LABELS) as ProfileGroup[];
