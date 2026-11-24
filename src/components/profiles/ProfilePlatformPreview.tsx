import { Icon } from "../Icon";
import type { ProfileSettings } from "../../api/profiles";
import { USB_POLICY_LABEL } from "../../api/usb";
import { FactList } from "../FactList";
import { Surface } from "../Surface";
import { pdLevelText, USB_PLAN_NAMES } from "../control/platform-values";
import { PowerSourceVisual } from "../control/PowerSourceVisual";

export function ProfilePlatformPreview({ settings }: { settings: ProfileSettings }) {
  const pd = settings.powerDelivery, usb = settings.usbPolicy;
  const zeroBased = pd?.encoding === "legacyZeroBased";
  const state = (value: boolean | null) => value === null ? "保留现值" : value ? "开启" : "关闭";
  return <>
    {pd && <Surface as="section" variant="group" className="ccc-profile-device" aria-label="档案PD档位">
      <div className="ccc-profile-values"><h3 className="ccc-icon-title"><Icon name="plug" />PD 档位</h3>
        <strong>{pdLevelText(pd.level, zeroBased)}</strong>
        <PowerSourceVisual source="adapter" watts={pd.adapterWatts} label="档案适配器报告" />
      </div>
      <div className="ccc-profile-values"><span className="ccc-control-caption">档案绑定的候选</span>
        <div className="ccc-profile-pd-levels" role="list" aria-label="档案PD候选等级">
          {pd.availableLevels.map(level => <span role="listitem" key={level} data-selected={level === pd.level}>
            {pdLevelText(level, zeroBased)}{level === pd.level ? " · 目标" : ""}
          </span>)}
        </div>
      </div>
    </Surface>}
    {usb && <Surface as="section" variant="group" className="ccc-profile-device" aria-label={`档案 ${USB_POLICY_LABEL}`}>
      <div className="ccc-profile-values"><h3 className="ccc-icon-title"><Icon name="usb" />{USB_POLICY_LABEL}</h3><strong>{usb.plans.length} 个标准计划</strong>
      </div>
      <FactList compact facts={usb.plans.map(plan => ({ label: USB_PLAN_NAMES[plan.id],
        value: `插电：${state(plan.ac)}；电池：${state(plan.dc)}` }))} />
    </Surface>}
  </>;
}
