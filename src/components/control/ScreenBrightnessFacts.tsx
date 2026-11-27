import type { ScreenBrightnessProfile } from "../../api/client";
import { FactList } from "../FactList";
import { MetricReadout } from "./MetricReadout";

import { powerPlanName } from "./power-plan-values";

export function ScreenBrightnessFacts({ profile }: { profile: ScreenBrightnessProfile }) {
  const current = profile.current;
  return <section className="ccc-screen-brightness-facts" aria-label="屏幕亮度策略事实">
    <strong>屏幕亮度策略</strong>
    {current ? <>
      <div className="ccc-screen-brightness-current">
        <MetricReadout label="实际亮度" value={current.reading.current} maximum={100} unit="%" />
        <FactList compact layout="tiles" facts={[
          { label: "当前供电", value: current.reading.onAc ? "插电" : "电池" },
          { label: "活动计划", value: powerPlanName(current.plan) },
          { label: "计划插电亮度", value: `${current.levels[0]}%` },
          { label: "计划电池亮度", value: `${current.levels[1]}%` },
        ]} />
      </div>
      <p className="ccc-control-caption" aria-label="原厂屏幕亮度触发目标">
        触发时电脑会设置的亮度：{current.targets.map((value, index) => `第${index + 1}项 ${value}%`).join(" · ")}
      </p>
      {current.targets.includes(0) && <p className="ccc-control-caption">目标里有 0%：触发后屏幕可能变得非常暗，确认接管前请留意。</p>}
    </> : <p className="ccc-control-caption">本次屏幕亮度报告不可用。</p>}
    {profile.error && <p className="ccc-control-error">{profile.error}</p>}
    <ScreenBrightnessRestorePreview profile={profile} />
  </section>;
}

export function ScreenBrightnessRestorePreview({ profile }: { profile: ScreenBrightnessProfile }) {
  return <>
    {profile.initial.map(initial => <div className="ccc-screen-brightness-initial" key={initial.plan}>
      <p className="ccc-control-caption">恢复接管前的值 · {powerPlanName(initial.plan)}</p>
      <FactList compact facts={(["插电", "电池"] as const).map((label, index) => ({
        label, value: initial.owned[index] ? `${initial.current ? `${initial.current[index]}%` : "未知"} → ${initial.original[index]}%` : "本软件未改动，保留当前值",
      }))} />
      {initial.error && <p className="ccc-control-error">{initial.error}</p>}
    </div>)}
    {profile.restoreError && <p className="ccc-control-error">{profile.restoreError}</p>}
  </>;
}
