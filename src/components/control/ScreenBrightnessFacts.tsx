import { restoreScreenBrightness, type ScreenBrightnessProfile } from "../../api/client";
import { ConfirmButton } from "../ConfirmButton";
import { FactList } from "../FactList";
import { MetricReadout } from "./MetricReadout";

type Props = { profile: ScreenBrightnessProfile; busy: boolean; run: (action: () => Promise<void>, message: string, reset: boolean) => Promise<void> };
import { powerPlanName } from "./power-plan-values";

export function ScreenBrightnessFacts({ profile, busy, run }: Props) {
  const current = profile.current;
  return <section className="ccc-screen-brightness-facts" aria-label="屏幕亮度策略事实">
    <strong>屏幕亮度策略</strong>
    {current ? <>
      <div className="ccc-screen-brightness-current">
      <MetricReadout label="实际亮度" value={current.reading.current} maximum={100} unit="%" colorVar="var(--ccc-action)" />
      <FactList compact facts={[
        { label: "当前供电", value: current.reading.onAc ? "插电" : "电池" },
        { label: "活动计划", value: powerPlanName(current.plan) },
        { label: "计划插电亮度", value: `${current.levels[0]}%` },
        { label: "计划电池亮度", value: `${current.levels[1]}%` },
      ]} />
      </div>
      <p className="ccc-control-caption" aria-label="原厂屏幕亮度触发目标">
        原厂触发目标：{current.targets.map((value, index) => `第${index + 1}项 ${value}%`).join(" · ")}
      </p>
      {current.targets.includes(0) && <p className="ccc-control-caption">目标包含 0%，触发后屏幕可能极暗，请在确认接管前核对。</p>}
    </> : <p className="ccc-control-caption">本次屏幕亮度报告不可用。</p>}
    {profile.error && <p className="ccc-control-error">{profile.error}</p>}
    {profile.initial.map(initial => <div className="ccc-screen-brightness-initial" key={initial.plan}>
      <p className="ccc-control-caption">初始恢复 · {powerPlanName(initial.plan)}</p>
      <FactList compact facts={(["插电", "电池"] as const).map((label, index) => ({
        label, value: initial.owned[index] ? `${initial.current ? `${initial.current[index]}%` : "未知"} → ${initial.original[index]}%` : "未接管，保留当前值",
      }))} />
      {initial.error && <p className="ccc-control-error">{initial.error}</p>}
    </div>)}
    {profile.restoreError && <p className="ccc-control-error">{profile.restoreError}</p>}
    {profile.initial.length > 0 && <>
      <span className="ccc-control-caption">恢复时停止本次监听 · 保留软件策略</span>
      <ConfirmButton icon="undo" key={profile.restoreRevision ?? JSON.stringify(profile.initial)} variant="secondary"
        label="恢复初始屏幕计划亮度" confirmLabel="确认停止监听并恢复上述亮度？" busy={busy} disabled={!profile.restoreRevision}
        onConfirm={() => run(() => restoreScreenBrightness(profile.restoreRevision!), "初始屏幕计划亮度已恢复", false)} />
    </>}
  </section>;
}
