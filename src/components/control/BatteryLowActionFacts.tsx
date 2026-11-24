import type { BatteryLowActionProfile } from "../../api/client";
import { Surface } from "../Surface";
import { DetailsDialog } from "../DetailsDialog";
import { Icon } from "../Icon";

const action = (value: number) => ["不操作", "睡眠", "休眠", "按电源计划关机"][value] ?? "未知";
export function BatteryLowActionFacts({ profile, powerSummary }: { profile: BatteryLowActionProfile; powerSummary: string | null }) {
  const current = profile.current;
  return <section className="ccc-battery-action-facts" aria-label="原厂低电量系统动作事实与预览">
    <strong>原厂低电量系统动作</strong>
    <div className="ccc-battery-action-columns">
      <Surface variant="inset" className="ccc-battery-action-part">
        <strong>当前电池计划</strong>
        {current ? <>
          <p>低电量：{action(current.low)}</p>
          <p>严重电量：{action(current.critical)}</p>
          {powerSummary && <p className="ccc-control-caption">{powerSummary}</p>}
        </> : <p className="ccc-control-caption">当前电池计划动作不可读取。</p>}
        {profile.error && <p className="ccc-control-error"><Icon name="error" size={16} />{profile.error}</p>}
        {profile.powerError && <p className="ccc-control-error"><Icon name="error" size={16} />{profile.powerError}</p>}
      </Surface>
      <Surface variant="inset" className="ccc-battery-action-part">
        <strong>执行条件</strong>
        <span className="ccc-rule-heading"><Icon name="battery" />仅电池供电时触发</span>
        <span className="ccc-rule-heading"><Icon name="sleep" />按当前电源计划睡眠或休眠</span>
        <DetailsDialog title="事件与恢复说明">
        <p className="ccc-control-caption">收到原厂的电量事件后，本软件会等 1 秒；如果此时电脑正在用电池，就按上面「当前电池计划」里的设置让电脑睡眠或休眠。已经插上电源，或者无法确认供电状态时，不会操作。</p>
        <p className="ccc-control-caption">电脑进入睡眠或休眠后，本软件不会自动把它唤醒。想避免再次触发，可以临时停止监听，或恢复接管前的热键设置。Windows 自己的电源计划也可能单独让电脑睡眠或休眠。</p>
        </DetailsDialog>
      </Surface>
    </div>
  </section>;
}
