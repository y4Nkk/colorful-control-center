import { useEffect, useState } from "react";
import { DisplaySettings } from "../components/control/DisplaySettings";
import { AutomaticRefreshSettings } from "../components/control/AutomaticRefreshSettings";
import { GpuPowerSettings } from "../components/control/GpuPowerSettings";
import { GraphicsModeSettings } from "../components/control/GraphicsModeSettings";
import type { HotkeyNavigationRequest } from "../api/client";
import type { HardwareWorkspaceProps } from "./useControlModel";
import { useHotkeyNavigation } from "./useHotkeyNavigation";
type Props = HardwareWorkspaceProps & { navigationRequest: HotkeyNavigationRequest | null; onNavigated: (id: number) => Promise<boolean> };
export function GraphicsWorkspace({ active, model, navigationRequest, onNavigated }: Props) {
  const anchor = useHotkeyNavigation(active, navigationRequest, onNavigated, "display");
  const [rotationRequest, setRotationRequest] = useState<number | null>(null);
  useEffect(() => { if (navigationRequest?.kind === "displayRotation") setRotationRequest(navigationRequest.id); }, [navigationRequest?.id, navigationRequest?.kind]);
  return <div className="ccc-control">
    <div ref={anchor} role="region" aria-label="显示控制区域" tabIndex={-1} className="ccc-device-anchor">
      <DisplaySettings active={active} revision={model.deviceRevision} rotationRequest={rotationRequest} />
    </div>
    <div className="ccc-device-pair">
      <AutomaticRefreshSettings active={active} revision={model.deviceRevision} />
      <GraphicsModeSettings active={active} revision={model.deviceRevision} />
    </div>
    <GpuPowerSettings active={active} revision={model.deviceRevision} onSettled={() => model.setRevisions(previous => ({ ...previous, gpu: previous.gpu + 1 }))} />
  </div>;
}
