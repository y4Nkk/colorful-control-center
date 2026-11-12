/**
 * 页面注册表：一级工作区的唯一接入清单。新增页面走 AGENTS.md
 * 「全局化接入契约」，页标识永远用 PageId，不用显示文本。
 */

export type PageId = "dashboard" | "fans" | "performance" | "profiles" | "settings";

export interface PageDefinition {
  id: PageId;
  title: string;
}

export const PAGES: readonly PageDefinition[] = [
  { id: "dashboard", title: "监控仪表盘" },
  { id: "fans", title: "风扇曲线" },
  { id: "performance", title: "性能调校" },
  { id: "profiles", title: "性能档案" },
  { id: "settings", title: "设置" },
];

export function pageTitle(page: PageId): string {
  const definition = PAGES.find((entry) => entry.id === page);
  return definition?.title ?? page;
}
