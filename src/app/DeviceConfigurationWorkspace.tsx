import { useEffect, useState } from "react";
import { DisplaySettings } from "../components/control/DisplaySettings";
import { AutomaticRefreshSettings } from "../components/control/AutomaticRefreshSettings";
import { GpuPowerSettings } from "../components/control/GpuPowerSettings";
import { GraphicsModeSettings } from "../components/control/GraphicsModeSettings";
import { KeyboardSettings } from "../components/control/KeyboardSettings";
import { TouchpadSettings } from "../components/control/TouchpadSettings";
import { MicrophoneSettings } from "../components/control/MicrophoneSettings";
import { AirplaneSettings } from "../components/control/AirplaneSettings";
import type { HotkeyNavigationRequest } from "../api/client";
import type { HardwareWorkspaceProps } from "./useControlModel";
import { useHotkeyNavigation } from "./useHotkeyNavigation";
type Props = HardwareWorkspaceProps & { navigationRequest: HotkeyNavigationRequest | null; onNavigated: (id: number) => Promise<boolean> };
export function DeviceConfigurationWorkspace({ active, model, navigationRequest, onNavigated }: Props) {
  const anchor = useHotkeyNavigation(active, navigationRequest, onNavigated, "display");
  const [rotationRequest, setRotationRequest] = useState<number | null>(null);
  useEffect(() => { if (navigationRequest?.kind === "displayRotation") setRotationRequest(navigationRequest.id); }, [navigationRequest?.id, navigationRequest?.kind]);
  return <div className="ccc-control">
    <div className="ccc-module-grid" data-module-layout="featured">
      <div ref={anchor} role="region" aria-label="显示控制区域" tabIndex={-1} className="ccc-control">
        <DisplaySettings active={active} revision={model.deviceRevision} rotationRequest={rotationRequest} />
      </div>
      <AutomaticRefreshSettings active={active} revision={model.deviceRevision} />
    </div>
    <div className="ccc-module-grid" data-module-layout="adaptive">
      <GraphicsModeSettings active={active} revision={model.deviceRevision} />
      <GpuPowerSettings active={active} revision={model.deviceRevision} onSettled={() => model.setRevisions(previous => ({ ...previous, gpu: previous.gpu + 1 }))} />
    </div>
    <KeyboardSettings active={active} revision={model.deviceRevision} />
    <div className="ccc-module-grid" data-module-layout="devices">
      <TouchpadSettings active={active} revision={model.deviceRevision} />
      <MicrophoneSettings active={active} revision={model.deviceRevision} />
      <AirplaneSettings active={active} revision={model.deviceRevision} />
    </div>
  </div>;
}
