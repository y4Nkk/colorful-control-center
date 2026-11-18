import { Icon } from "../Icon";
import { ProfileModeAuthorization } from "../profiles/ProfileModeAuthorization";
import type { PowerPolicyRecord, ProfileApplication } from "../../api/client";
import { Button } from "../Button";
import { Switch } from "../Switch";
import { ConfigurationAction } from "../ConfigurationAction";
import { DetailsDialog } from "../DetailsDialog";
import { ConfirmButton } from "../ConfirmButton";
import { Select } from "../Select";
import { Surface } from "../Surface";
import { ProfilePreview } from "../profiles/ProfilePreview";
import { POWER_RULES, usePowerProfilePolicy } from "./usePowerProfilePolicy";
import { useAutomaticProfileReport } from "./useAutomaticProfileReport";
import { PowerSourceVisual } from "./PowerSourceVisual";

export function PowerProfilePolicy({ active, revision }: { active: boolean; revision: number }) {
  const p = usePowerProfilePolicy(active, revision), latest = useAutomaticProfileReport(active);
  const { view, draft } = p;
  const status = view?.active ? "已启用" : view?.paused ? "已暂停" : view ? "未启用" : "未读取";
  const issues = Array.from(new Set([p.error, view?.configurationError, view?.listenerError, view?.lastIssue, latest.error].filter(Boolean)));
  const blocked = !view?.record || !!view.configurationError || (draft.enabled && (!view.route || !!view.listenerError));
  const changed = !!view?.record && (p.edited || view.paused && draft.enabled);
  return <Surface as="section" variant="group" className="ccc-panel" aria-label="供电与唤醒档案">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="profiles" />供电与唤醒</h2>
      <div className="ccc-module-tools"><DetailsDialog title="供电档案状态与记录" label="详情" wide>
        <div className="ccc-detail-stack">
          <SavedRules record={view?.record ?? null} />
          {latest.report && <AutomaticReport report={latest.report} />}
          {issues.map(issue => <p key={issue} className="ccc-control-error">{issue}</p>)}
          {view?.route?.warnings.map(warning => <p key={warning} className="ccc-control-caption">{warning}</p>)}
          <Button icon="refresh" variant="secondary" disabled={p.busy} onClick={() => void p.reread()}>重新读取并暂停</Button>
        </div>
      </DetailsDialog></div>
    </div>
    <PowerSourceVisual source={view?.source === "ac" || view?.source === "battery" || view?.source === "ups" ? view.source : "unknown"}
      label={status} />
    {view?.pending && <p className="ccc-control-caption" role="status">等待供电稳定或当前档案结束</p>}
    {issues.length > 0 && <p className="ccc-control-caption">部分规则需要处理 · 查看详情</p>}
    <div className="ccc-keyboard-setting"><strong>自动应用档案</strong>
      <Switch label="启用自动档案" checked={draft.enabled} disabled={p.busy || !view?.record}
        onChange={enabled => p.setDraft({ ...draft, enabled })} /></div>
    <PolicyFields model={p} />
    {view?.active && <Button className="ccc-inline-tool" icon="pause" variant="secondary" size="compact" disabled={p.busy}
      onClick={() => void p.pause()}>暂停自动应用</Button>}
    {view?.resetRevision && <ConfirmButton icon="delete" key={view.resetRevision} label="清除不可用规则" confirmLabel="清除规则并停用"
      busy={p.busy} onConfirm={p.reset} />}
    <ConfigurationAction label="供电与唤醒档案" revision={JSON.stringify([p.request, view?.paused])} disabled={p.busy || blocked || !p.request || !changed}
      onConfirm={p.save}>
      <p>{draft.enabled ? "按下列条件应用档案。当前设备设置会保留到下次触发。" : "保存规则并停用自动应用。"}</p>
      <PolicyTargets model={p} />
    </ConfigurationAction>
    <ConfigurationAction label="供电与唤醒档案" revision={JSON.stringify([view?.record, view?.initial])} restore
      disabled={p.busy || !view?.initial || !view.record || !!view.configurationError} onConfirm={p.restore}>
      <SavedRules record={view?.initial ?? null} />
      <p>恢复后暂停自动应用，设备现值保留。</p>
    </ConfigurationAction>
  </Surface>;
}

function PolicyFields({ model: p }: { model: ReturnType<typeof usePowerProfilePolicy> }) {
  return <div className="ccc-display-controls">{POWER_RULES.map(rule => {
    const choice = p.choices[rule.key], selection = choice.selection, name = p.draft[rule.key];
    const options = [{ value: "", label: "不应用档案" }, ...p.profiles.map(profile => ({ value: profile.name, label: profile.name }))];
    if (name && !options.some(option => option.value === name)) options.push({ value: name, label: `${name}（不可读取）` });
    const issues = [choice.error, selection?.modeError, selection?.displayError, selection?.gpuPowerError, selection?.graphicsModeError].filter(Boolean);
    return <div key={rule.key}>
      <label className="ccc-config-field">{rule.label}<Select label={rule.label} value={name} options={options}
        disabled={p.busy || !p.view?.record} onChange={value => p.setDraft({ ...p.draft, [rule.key]: value })} /></label>
      {issues.length > 0 && <DetailsDialog title={`${rule.label} · 档案不可用`}>{issues.map(error => <p key={error}>{error}</p>)}</DetailsDialog>}
    </div>;
  })}</div>;
}

function PolicyTargets({ model: p }: { model: ReturnType<typeof usePowerProfilePolicy> }) {
  return <div className="ccc-detail-stack">{POWER_RULES.map(rule => {
    const selection = p.choices[rule.key].selection;
    return <section className="ccc-review-section" key={rule.key}>
      <h3>{rule.label}</h3><p>{p.draft[rule.key] || "不应用档案"}</p>
      <ProfileModeAuthorization approval={selection?.modeAuthorization ?? null} />
      {selection && <DetailsDialog title={`查看「${selection.file.name}」参数`} wide>
        <ProfilePreview settings={selection.file.settings} modeTarget={selection.modeTarget} displayReview={selection.displayReview}
          gpuPowerReview={selection.gpuPowerReview} graphicsModeReview={selection.graphicsModeReview} />
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
