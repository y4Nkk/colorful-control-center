import { useCallback, useEffect, useRef, useState } from "react";
import { listProfiles, loadProfile, previewProfileMode, previewProfileAuthorization, previewProfileDisplay, previewProfileGpuPower, previewProfileGraphicsMode, type ProfileFile, type ProfileMeta, type OemModeTarget, type ProfileModeApproval, type ProfileDisplayReview, type ProfileGpuPowerReview, type ProfileGraphicsModeReview } from "../api/client";

export interface ProfileSelection { file: ProfileFile; modeTarget: OemModeTarget | null; modeAuthorization: ProfileModeApproval | null; modeError: string | null; displayReview: ProfileDisplayReview | null; displayError: string | null; gpuPowerReview: ProfileGpuPowerReview | null; gpuPowerError: string | null; graphicsModeReview: ProfileGraphicsModeReview | null; graphicsModeError: string | null }

export async function loadProfileSelection(name: string, purpose: "manual" | "background"): Promise<ProfileSelection> {
  const file = await loadProfile(name);
  const [mode, display, gpuPower, graphicsMode] = await Promise.allSettled([
    file.settings.oemPowerMode == null ? Promise.resolve({ target: null, approval: null })
      : purpose === "background" ? previewProfileAuthorization(file.settings).then(approval => ({ target: approval?.target ?? null, approval }))
      : previewProfileMode(file.settings).then(target => ({ target, approval: null })),
    file.settings.display == null ? Promise.resolve(null) : previewProfileDisplay(file.settings),
    file.settings.gpuPower == null ? Promise.resolve(null) : previewProfileGpuPower(file.settings),
    file.settings.graphicsMode == null ? Promise.resolve(null) : previewProfileGraphicsMode(file.settings),
  ]);
  const modeTarget = mode.status === "fulfilled" ? mode.value.target : null;
  const displayReview = display.status === "fulfilled" ? display.value : null;
  const gpuPowerReview = gpuPower.status === "fulfilled" ? gpuPower.value : null;
  const graphicsModeReview = graphicsMode.status === "fulfilled" ? graphicsMode.value : null;
  return { file, modeTarget, modeAuthorization: mode.status === "fulfilled" ? mode.value.approval : null, displayReview, gpuPowerReview, graphicsModeReview,
    modeError: mode.status === "rejected" ? String(mode.reason) : file.settings.oemPowerMode != null && !modeTarget ? "当前设备不能预览此档案的电源模式" : null,
    displayError: display.status === "rejected" ? String(display.reason) : file.settings.display != null && !displayReview ? "当前设备不能预览此档案的显示配置" : null,
    gpuPowerError: gpuPower.status === "rejected" ? String(gpuPower.reason) : file.settings.gpuPower != null && !gpuPowerReview ? "当前设备不能预览此档案的独显配置" : null,
    graphicsModeError: graphicsMode.status === "rejected" ? String(graphicsMode.reason) : file.settings.graphicsMode != null && !graphicsModeReview ? "当前设备不能预览此档案的显卡模式" : null };
}

export function useProfileList(active: boolean, reportError: (error: string) => void) {
  const [profiles, setProfiles] = useState<ProfileMeta[]>([]);
  const generation = useRef(0);
  const reload = useCallback(async () => {
    const id = ++generation.current;
    try {
      const next = await listProfiles();
      if (id === generation.current) setProfiles(next);
    } catch (error) { if (id === generation.current) throw error; }
  }, []);
  useEffect(() => { if (active) void reload().catch((error) => reportError(String(error))); }, [active, reload, reportError]);
  return { profiles, reload };
}
