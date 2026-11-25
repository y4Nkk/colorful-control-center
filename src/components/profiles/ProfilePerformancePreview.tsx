import { Icon } from "../Icon";
import type { ProfileSettings } from "../../api/profiles";
import { Surface } from "../Surface";
import { FactList } from "../FactList";
import { PERFORMANCE_OPTION_LABELS } from "../../api/power";
import { performanceState } from "../control/format";

export function ProfilePerformancePreview({ settings }: { settings: ProfileSettings }) {
  const values = settings.performanceOptions;
  if (!values) return null;
  const mode = settings.oemPowerMode;
  const summary = mode === 2 ? "随性能模式应用" : mode === 3 ? "按娱乐模式能力应用" : mode == null ? "按当前模式应用" : "保存性能选项偏好";
  return <Surface as="section" variant="group" className="ccc-profile-device" aria-label="档案性能选项">
    <div className="ccc-profile-values"><h3 className="ccc-icon-title"><Icon name="performance" />性能选项</h3>
      <strong>{summary}</strong>
    </div>
    <FactList compact layout="tiles" facts={(["turboFan", "dtt"] as const).filter(key => values[key] !== null).map(key => ({
      label: PERFORMANCE_OPTION_LABELS[key], value: performanceState(values[key]),
    }))} />
  </Surface>;
}
