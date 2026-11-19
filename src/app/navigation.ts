/** 一级工作区的唯一页面、标题与语义图标清单。 */
import type { IconName } from "../components/Icon";
export type PageId = "dashboard" | "graphics" | "keyboard" | "power" | "settings";
export interface PageDefinition { id: PageId; title: string; icon: IconName; workspace?: string }
export const PAGES: readonly PageDefinition[] = [
  { id: "dashboard", workspace: "SystemWorkspace", title: "系统", icon: "overview" },
  { id: "keyboard", title: "键盘与灯光", icon: "keyboard" },
  { id: "power", title: "电源", icon: "power" },
  { id: "graphics", workspace: "DeviceConfigurationWorkspace", title: "设备配置", icon: "display" },
  { id: "settings", title: "设置", icon: "settings" },
];
export function pageTitle(page: PageId): string { return PAGES.find(entry => entry.id === page)!.title; }

export const SYSTEM_AREAS = [
  { value: "overview", label: "概览" },
  { value: "cooling", label: "风扇散热" },
  { value: "performance", label: "性能调校" },
  { value: "profiles", label: "配置档案" },
] as const;
export type SystemArea = typeof SYSTEM_AREAS[number]["value"];
export function systemAreaTitle(area: SystemArea): string { return SYSTEM_AREAS.find(entry => entry.value === area)!.label; }
export interface WorkspaceLocation { page: PageId; systemArea: SystemArea }

export const PROFILES_LOCATION = { page: "dashboard", systemArea: "profiles" } as const satisfies WorkspaceLocation;
export function workspaceLocationKey({ page, systemArea }: WorkspaceLocation) {
  return page === "dashboard" && systemArea === "profiles" ? "dashboard:profiles" : page;
}
