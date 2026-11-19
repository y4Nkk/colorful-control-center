import type { StartupPage } from "../api/client";
import { PAGES, PROFILES_LOCATION, workspaceLocationKey, type WorkspaceLocation } from "../app/navigation";

const KEY = "ccc-last-page";
export function startupPage(preference: StartupPage): WorkspaceLocation {
  if (preference === "dashboard") return { page: "dashboard", systemArea: "overview" };
  const saved = localStorage.getItem(KEY);
  if (saved === "profiles") {
    rememberPage(PROFILES_LOCATION);
    return PROFILES_LOCATION;
  }
  if (saved === workspaceLocationKey(PROFILES_LOCATION)) return PROFILES_LOCATION;
  let page = PAGES.find(entry => entry.id === saved)?.id;
  if (saved === "battery" || saved === "automation") page = "power";
  if (saved === "lighting" || saved === "input") page = "keyboard";
  const location = { page: page ?? "dashboard", systemArea: "overview" } satisfies WorkspaceLocation;
  if (page && page !== saved) rememberPage(location);
  return location;
}
export function rememberPage(location: WorkspaceLocation) {
  localStorage.setItem(KEY, workspaceLocationKey(location));
}
