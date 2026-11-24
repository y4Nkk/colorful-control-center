import { Icon } from "../Icon";
import { applyFanDustSchedule, restoreFanDustSchedule, startFanDust, type DustSchedule } from "../../api/client";
import { Button } from "../Button";
import { ConfirmButton } from "../ConfirmButton";
import { ConfigurationAction } from "../ConfigurationAction";
import { Surface } from "../Surface";
import { DetailsDialog } from "../DetailsDialog";
import { DustScheduleEditor, formatDustSchedule } from "./DustScheduleEditor";
import { useFanDust } from "./useFanDust";

const START_MESSAGES = { sent: "除尘请求已发送，当前尚未报告运行。", uncertain: "上次请求结果未确认，请重新读取状态后决定是否重试。",
  running: "固件正在执行除尘，请等待结束。", completed: "已观察到本次除尘结束。" };
export function FanDust({ active, revision, onSettled }: { active: boolean; revision: number; onSettled: () => void }) {
  const { profile, loaded, busy, readValid, error, run, reread } = useFanDust(active, revision, onSettled);
  if (loaded && !profile && !error) return null;
  const current = profile?.current ?? { weekdays: 0, hour: 0, onResume: false };
  const blocked = busy || !readValid;
  const changeSchedule = (schedule: DustSchedule) => {
    if (!profile || profile.scheduleBlockedReason || blocked ||
      profile.softwareActive && (schedule.weekdays !== 0 || schedule.onResume) ||
      profile.current && JSON.stringify(schedule) === JSON.stringify(profile.current)) return;
    void run(() => applyFanDustSchedule(profile.revision, schedule), "除尘计划已保存并回读");
  };
  const restoreConflict = !!profile?.softwareActive && !!profile.initial &&
    (!profile.initial.schedule || profile.initial.schedule.weekdays !== 0 || profile.initial.schedule.onResume);
  return <Surface as="section" variant="group" className="ccc-panel ccc-dust-panel" aria-label="风扇除尘" aria-busy={busy || undefined}>
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="fan" />风扇除尘</h2>
      <Button icon="refresh" variant="ghost" size="compact" disabled={busy} onClick={reread}>重新读取除尘状态</Button></div>
    {error && <p className="ccc-control-error" role="alert">{error}</p>}
    {!readValid && profile && <p className="ccc-control-caption">显示上次读取的除尘状态。</p>}
    {!profile ? <p className="ccc-control-caption">{error ? "无法读取，请重试。" : "正在读取除尘状态。"}</p> : <div className="ccc-dust-columns">
      <Surface as="div" variant="inset" className="ccc-dust-section">
        <div className="ccc-dust-visual" role="img" aria-label={profile.running ? "正在除尘" : "除尘风扇"}>
          <svg viewBox="0 0 120 110" aria-hidden="true" data-dust-running={profile.running}>
            <circle cx="54" cy="55" r="40" fill="var(--ccc-content-muted)" stroke="var(--ccc-hairline)" />
            {[0, 90, 180, 270].map(angle => <path key={angle} d="M54 52C33 28 58 14 68 25c10 11-4 24-12 29Z" transform={`rotate(${angle} 54 55)`} fill="currentColor" opacity={profile.running ? .7 : .15} />)}
            <circle cx="54" cy="55" r="8" fill="var(--ccc-content)" stroke="var(--ccc-hairline)" />
            <path d="M99 27h13m-11 28h15m-17 28h13" stroke="currentColor" strokeWidth="3" strokeLinecap="round" opacity={profile.running ? .7 : .15} />
          </svg>{profile.running && <strong>正在除尘</strong>}
        </div>
        {profile.running && <span className="ccc-control-caption is-warn">风扇 / 电源模式暂锁定</span>}
        {profile.lastStart && <p className="ccc-control-caption" role="status">{START_MESSAGES[profile.lastStart]}</p>}
        {profile.startBlockedReason && <p className="ccc-control-caption">{profile.startBlockedReason}</p>}
        <ConfirmButton key={`start-${profile.revision}`} label="立即除尘" confirmLabel="确认开始一次除尘？" busy={busy}
          disabled={!readValid || !!profile.startBlockedReason} onConfirm={() => run(() => startFanDust(profile.revision), "除尘请求已发送")} />
        {!profile.current && <p className="ccc-control-caption">原厂未保存计划</p>}
      </Surface>
      <Surface as="div" variant="inset" className="ccc-dust-section">
        <strong>除尘计划</strong>
        <DustScheduleEditor schedule={current} disabled={!readValid || !!profile.scheduleBlockedReason} busy={busy}
          softwareActive={profile.softwareActive} onChange={changeSchedule} />
        {!profile.current && <Button variant="secondary" size="compact" disabled={blocked || !!profile.scheduleBlockedReason}
          onClick={() => changeSchedule(current)}>关闭定时与唤醒除尘</Button>}
        {profile.softwareActive && <p className="ccc-control-caption">请先退出软件风扇曲线，再启用或编辑除尘计划。</p>}
        {profile.scheduleBlockedReason && <p className="ccc-control-caption">{profile.scheduleBlockedReason}</p>}
        <ConfigurationAction revision={profile.revision} label="除尘计划" restore
          disabled={!profile.initial || !!profile.scheduleBlockedReason || restoreConflict} blocked={blocked}
          onConfirm={() => run(() => restoreFanDustSchedule(profile.revision), "初始除尘计划记录已恢复")}>
          <p>{profile.initial?.schedule ? formatDustSchedule(profile.initial.schedule) : "初始计划未报告。恢复会关闭本次设置的除尘触发。"}</p>
        </ConfigurationAction>
        {restoreConflict && <p className="ccc-control-caption">请先退出软件风扇曲线，再恢复初始除尘计划。</p>}
      </Surface>
    </div>}
    <DetailsDialog title="除尘说明"><p>点击开始后，除尘由电脑自行完成，期间暂时不能修改风扇和电源模式。「正在除尘」以电脑报告的状态为准：发出开始指令后，要等电脑确认才会显示为进行中。</p></DetailsDialog>
  </Surface>;
}
