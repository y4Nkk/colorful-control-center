import { Icon } from "../Icon";
import type { ProfileSettings, LanWritableField, LanValue } from "../../api/client";
import { Surface } from "../Surface";
import { FactList } from "../FactList";
import { Collapsible } from "../Collapsible";
import { LAN_FIELDS, lanMeaning, lanValue } from "../control/lan-values";

export function ProfileLanPreview({ settings }: { settings: ProfileSettings }) {
  return settings.lanPolicy?.devices.map(device => {
    const fields = Object.entries(device.values) as [LanWritableField, LanValue | null][];
    return <Surface as="section" variant="group" className="ccc-profile-device" key={device.identity.instance}
      aria-label={`档案网络策略 · ${device.name}`}>
      <div className="ccc-profile-values"><h3 className="ccc-icon-title"><Icon name="network" />网络驱动策略</h3><strong>{device.name}</strong>
        <span className="ccc-chip">需系统授权</span>
        {fields.some(([, value]) => value === null) && <p className="ccc-control-caption">未设置项会清除现有配置，恢复到未设置状态。</p>}
        <Collapsible summary="网卡身份与原始配置">
          <FactList compact facts={[
            { label: "设备实例", value: device.identity.instance },
            { label: "驱动绑定", value: device.identity.driver },
            { label: "驱动版本", value: lanValue(device.identity.driverVersion) },
            ...fields.map(([field, value]) => ({ label: LAN_FIELDS[field], value: lanValue(value) })),
          ]} />
        </Collapsible>
      </div>
      <FactList compact facts={fields.map(([field, value]) => {
        const meaning = lanMeaning(field, value);
        return { label: LAN_FIELDS[field], value: value === null ? "恢复为未设置" : meaning === "现有驱动配置" ? "保存的驱动配置" : meaning };
      })} />
    </Surface>;
  });
}
