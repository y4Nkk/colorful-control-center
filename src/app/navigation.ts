/** 一级工作区的唯一页面、标题与语义图标清单。 */
import type { NavigationIconName } from "../components/Icon";
export type PageId = "dashboard" | "performance" | "graphics" | "keyboard" | "power" | "settings";
export interface PageDefinition { id: PageId; title: string; icon: NavigationIconName; workspace?: string }
export const PAGES: readonly PageDefinition[] = [
  { id: "dashboard", workspace: "ControlWorkspace", title: "概览", icon: "overview" },
  { id: "performance", title: "性能与散热", icon: "performance" },
  { id: "keyboard", title: "键盘与灯光", icon: "keyboard" },
  { id: "power", title: "电源与档案", icon: "power" },
  { id: "graphics", workspace: "DeviceConfigurationWorkspace", title: "设备配置", icon: "display" },
  { id: "settings", title: "设置", icon: "settings" },
];
export function pageTitle(page: PageId): string { return PAGES.find(entry => entry.id === page)!.title; }

export const POWER_AREAS = [
  { value: "settings", label: "电源设置" },
  { value: "profiles", label: "配置档案" },
] as const;
export type PowerArea = typeof POWER_AREAS[number]["value"];
export interface WorkspaceLocation { page: PageId; powerArea: PowerArea }

export const DEFAULT_LOCATION: WorkspaceLocation = { page: "dashboard", powerArea: "settings" };
export const PROFILES_LOCATION = { page: "power", powerArea: "profiles" } as const;
export function workspaceLocationKey({ page, powerArea }: WorkspaceLocation) {
  if (page === "power" && powerArea === "profiles") return "power:profiles";
  return page;
}
