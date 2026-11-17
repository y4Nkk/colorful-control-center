/** 一级工作区的唯一页面、标题与语义图标清单。 */
import type { IconName } from "../components/Icon";
export type PageId = "dashboard" | "cooling" | "performance" | "graphics" | "lighting" | "input" | "battery" | "automation" | "profiles" | "settings";
export interface PageDefinition { id: PageId; title: string; icon: IconName; workspace?: string }
export const PAGES: readonly PageDefinition[] = [
  { id: "dashboard", workspace: "ControlWorkspace", title: "控制台", icon: "controls" },
  { id: "cooling", title: "风扇散热", icon: "fan" },
  { id: "performance", title: "性能调校", icon: "performance" },
  { id: "graphics", title: "显示与显卡", icon: "display" },
  { id: "lighting", title: "灯光效果", icon: "lighting" },
  { id: "input", title: "键盘与输入", icon: "keyboard" },
  { id: "battery", title: "电池与节能", icon: "battery" },
  { id: "automation", title: "自动策略", icon: "power" },
  { id: "profiles", title: "性能档案", icon: "profiles" },
  { id: "settings", title: "设置", icon: "settings" },
];
export function pageTitle(page: PageId): string { return PAGES.find(entry => entry.id === page)!.title; }
