import { KeyboardLighting } from "../components/control/KeyboardLighting";
import { MultiZoneLighting } from "../components/control/MultiZoneLighting";
import { LightingOptions } from "../components/control/LightingOptions";
import { LightBarSettings } from "../components/control/LightBarSettings";
import { KeymapSettings } from "../components/keymap/KeymapSettings";
import { HotkeySettings } from "../components/control/HotkeySettings";
import type { HotkeyNavigationRequest } from "../api/client";
import { useHotkeyNavigation } from "./useHotkeyNavigation";
import type { HardwareWorkspaceProps } from "./useControlModel";

type Props = HardwareWorkspaceProps & { navigationRequest: HotkeyNavigationRequest | null; onNavigated: (id: number) => Promise<boolean> };
export function KeyboardWorkspace({ active, model, navigationRequest, onNavigated }: Props) {
  const anchor = useHotkeyNavigation(active, navigationRequest, onNavigated, "keyboard");
  const revision = model.deviceRevision;
  return <div className="ccc-control">
    <div ref={anchor} role="region" aria-label="键盘灯光区域" tabIndex={-1} className="ccc-control">
      <KeyboardLighting active={active} revision={revision} />
      <MultiZoneLighting active={active} revision={revision} />
    </div>
    <div className="ccc-module-grid" data-module-layout="adaptive">
      <KeymapSettings active={active} revision={revision} />
      <HotkeySettings active={active} revision={revision} />
    </div>
    <LightingOptions active={active} revision={revision} />
    <LightBarSettings active={active} revision={revision} />
  </div>;
}
