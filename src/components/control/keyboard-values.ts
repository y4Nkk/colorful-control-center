import type { KeyboardValues } from "../../api/client";
export const FIELDS: { key: keyof KeyboardValues; title: string; description: string }[] = [
  { key: "fnLocked", title: "Fn 锁", description: "切换 F1–F12 按键的默认作用：直接执行音量、亮度等功能，或作为标准功能键" },
  { key: "winLocked", title: "Win 锁", description: "锁定 Windows 键，玩游戏时误触也不会弹出开始菜单" },
  { key: "fnWinSwap", title: "Fn / Win 交换", description: "对调 Fn 键和 Windows 键的功能" },
];
export const stateText = (value: boolean | null) => value === null ? "未提供" : value ? "开启" : "关闭";
