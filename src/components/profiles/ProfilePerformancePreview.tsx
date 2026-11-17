import { Icon } from "../Icon";
import type { ProfileSettings } from "../../api/profiles";
import { Surface } from "../Surface";
import { FactList } from "../FactList";
import { PERFORMANCE_FIELDS, performanceState } from "../control/OemPerformanceOptions";
import { Collapsible } from "../Collapsible";

export function ProfilePerformancePreview({ settings }: { settings: ProfileSettings }) {
  const values = settings.performanceOptions;
  if (!values) return null;
  const mode = settings.oemPowerMode;
  return <Surface as="section" variant="group" className="ccc-profile-device" aria-label="档案性能选项">
    <div className="ccc-profile-values"><h3 className="ccc-icon-title"><Icon name="performance" />性能选项</h3>
      <strong>{mode === 2 ? "随性能模式应用" : mode == null ? "按当前模式应用" : "保存性能模式偏好"}</strong>
      <Collapsible summary="应用顺序">
      <p className="ccc-control-caption">{mode === 2 ? "选项合入已预览的模式命令。" : mode == null ? "性能模式立即应用；其他模式只保存偏好。" : "进入性能模式后使用这些偏好；本次不激活它们。"}</p>
      {(mode == null || mode === 2) && <p className="ccc-control-caption">Turbo 变更先切回自动风扇，再应用档案风扇目标。</p>}
      {(mode == null || mode === 2) && settings.fanMode == null && !settings.fanCurves && !settings.fanRpmCurves && !settings.fanSoftwareCurves
        && <p className="ccc-control-caption">未指定风扇目标时，临时释放的软件曲线会恢复。</p>}
      </Collapsible>
    </div>
    <FactList compact layout="tiles" facts={PERFORMANCE_FIELDS.filter(field => values[field.key] !== null).map(field => ({
      label: field.label, value: performanceState(values[field.key]),
    }))} />
  </Surface>;
}
