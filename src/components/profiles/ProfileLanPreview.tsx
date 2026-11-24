import { Icon } from "../Icon";
import type { ProfileSettings, LanWritableField, LanValue } from "../../api/client";
import { Surface } from "../Surface";
import { FactList } from "../FactList";
import { LAN_FIELDS, lanMeaning } from "../control/lan-values";

export function ProfileLanPreview({ settings }: { settings: ProfileSettings }) {
  return settings.lanPolicy?.devices.map(device => {
    const fields = Object.entries(device.values) as [LanWritableField, LanValue | null][];
    return <Surface as="section" variant="group" className="ccc-profile-device" key={device.identity.instance}
      aria-label={`档案网络策略 · ${device.name}`}>
      <div className="ccc-profile-values"><h3 className="ccc-icon-title"><Icon name="network" />网络驱动策略</h3><strong>{device.name}</strong>
        <span className="ccc-control-caption">需系统授权</span>
        {fields.some(([, value]) => value === null) && <p className="ccc-control-caption">未设置项会清除现有配置，恢复到未设置状态。</p>}
      </div>
      <FactList compact facts={fields.map(([field, value]) => {
        const meaning = lanMeaning(field, value);
        return { label: LAN_FIELDS[field], value: value === null ? "恢复为未设置" : meaning === "现有驱动配置" ? "保存的驱动配置" : meaning };
      })} />
    </Surface>;
  });
}
