import { useCallback, useEffect, useRef, useState } from "react";
import { listProfiles, loadProfile, previewProfileAuthorization, previewProfileDisplay, previewProfileGpuPower, previewProfileGraphicsMode, type ProfileFile, type ProfileMeta, type OemModeTarget, type ProfileModeApproval, type ProfileDisplayReview, type ProfileGpuPowerReview, type ProfileGraphicsModeReview } from "../api/client";

export interface ProfileSelection { file: ProfileFile; modeTarget: OemModeTarget | null; modeAuthorization: ProfileModeApproval | null; modeError: string | null; displayReview: ProfileDisplayReview | null; displayError: string | null; gpuPowerReview: ProfileGpuPowerReview | null; gpuPowerError: string | null; graphicsModeReview: ProfileGraphicsModeReview | null; graphicsModeError: string | null }

export async function loadBackgroundProfile(name: string): Promise<ProfileSelection> {
  const file = await loadProfile(name);
  const [mode, display, gpuPower, graphicsMode] = await Promise.allSettled([
    file.settings.oemPowerMode == null ? Promise.resolve(null) : previewProfileAuthorization(file.settings),
    file.settings.display == null ? Promise.resolve(null) : previewProfileDisplay(file.settings),
    file.settings.gpuPower == null ? Promise.resolve(null) : previewProfileGpuPower(file.settings),
    file.settings.graphicsMode == null ? Promise.resolve(null) : previewProfileGraphicsMode(file.settings),
  ]);
  const modeAuthorization = mode.status === "fulfilled" ? mode.value : null;
  const modeTarget = modeAuthorization?.target ?? null;
  const displayReview = display.status === "fulfilled" ? display.value : null;
  const gpuPowerReview = gpuPower.status === "fulfilled" ? gpuPower.value : null;
  const graphicsModeReview = graphicsMode.status === "fulfilled" ? graphicsMode.value : null;
  return { file, modeTarget, modeAuthorization, displayReview, gpuPowerReview, graphicsModeReview,
    modeError: mode.status === "rejected" ? String(mode.reason) : file.settings.oemPowerMode != null && !modeTarget ? "当前设备不能预览此档案的电源模式" : null,
    displayError: display.status === "rejected" ? String(display.reason) : file.settings.display != null && !displayReview ? "当前设备不能预览此档案的显示配置" : null,
    gpuPowerError: gpuPower.status === "rejected" ? String(gpuPower.reason) : file.settings.gpuPower != null && !gpuPowerReview ? "当前设备不能预览此档案的独显配置" : null,
    graphicsModeError: graphicsMode.status === "rejected" ? String(graphicsMode.reason) : file.settings.graphicsMode != null && !graphicsModeReview ? "当前设备不能预览此档案的显卡模式" : null };
}

export function useProfileList(active: boolean, reportError: (error: string | null) => void) {
  const [profiles, setProfiles] = useState<ProfileMeta[]>([]);
  const [status, setStatus] = useState<"loading" | "ready" | "error">("loading");
  const generation = useRef(0);
  const reload = useCallback(async () => {
    const id = ++generation.current;
    setStatus("loading");
    try {
      const next = await listProfiles();
      if (id === generation.current) { setProfiles(next); setStatus("ready"); reportError(null); }
    } catch (error) {
      if (id === generation.current) { setStatus("error"); throw error; }
    }
  }, [reportError]);
  useEffect(() => {
    if (!active) return;
    void reload().catch((error) => reportError(String(error)));
    return () => { ++generation.current; };
  }, [active, reload, reportError]);
  return { profiles, reload, status };
}
