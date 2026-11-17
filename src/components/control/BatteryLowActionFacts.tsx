import type { BatteryLowActionProfile } from "../../api/client";
import { Surface } from "../Surface";
import { Collapsible } from "../Collapsible";
import { Icon } from "../Icon";

const action = (value: number) => ["不操作", "睡眠", "休眠", "计划设置关机，原厂事件不另行请求"][value] ?? "未知";
export function BatteryLowActionFacts({ profile }: { profile: BatteryLowActionProfile }) {
  const current = profile.current;
  return <section className="ccc-battery-action-facts" aria-label="原厂低电量系统动作事实与预览">
    <strong>原厂低电量系统动作</strong>
    <div className="ccc-battery-action-columns">
      <Surface variant="inset" className="ccc-battery-action-part">
        <strong>当前电池计划</strong>
        {current ? <>
          <p>低电量：{action(current.low)}</p>
          <p>严重电量：{action(current.critical)}</p>
          <p className="ccc-control-caption">{profile.onAc === null ? "供电状态未知" : profile.onAc ? "当前插电" : "当前电池供电"} · {profile.percent === null ? "电量未知" : `电量 ${profile.percent}%`}</p>
        </> : <p className="ccc-control-caption">当前电池计划动作不可读取。</p>}
        {profile.error && <p className="ccc-control-error">{profile.error}</p>}
        {profile.powerError && <p className="ccc-control-error">{profile.powerError}</p>}
      </Surface>
      <Surface variant="inset" className="ccc-battery-action-part">
        <strong>事件执行与恢复</strong>
        <span className="ccc-rule-heading"><Icon name="battery" />仅电池供电时触发</span>
        <span className="ccc-rule-heading"><Icon name="sleep" />按当前计划睡眠 / 休眠</span>
        <span className="ccc-chip is-warn">系统动作不自动撤销</span>
        <Collapsible summary="事件与恢复说明">
        <p className="ccc-control-caption">原厂事件等待 1 秒后，在电池供电时按上述计划请求睡眠或休眠；接电或供电未知时不请求。</p>
        <p className="ccc-control-caption">睡眠和休眠不会自动撤销；可停止监听或恢复初始热键策略。Windows 电源计划仍可能独立执行其设置。</p>
        </Collapsible>
      </Surface>
    </div>
  </section>;
}
