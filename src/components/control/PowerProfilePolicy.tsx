import { Icon } from "../Icon";
import { ProfileModeAuthorization, ProfileModeExecutionNotes } from "../profiles/ProfileModeAuthorization";
import type { PowerPolicyRecord, ProfileApplication } from "../../api/client";
import { Button } from "../Button";
import { Switch } from "../Switch";
import { ConfigurationAction } from "../ConfigurationAction";
import { DetailsDialog } from "../DetailsDialog";
import { OemConflictAction } from "../OemConflictAction";
import { ConfirmButton } from "../ConfirmButton";
import { Select } from "../Select";
import { Surface } from "../Surface";
import { ProfilePreview } from "../profiles/ProfilePreview";
import { POWER_RULES, usePowerProfilePolicy } from "./usePowerProfilePolicy";
import { useAutomaticProfileReport } from "./useAutomaticProfileReport";
import { PowerSourceVisual } from "./PowerSourceVisual";

export function PowerProfilePolicy({ active, revision }: { active: boolean; revision: number }) {
  const p = usePowerProfilePolicy(active, revision), latest = useAutomaticProfileReport(active);
  const { view } = p;
  const source = view?.source === "ac" || view?.source === "battery" || view?.source === "ups" ? view.source : "unknown";
  const status = view?.paused && view.record?.enabled ? "已暂停" : !view && !p.error ? "读取中…" : null;
  const issues = Array.from(new Set([p.error, view?.configurationError, view?.listenerError, view?.lastIssue, latest.error].filter(Boolean)));
  const unavailableRules = POWER_RULES.map(rule => {
    const { error, selection } = p.choices[rule.key];
    return { ...rule, issues: [error, selection?.modeError, selection?.displayError, selection?.gpuPowerError, selection?.graphicsModeError].filter(Boolean) };
  }).filter(rule => rule.issues.length > 0);
  const savedRules = !!view?.record && POWER_RULES.some(rule => view.record?.[rule.key]);
  const canEnable = savedRules && !!view?.route && !view.listenerError;
  const blocked = !view?.record || !!view.configurationError;
  return <section aria-label="供电与唤醒档案">
    <div className="ccc-power-setting">
      <div>
        <strong>启用自动档案</strong>
        {status && <p className="ccc-control-caption" role="status">{status}</p>}
      </div>
      <div className="ccc-power-setting-tools">
        <DetailsDialog title="供电档案状态与记录" label="详情" wide>
          <div className="ccc-detail-stack">
            <PowerSourceVisual source={source} />
            <SavedRules record={view?.record ?? null} />
            {latest.report && <AutomaticReport report={latest.report} />}
            {issues.map(issue => <p key={issue} className="ccc-control-error"><Icon name="error" size={16} />{issue}</p>)}
            {view?.route?.warnings.map(warning => <p key={warning} className="ccc-control-caption">{warning}</p>)}
            {unavailableRules.map(rule => <section key={rule.key} aria-label={`${rule.label} · 档案不可用`}>
              <h3>{rule.label} · 档案不可用</h3>
              {rule.issues.map(issue => <p key={issue}>{issue}</p>)}
              <OemConflictAction issues={rule.issues} disabled={p.busy} onClosed={p.refreshAfterExit} />
            </section>)}
            <Button icon="refresh" variant="secondary" disabled={p.busy} onClick={() => void p.reread()}>重新读取并暂停</Button>
          </div>
        </DetailsDialog>
        <Switch label="启用自动档案" checked={!!view?.record?.enabled}
          disabled={p.busy || blocked || !view?.record?.enabled && (!canEnable || !!p.staleRules.length)} onChange={enabled => void p.toggle(enabled)} />
      </div>
    </div>
    {view?.pending && <p className="ccc-control-caption" role="status">等待供电稳定或当前档案结束</p>}
    {issues.length > 0 && <p className="ccc-control-caption">部分规则需要处理 · 查看详情</p>}
    <OemConflictAction issues={[...issues, ...(latest.report?.steps.filter(step => !step.ok).map(step => step.message) ?? [])]}
      disabled={p.busy} onClosed={p.refreshAfterExit} />
    {view?.record && !savedRules && <p className="ccc-control-caption">保存一条规则后可开启自动档案。</p>}
    <PolicyFields model={p} unavailable={unavailableRules.map(rule => rule.key)} />
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
  model: ReturnType<typeof usePowerProfilePolicy>; unavailable: readonly (typeof POWER_RULES)[number]["key"][];
}) {
  return <div className="ccc-power-subsettings">{POWER_RULES.map(rule => {
    const name = p.draft[rule.key];
    const options = [{ value: "", label: "不应用档案" }, ...p.profiles.map(profile => ({ value: profile.name, label: profile.name }))];
    if (name && !options.some(option => option.value === name)) options.push({ value: name, label: `${name}（不可读取）` });
    return <div key={rule.key}>
      <label className="ccc-power-setting"><span>{rule.label}</span><span className="ccc-power-setting-picker">
        <Select label={rule.label} size="compact" value={name} options={options}
          disabled={p.busy || !p.view?.record} onChange={value => p.setDraft({ ...p.draft, [rule.key]: value })} />
      </span></label>
      {p.staleRules.some(stale => stale.key === rule.key) && <p className="ccc-control-caption is-warn">
        {p.profiles.some(profile => profile.name === name) ? "档案有更新，请核对并保存规则" : "档案已不可用，请重新选择"}
      </p>}
      {unavailable.includes(rule.key) && <p className="ccc-control-caption is-warn">档案不可用 · 查看详情</p>}
    </div>;
  })}</div>;
}

