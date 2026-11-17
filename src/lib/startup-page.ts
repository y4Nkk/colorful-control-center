import type { StartupPage } from "../api/client";
import { PAGES, type PageId } from "../app/navigation";

const KEY = "ccc-last-page";
export function startupPage(preference: StartupPage): PageId {
  if (preference === "dashboard") return "dashboard";
  const saved = localStorage.getItem(KEY);
  return PAGES.find(page => page.id === saved)?.id ?? "dashboard";
}
export function rememberPage(page: PageId) { localStorage.setItem(KEY, page); }
