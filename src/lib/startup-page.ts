import type { StartupPage } from "../api/client";
import { DEFAULT_LOCATION, PAGES, PROFILES_LOCATION, workspaceLocationKey, type WorkspaceLocation } from "../app/navigation";

const KEY = "ccc-last-page";
export function startupPage(preference: StartupPage): WorkspaceLocation {
  if (preference === "dashboard") return DEFAULT_LOCATION;
  const saved = localStorage.getItem(KEY);
  const profiles = { ...DEFAULT_LOCATION, ...PROFILES_LOCATION };
  if (saved === "profiles" || saved === "dashboard:profiles") {
    rememberPage(profiles);
    return profiles;
  }
  if (saved === workspaceLocationKey(profiles)) return profiles;
  let page = PAGES.find(entry => entry.id === saved)?.id;
  if (saved === "battery" || saved === "automation") page = "power";
  if (saved === "lighting" || saved === "input") page = "keyboard";
  const location = { ...DEFAULT_LOCATION, page: page ?? DEFAULT_LOCATION.page };
  if (page && page !== saved) rememberPage(location);
  return location;
}
export function rememberPage(location: WorkspaceLocation) {
  localStorage.setItem(KEY, workspaceLocationKey(location));
}