function PolicyTargets({ model: p }: { model: ReturnType<typeof usePowerProfilePolicy> }) {
  return <div className="ccc-detail-stack">{POWER_RULES.map(rule => {
    const selection = p.choices[rule.key].selection;
    return <section className="ccc-review-section" key={rule.key} aria-label={rule.label}>
      <h3>{rule.label}</h3><p>{p.draft[rule.key] || "不应用档案"}</p>
      <ProfileModeAuthorization approval={selection?.modeAuthorization ?? null} />
      {selection && <DetailsDialog title={`查看「${selection.file.name}」参数`} wide>
        <ProfilePreview settings={selection.file.settings} modeTarget={selection.modeTarget} displayReview={selection.displayReview}
          gpuPowerReview={selection.gpuPowerReview} graphicsModeReview={selection.graphicsModeReview} />
        <ProfileModeExecutionNotes approval={selection.modeAuthorization} />
      </DetailsDialog>}
    </section>;
  })}</div>;
}

function SavedRules({ record }: { record: PowerPolicyRecord | null }) {
  return <dl className="ccc-power-policy-saved">{POWER_RULES.map(rule => <div key={rule.key}>
    <dt className="ccc-rule-heading"><Icon name={rule.key === "ac" ? "plug" : rule.key === "battery" ? "battery" : "sleep"} size={16} />{rule.label}</dt><dd>{record?.[rule.key]?.profile ?? "不应用档案"}</dd>
  </div>)}</dl>;
}
function AutomaticReport({ report }: { report: ProfileApplication }) {
  const source = report.origin === "ac" ? "插电" : report.origin === "battery" ? "电池" : "唤醒";
  const state = report.pending ? "等待显示确认" : report.targetsApplied ? report.steps.every(step => step.ok) ? "已应用" : "已应用，收尾待处理" : "未完成";
  return <Surface as="section" variant="inset" className="ccc-power-policy-summary" aria-label="最近自动应用">
    <h3>最近自动应用</h3><strong>{report.name}</strong><span className="ccc-control-caption">{source} · {state}</span>
    {report.steps.length > 0 && <ul className="ccc-profile-steps" aria-label="自动档案执行结果">{report.steps.map((step, index) => <li key={index}>
      <span className="ccc-step-status"><Icon name={step.ok ? "check-circle" : "error"} label={step.ok ? "成功" : "失败"} size={16} />{step.step}</span><span>{step.message}</span>
    </li>)}</ul>}
  </Surface>;
}
