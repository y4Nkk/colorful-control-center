import { Icon } from "../Icon";
import { ProfileModeAuthorization } from "../profiles/ProfileModeAuthorization";
import type { PowerPolicyRecord, ProfileApplication } from "../../api/client";
import { Button } from "../Button";
import { Checkbox } from "../Checkbox";
import { Collapsible } from "../Collapsible";
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
  const status = view?.active ? "自动应用已启用" : view?.paused ? "自动应用已暂停" : "自动应用未启用";
  const issues = Array.from(new Set([p.error, view?.configurationError, view?.listenerError, view?.lastIssue, latest.error].filter(Boolean)));
  const blocked = !view?.record || Boolean(view.configurationError) || (draft.enabled && (!view.route || Boolean(view.listenerError)));
  return <Surface as="section" variant="group" className="ccc-panel ccc-power-policy" aria-label="供电与唤醒档案">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="plug" />供电与唤醒</h2><span className="ccc-chip">{status}</span></div>
    <div className="ccc-power-policy-columns">
      <div className="ccc-power-policy-current">
        <Surface variant="inset" className="ccc-power-policy-summary">
          <h3>当前状态</h3><PowerSourceVisual source={view?.source === "ac" || view?.source === "battery" || view?.source === "ups" ? view.source : "unknown"} />
          {view?.sleeping && <span className="ccc-control-caption">设备处于睡眠状态，等待恢复</span>}
          {view?.pending && <span className="ccc-control-caption">等待供电稳定或当前档案结束</span>}
          <SavedRules record={view?.record ?? null} />
          <Button icon="pause" variant="secondary" disabled={p.busy || !view?.active} onClick={() => { void p.pause(); }}>暂停自动应用</Button>
        </Surface>
        {latest.report && <AutomaticReport report={latest.report} />}
        {issues.map((issue, index) => <p key={index} className="ccc-control-error" role="alert">{issue}</p>)}
        {view?.route?.warnings.map(warning => <p key={warning} className="ccc-control-caption">{warning}</p>)}
        <Button icon="refresh" variant="secondary" disabled={p.busy} onClick={() => { void p.reread(); }}>重新读取并暂停</Button>
        {view?.resetRevision && <ConfirmButton icon="delete" key={view.resetRevision} label="清除不可用规则" confirmLabel="清除旧规则并保持停用？"
          variant="secondary" busy={p.busy} onConfirm={p.reset} />}
        {view?.initial && <Collapsible summary="查看初始规则">
          <SavedRules record={view.initial} />
          <p className="ccc-control-caption">恢复后暂停自动应用，设备现值保留。</p>
          <ConfirmButton icon="undo" key={JSON.stringify([view.record, view.initial])} label="恢复初始规则" confirmLabel="恢复初始规则并暂停？"
            disabled={!view.record || Boolean(view.configurationError)} busy={p.busy} variant="secondary" onConfirm={p.restore} />
        </Collapsible>}
      </div>
      <div className="ccc-power-policy-editor">
        <Checkbox label="启用自动档案" checked={draft.enabled} disabled={p.busy || !view?.record}
          onChange={event => p.setDraft({ ...draft, enabled: event.currentTarget.checked })} />
        {POWER_RULES.map(rule => {
          const choice = p.choices[rule.key], selection = choice.selection, name = draft[rule.key];
          const options = [{ value: "", label: "不应用档案" }, ...p.profiles.map(profile => ({ value: profile.name, label: profile.name }))];
          if (name && !options.some(option => option.value === name)) options.push({ value: name, label: `${name}（不可读取）` });
          return <Surface key={rule.key} variant="inset" className="ccc-power-policy-rule">
            <div className="ccc-power-policy-select"><label htmlFor={`power-profile-${rule.key}`}>{rule.label}</label>
              <Select id={`power-profile-${rule.key}`} label={rule.label} value={name} options={options} disabled={p.busy || !view?.record}
                onChange={value => p.setDraft({ ...draft, [rule.key]: value })} /></div>
            {choice.error && <p className="ccc-control-error" role="alert">{choice.error}</p>}
            {selection && [selection.modeError, selection.displayError, selection.gpuPowerError, selection.graphicsModeError].filter(Boolean)
              .map((error, index) => <p className="ccc-control-error" role="alert" key={index}>{error}</p>)}
            <ProfileModeAuthorization approval={selection?.modeAuthorization ?? null} />
            {selection && <Collapsible summary={`查看「${selection.file.name}」参数`}>
              <ProfilePreview settings={selection.file.settings} modeTarget={selection.modeTarget} displayReview={selection.displayReview}
                gpuPowerReview={selection.gpuPowerReview} graphicsModeReview={selection.graphicsModeReview} />
            </Collapsible>}
          </Surface>;
        })}
        <span className="ccc-chip">启用时保持当前设置</span>
        <div className="ccc-actions"><ConfirmButton key={JSON.stringify([p.request, view?.paused])} label={draft.enabled ? "保存并启用" : "保存规则"}
          confirmLabel={draft.enabled ? "按以上条件自动应用档案？" : "保存规则并停用自动应用？"} disabled={blocked || !p.request} busy={p.busy} onConfirm={p.save} /></div>
        <Collapsible summary="自动档案说明"><p>档案修改后需重新确认；显示试用、系统授权和重启仍分别确认。启用时不立即应用档案。</p></Collapsible>
      </div>
    </div>
  </Surface>;
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
