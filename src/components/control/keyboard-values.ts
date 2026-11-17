import type { KeyboardValues } from "../../api/client";
export const FIELDS: { key: keyof KeyboardValues; title: string; description: string }[] = [
  { key: "fnLocked", title: "Fn 锁", description: "切换功能键的默认操作" },
  { key: "winLocked", title: "Win 锁", description: "阻止 Windows 键中断当前操作" },
  { key: "fnWinSwap", title: "Fn / Win 交换", description: "交换 Fn 与 Windows 键的位置" },
];
export const stateText = (value: boolean | null) => value === null ? "未提供" : value ? "开启" : "关闭";
