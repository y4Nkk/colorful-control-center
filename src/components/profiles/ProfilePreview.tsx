import { ProfileMultiZonePreview } from "./ProfileMultiZonePreview";
import { ProfileTouchpadPreview } from "./ProfileTouchpadPreview";
import { ProfileMicrophonePreview } from "./ProfileMicrophonePreview";
import { ProfileGraphicsModePreview } from "./ProfileGraphicsModePreview";
import type { ProfileSettings, OemModeTarget, ProfileDisplayReview, ProfileGraphicsModeReview, ProfileGpuPowerReview } from "../../api/client";
import { OEM_POWER_MODES, offsetUnitsToMv } from "../../api/client";
import { OemModeEffects } from "../control/OemModeEffects";
import { ProfileDevicePreview } from "./ProfileDevicePreview";
import { ProfileDisplayPreview } from "./ProfileDisplayPreview";
import { ProfileGpuPowerPreview } from "./ProfileGpuPowerPreview";
import { FactList } from "../FactList";
import { Surface } from "../Surface";
import { Icon } from "../Icon";
import { ProfileCurves } from "./ProfileCurves";

export function ProfilePreview({ settings, modeTarget, displayReview, gpuPowerReview, graphicsModeReview }: { settings: ProfileSettings; modeTarget: OemModeTarget | null; displayReview: ProfileDisplayReview | null; gpuPowerReview: ProfileGpuPowerReview | null; graphicsModeReview: ProfileGraphicsModeReview | null }) {
  const saved = [
    { label: "持续功耗上限", value: settings.pl1W, unit: "W" },
    { label: "短时功耗上限", value: settings.pl2W, unit: "W" },
    { label: "核心电压偏移", value: settings.coreOffsetMv, unit: "mV" },
    { label: "核显电压偏移", value: settings.gtOffsetMv, unit: "mV" },
  ];
  const cpu = settings.runtimeCpu;
  const voltage = (units: number | null | undefined) => units == null ? null : offsetUnitsToMv(units);
  const runtime = [
    { label: "持续功耗上限", value: cpu?.pl1W, unit: "W" },
    { label: "短时功耗上限", value: cpu?.pl2W, unit: "W" },
    { label: "核心电压偏移", value: voltage(cpu?.coreOffsetUnits), unit: "mV" },
    { label: "核显电压偏移", value: voltage(cpu?.gtOffsetUnits), unit: "mV" },
  ];
  return (
    <div className="ccc-profile-preview" aria-label="档案参数预览">
      {settings.oemPowerMode != null && <span className="ccc-control-caption">电源模式：{OEM_POWER_MODES[settings.oemPowerMode]}</span>}
      {modeTarget && <OemModeEffects target={modeTarget} />}
      {settings.oemPowerMode != null && <span className="ccc-rule-heading ccc-control-caption">原厂模式 <Icon name="arrow-right" size={16} /> 独立参数</span>}
      <ProfileCurves settings={settings} />
      <ValueCards label="立即应用 · CPU" values={runtime} />
      <ValueCards label="保存设置 · CPU" values={saved} />
      <ValueCards label="立即应用 · GPU" values={[
        { label: "核心频率偏移", value: settings.gpu?.coreOffsetMhz, unit: "MHz" },
        { label: "显存频率偏移", value: settings.gpu?.memOffsetMhz, unit: "MHz" },
      ]} />
      <ProfileDevicePreview settings={settings} />
      <ProfileMicrophonePreview target={settings.microphones} />
      <ProfileTouchpadPreview target={settings.touchpad} />
      <ProfileDisplayPreview target={settings.display} review={displayReview} />
      <ProfileMultiZonePreview target={settings.multiZoneLighting} independentLightBar={settings.lightBar != null} />
      <ProfileGraphicsModePreview target={settings.graphicsMode} review={graphicsModeReview} />
      <ProfileGpuPowerPreview target={settings.gpuPower} review={gpuPowerReview} />
    </div>
  );
}

function ValueCards({ label, values }: { label: string; values: { label: string; value: number | null | undefined; unit: string }[] }) {
  const visible = values.filter(field => field.value != null);
  if (!visible.length) return null;
  return <Surface as="section" variant="group" className="ccc-profile-device" aria-label={label}>
    <div className="ccc-profile-values"><h3 className="ccc-icon-title"><Icon name={label.includes("GPU") ? "gpu" : "cpu"} />{label}</h3></div>
    <FactList compact layout="tiles" facts={visible.map(field => ({ label: field.label, value:
      <output className="ccc-mono" aria-label={`${label} ${field.label}`}>{Number(field.value!.toFixed(3))} {field.unit}</output>,
    }))} />
  </Surface>;
}
