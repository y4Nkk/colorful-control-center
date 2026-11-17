import { KeyboardLighting } from "../components/control/KeyboardLighting";
import { MultiZoneLighting } from "../components/control/MultiZoneLighting";
import { LightingOptions } from "../components/control/LightingOptions";
import { LightBarSettings } from "../components/control/LightBarSettings";
import type { HotkeyNavigationRequest } from "../api/client";
import { useHotkeyNavigation } from "./useHotkeyNavigation";
import type { HardwareWorkspaceProps } from "./useControlModel";

type Props = HardwareWorkspaceProps & { navigationRequest: HotkeyNavigationRequest | null; onNavigated: (id: number) => Promise<boolean> };
export function LightingWorkspace({ active, model, navigationRequest, onNavigated }: Props) {
  const anchor = useHotkeyNavigation(active, navigationRequest, onNavigated, "keyboard");
  return <div ref={anchor} role="region" aria-label="键盘灯光区域" tabIndex={-1} className="ccc-control">
    <KeyboardLighting active={active} revision={model.deviceRevision} />
    <MultiZoneLighting active={active} revision={model.deviceRevision} />
    <LightingOptions active={active} revision={model.deviceRevision} />
    <LightBarSettings active={active} revision={model.deviceRevision} />
  </div>;
}
