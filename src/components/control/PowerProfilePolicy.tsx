import { FactList } from "../FactList";
import { Loading } from "../Loading";
import { ProfileModeAuthorization, ProfileModeExecutionNotes } from "../profiles/ProfileModeAuthorization";
import type { PowerPolicyRecord } from "../../api/client";
import { Button } from "../Button";
import { Switch } from "../Switch";
import { ConfigurationAction } from "../ConfigurationAction";
import { DetailsDialog } from "../DetailsDialog";
import { OemConflictAction } from "../OemConflictAction";
import { ConfirmButton } from "../ConfirmButton";
import { Select } from "../Select";
import { ProfilePreview } from "../profiles/ProfilePreview";
import { POWER_RULES, usePowerProfilePolicy } from "./usePowerProfilePolicy";

export function PowerProfilePolicy({ active, revision }: { active: boolean; revision: number }) {
  const p = usePowerProfilePolicy(active, revision);
  const { view } = p;
  const reading = !view && !p.error;
  const status = view?.paused && view.record?.enabled ? "已暂停" : null;
  const currentIssues = [p.error, view?.configurationError, view?.listenerError];
  const issues = Array.from(new Set([...currentIssues, view?.lastIssue].filter(Boolean)));
  const unavailableRules = POWER_RULES.map(rule => {
    const { error, selection } = p.choices[rule.key];
    return { ...rule, issues: [error, selection?.modeError, selection?.displayError, selection?.gpuPowerError, selection?.graphicsModeError].filter((issue): issue is string => !!issue) };
  }).filter(rule => rule.issues.length > 0);
  const savedRules = !!view?.record && POWER_RULES.some(rule => view.record?.[rule.key]);
  const canEnable = savedRules && !!view?.route && !view.listenerError;
  const blocked = !view?.record || !!view.configurationError;
  return <section aria-label="供电与唤醒档案">
    <div className="ccc-power-setting">
      <div>
        <strong>启用自动档案</strong>
        {reading ? <Loading label="正在读取…" /> : status && <p className="ccc-control-caption" role="status">{status}</p>}
      </div>
      <div className="ccc-power-setting-tools">
        <Switch label="启用自动档案" checked={!!view?.record?.enabled}
          disabled={p.busy || blocked || !view?.record?.enabled && (!canEnable || !!p.staleRules.length)} onChange={enabled => void p.toggle(enabled)} />
      </div>
    </div>
    {view?.pending && <p className="ccc-control-caption" role="status">等待供电稳定或当前档案结束</p>}
    {issues.map(issue => <p key={issue} className="ccc-control-error" role="alert">{issue}</p>)}
    {issues.length > 0 && <div className="ccc-actions">
      <Button icon="refresh" variant="secondary" size="compact" disabled={p.busy} onClick={() => void p.reread()}>重新读取并暂停</Button>
    </div>}
    <OemConflictAction issues={[...currentIssues, ...unavailableRules.flatMap(rule => rule.issues)]}
      disabled={p.busy} onClosed={p.refreshAfterExit} />
    {view?.record && !savedRules && <p className="ccc-control-caption">保存一条规则后可开启自动档案。</p>}
    <PolicyFields model={p} unavailable={unavailableRules} />
    {(view?.active || view?.paused && view.record?.enabled || view?.resetRevision) && <div className="ccc-power-setting-actions">
      {view.active &&
        <Button icon="pause" variant="secondary" size="compact" disabled={p.busy}
          onClick={() => void p.pause()}>暂停自动应用</Button>}
      {view.paused && view.record?.enabled && <Button icon="play" variant="secondary" size="compact" disabled={p.busy || blocked || !canEnable || !!p.staleRules.length}
        onClick={() => void p.toggle(true)}>继续自动应用</Button>}
      {view.resetRevision && <ConfirmButton icon="delete" key={view.resetRevision} label="清除不可用规则" confirmLabel="清除规则并停用"
        busy={p.busy} onConfirm={p.reset} />}
    </div>}
    <ConfigurationAction label="供电与唤醒档案" applyLabel={p.request?.enabled && view?.paused ? "保存规则并继续自动应用" : "保存自动档案规则"}
      revision={JSON.stringify([p.request, view?.paused])} disabled={p.busy || blocked || !p.request || (!p.edited && !p.staleRules.length) || !!p.request?.enabled && !canEnable}
      onConfirm={p.save}>
      {p.request?.enabled && <p>新规则从下次触发时生效。</p>}
      <PolicyTargets model={p} />
    </ConfigurationAction>
    <ConfigurationAction label="供电与唤醒档案" revision={JSON.stringify([view?.record, view?.initial])} restore
      disabled={p.busy || !view?.initial || !view.record || !!view.configurationError} onConfirm={p.restore}>
      <SavedRules record={view?.initial ?? null} />
      <p>恢复后暂停自动应用，设备现值保留。</p>
    </ConfigurationAction>
  </section>;
}

