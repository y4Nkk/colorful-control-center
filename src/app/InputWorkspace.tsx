import { KeyboardSettings } from "../components/control/KeyboardSettings";
import { TouchpadSettings } from "../components/control/TouchpadSettings";
import { MicrophoneSettings } from "../components/control/MicrophoneSettings";
import { AirplaneSettings } from "../components/control/AirplaneSettings";
import { KeymapSettings } from "../components/keymap/KeymapSettings";
import { HotkeySettings } from "../components/control/HotkeySettings";
import type { HardwareWorkspaceProps } from "./useControlModel";
export function InputWorkspace({ active, model }: HardwareWorkspaceProps) {
  return <div className="ccc-control">
    <KeyboardSettings active={active} revision={model.deviceRevision} />
    <div className="ccc-device-grid">
      <TouchpadSettings active={active} revision={model.deviceRevision} />
      <MicrophoneSettings active={active} revision={model.deviceRevision} />
      <AirplaneSettings active={active} revision={model.deviceRevision} />
    </div>
    <div className="ccc-device-pair">
      <KeymapSettings active={active} revision={model.deviceRevision} />
      <HotkeySettings active={active} revision={model.deviceRevision} />
    </div>
  </div>;
}
