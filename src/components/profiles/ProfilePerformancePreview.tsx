import { Icon } from "../Icon";
import type { ProfileSettings } from "../../api/profiles";
import { Surface } from "../Surface";
import { FactList } from "../FactList";
import { PERFORMANCE_FIELDS, performanceState } from "../control/OemPerformanceOptions";
import { DetailsDialog } from "../DetailsDialog";

export function ProfilePerformancePreview({ settings }: { settings: ProfileSettings }) {
  const values = settings.performanceOptions;
  if (!values) return null;
  const mode = settings.oemPowerMode;
  const canApplyOptions = mode == null || mode === 2 || mode === 3;
  const summary = mode === 2 ? "随性能模式应用" : mode === 3 ? "按娱乐模式能力应用" : mode == null ? "按当前模式应用" : "保存性能选项偏好";
  return <Surface as="section" variant="group" className="ccc-profile-device" aria-label="档案性能选项">
    <div className="ccc-profile-values"><h3 className="ccc-icon-title"><Icon name="performance" />性能选项</h3>
      <strong>{summary}</strong>
      <DetailsDialog title="应用顺序">
      <p className="ccc-control-caption">{canApplyOptions ? "娱乐与性能模式按固件能力应用性能选项。模式联动目标会列出将开启的选项。" : "将选项保存为偏好。"}</p>
      {canApplyOptions && <p className="ccc-control-caption">Turbo 风扇生效时，先应用自动风扇策略，再应用档案风扇目标。</p>}
      {canApplyOptions && settings.fanMode == null && !settings.fanCurves && !settings.fanRpmCurves && !settings.fanSoftwareCurves
        && <p className="ccc-control-caption">未指定风扇目标时，临时释放的软件曲线会恢复。</p>}
      </DetailsDialog>
    </div>
    <FactList compact layout="tiles" facts={PERFORMANCE_FIELDS.filter(field => values[field.key] !== null).map(field => ({
      label: field.label, value: performanceState(values[field.key]),
    }))} />
  </Surface>;
}
