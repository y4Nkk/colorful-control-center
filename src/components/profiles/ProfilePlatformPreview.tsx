import { Icon } from "../Icon";
import type { ProfileSettings } from "../../api/profiles";
import { USB_POLICY_LABEL } from "../../api/usb";
import { FactList } from "../FactList";
import { Surface } from "../Surface";
import { pdLevelText, USB_PLAN_NAMES } from "../control/platform-values";
import { PowerSourceVisual } from "../control/PowerSourceVisual";
import { DetailsDialog } from "../DetailsDialog";

export function ProfilePlatformPreview({ settings }: { settings: ProfileSettings }) {
  const pd = settings.powerDelivery, usb = settings.usbPolicy;
  const zeroBased = pd?.encoding === "legacyZeroBased";
  const state = (value: boolean | null) => value === null ? "保留现值" : value ? "开启" : "关闭";
  return <>
    {pd && <Surface as="section" variant="group" className="ccc-profile-device" aria-label="档案PD档位">
      <div className="ccc-profile-values"><h3 className="ccc-icon-title"><Icon name="plug" />PD 档位</h3>
        <strong>{pdLevelText(pd.level, zeroBased)}</strong>
        <PowerSourceVisual source="adapter" watts={pd.adapterWatts} label="档案适配器报告" />
        <DetailsDialog title="PD应用检查"><p>应用时核对编号、适配器及候选等级。</p></DetailsDialog>
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
        <DetailsDialog title={`${USB_POLICY_LABEL}应用检查`}><p className="ccc-control-caption">按计划分别应用插电和电池设置，不创建缺失计划。</p>
        <p className="ccc-control-caption">跟随档案已确认的模式切换；其他活动计划变化会中止执行。</p></DetailsDialog>
      </div>
      <FactList compact facts={usb.plans.map(plan => ({ label: USB_PLAN_NAMES[plan.id],
        value: `插电：${state(plan.ac)}；电池：${state(plan.dc)}` }))} />
    </Surface>}
  </>;
}
