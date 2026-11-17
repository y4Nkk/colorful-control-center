import { Icon } from "../Icon";
import type { MultiZoneTarget } from "../../api/client";
import { Surface } from "../Surface";
import { MultiZoneColors } from "../control/MultiZoneColors";
import { multiZoneSummary } from "../control/multizone-values";

export function ProfileMultiZonePreview({ target, independentLightBar }: { target: MultiZoneTarget | null; independentLightBar: boolean }) {
  if (!target) return null;
  return <Surface as="section" variant="group" className="ccc-profile-device" aria-label="档案多区键盘">
    <div className="ccc-profile-values"><h3 className="ccc-icon-title"><Icon name="lighting" />多区键盘</h3><strong>{multiZoneSummary(target.values)}</strong>
      {!target.capabilities.lightBarEnabled && <p className="ccc-control-caption">平台灯条区关闭，原颜色保持。</p>}
      {independentLightBar && <p className="ccc-control-caption">独立灯条效果在多区设置后发送。</p>}
      <p className="ccc-control-caption">闲置关灯及开机覆盖使用独立灯光策略。</p>
    </div><MultiZoneColors values={target.values} label="档案多区颜色预设" />
  </Surface>;
}
