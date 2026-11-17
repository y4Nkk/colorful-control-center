import { ProfileMultiZonePreview } from "./ProfileMultiZonePreview";
import { ProfileTouchpadPreview } from "./ProfileTouchpadPreview";
import { ProfileMicrophonePreview } from "./ProfileMicrophonePreview";
import { ProfileGraphicsModePreview } from "./ProfileGraphicsModePreview";
import type { ProfileSettings, OemModeTarget, ProfileDisplayReview, ProfileGraphicsModeReview, ProfileGpuPowerReview } from "../../api/client";
import { FAN_LABELS, OEM_POWER_MODES, offsetUnitsToMv, type FanId } from "../../api/client";

import { rpmStepPath } from "../control/rpmCurve";
import { OemModeEffects } from "../control/OemModeEffects";
import { ProfileDevicePreview } from "./ProfileDevicePreview";
import { ProfileDisplayPreview } from "./ProfileDisplayPreview";
import { ProfileGpuPowerPreview } from "./ProfileGpuPowerPreview";
import { FactList } from "../FactList";
import { Surface } from "../Surface";
import { Icon } from "../Icon";

export function ProfilePreview({ settings, modeTarget, displayReview, gpuPowerReview, graphicsModeReview }: { settings: ProfileSettings; modeTarget: OemModeTarget | null; displayReview: ProfileDisplayReview | null; gpuPowerReview: ProfileGpuPowerReview | null; graphicsModeReview: ProfileGraphicsModeReview | null }) {
  const curves = [
    ...Object.entries(settings.fanCurves ?? {}).map(([id, p]) => ({ id, unit: "占空比",
      path: `M${p.t2},${100-p.d2Pct} L${p.t3},${100-p.d3Pct} L100,0` })),
    ...Object.entries(settings.fanRpmCurves ?? {}).map(([id, points]) => ({ id, unit: "最大转速百分比", path: rpmStepPath(points) })),
    ...Object.entries(settings.fanSoftwareCurves ?? {}).map(([id, curve]) => ({ id, unit: `软件占空比 · 回差 ${curve!.hysteresisC}° · 斜坡 ${curve!.rampPctPerS}%/秒`,
      path: `M0,${100 - curve!.points[0].dutyPct} ` + curve!.points.map((p) => `L${p.tempC},${100 - p.dutyPct}`).join(" ") })),
  ];
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
      {settings.oemPowerMode != null && <span className="ccc-chip">电源模式：{OEM_POWER_MODES[settings.oemPowerMode]}</span>}
      {modeTarget && <OemModeEffects target={modeTarget} />}
      {settings.oemPowerMode != null && <span className="ccc-rule-heading ccc-control-caption">原厂模式 <Icon name="arrow-right" size={16} /> 独立参数</span>}
      {curves.length > 0 && <div className="ccc-profile-curves">
        {curves.map(({ id, unit, path }) => <figure key={id}>
          <figcaption>{FAN_LABELS[id as FanId]} · {unit}</figcaption>
          <svg viewBox="0 0 100 100" preserveAspectRatio="none" role="img" aria-label={`${FAN_LABELS[id as FanId]} 档案曲线`}>
            <path d={path} fill="none" stroke="var(--ccc-action)" strokeWidth="2" vectorEffect="non-scaling-stroke" />
          </svg>
          <div className="ccc-profile-curve-axis" aria-hidden="true"><span>0°C</span><span>100°C / 100%</span></div>
        </figure>)}
      </div>}
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