function PolicyFields({ model: p, unavailable }: {
  model: ReturnType<typeof usePowerProfilePolicy>; unavailable: { key: (typeof POWER_RULES)[number]["key"]; issues: string[] }[];
}) {
  return <div className="ccc-power-subsettings">{POWER_RULES.map(rule => {
    const name = p.draft[rule.key];
    const options = [{ value: "", label: "不应用档案" }, ...p.profiles.map(profile => ({ value: profile.name, label: profile.name }))];
    if (name && !options.some(option => option.value === name)) options.push({ value: name, label: `${name}（不可读取）` });
    const blocked = unavailable.find(item => item.key === rule.key);
    return <div key={rule.key}>
      <label className="ccc-power-setting"><span>{rule.label}</span><span className="ccc-power-setting-picker">
        <Select label={rule.label} size="compact" value={name} options={options}
          disabled={p.busy || !p.view?.record} onChange={value => p.setDraft({ ...p.draft, [rule.key]: value })} />
      </span></label>
      {p.staleRules.some(stale => stale.key === rule.key) && <p className="ccc-control-caption is-warn">
        {p.profiles.some(profile => profile.name === name) ? "档案有更新，请核对并保存规则" : "档案已不可用，请重新选择"}
      </p>}
      {blocked && <p className="ccc-control-caption is-warn">档案不可用</p>}
      {blocked?.issues.map((issue, index) => <p key={index} className="ccc-control-caption">{issue}</p>)}
    </div>;
  })}</div>;
}

function PolicyTargets({ model: p }: { model: ReturnType<typeof usePowerProfilePolicy> }) {
  return <div className="ccc-detail-stack">{POWER_RULES.map(rule => {
    const selection = p.choices[rule.key].selection;
    return <section className="ccc-review-section" key={rule.key} aria-label={rule.label}>
      <h3>{rule.label}</h3><p>{p.draft[rule.key] || "不应用档案"}</p>
      <ProfileModeAuthorization approval={selection?.modeAuthorization ?? null} />
      {selection && <DetailsDialog title={`查看「${selection.file.name}」参数`} label="查看档案参数" icon={null} wide>
        <ProfilePreview settings={selection.file.settings} modeTarget={selection.modeTarget} displayReview={selection.displayReview}
          gpuPowerReview={selection.gpuPowerReview} graphicsModeReview={selection.graphicsModeReview} />
        <ProfileModeExecutionNotes approval={selection.modeAuthorization} />
      </DetailsDialog>}
    </section>;
  })}</div>;
}

function SavedRules({ record }: { record: PowerPolicyRecord | null }) {
  return <FactList compact facts={POWER_RULES.map(rule => ({
    label: rule.label,
    icon: rule.key === "ac" ? "plug" : rule.key === "battery" ? "battery" : "sleep",
    value: record?.[rule.key]?.profile ?? "不应用档案",
  }))} />;
}
