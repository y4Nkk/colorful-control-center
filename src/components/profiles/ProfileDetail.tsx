import type { ProfileApplication, ProfileFile, ProfileMeta } from "../../api/client";
import type { useProfiles } from "../../app/useProfiles";
import { Button } from "../Button";
import { ConfigurationAction } from "../ConfigurationAction";
import { ConfirmButton } from "../ConfirmButton";
import { DetailsDialog } from "../DetailsDialog";
import { Icon } from "../Icon";
import { OemConflictAction } from "../OemConflictAction";
import { Surface } from "../Surface";
import { ProfileCurves } from "./ProfileCurves";
import { ProfileParameters } from "./ProfileParameters";
import { ProfileRuleImpact } from "./ProfileRuleImpact";
import { profileDifferences, type ProfileDifference } from "./profile-differences";

type Model = ReturnType<typeof useProfiles>;

export function ProfileDetail({ model, onUpdate, onOpenRules }: {
  model: Model; onUpdate: (profile: ProfileMeta) => void; onOpenRules: () => void;
}) {
  const selection = model.selection;
  const name = model.selectedName;
  const profile = model.profiles.find(item => item.name === name);
  if (!selection) return <Surface as="section" variant="group" className="ccc-panel" aria-label="档案预览">
    {model.readError && name
      ? <><p className="ccc-control-error" role="alert">{model.readError}</p>
        <Button icon="refresh" variant="secondary" onClick={() => model.select(name)}>重新读取档案</Button></>
      : <p className="ccc-control-caption" role="status">正在读取档案设置…</p>}
  </Surface>;
  const { file, review } = selection;
  const configured = model.rules.some(rule => rule.name === file.name);
  const stale = profile && model.rules.some(rule => rule.name === file.name && rule.revision !== profile.revision);
  const rows = profileDifferences(file.settings, review);
  const changes = <div className="ccc-profile-comparison">
    <h3>应用预览</h3>
    <ProfileValues rows={rows} />
    <ProfileCurves settings={file.settings} before={review.before}
      emptyMessage={rows.length ? undefined : "档案设置与已读取值一致。"} />
  </div>;
  return <Surface as="section" variant="group" className="ccc-panel ccc-profile-detail" aria-label="档案预览">
    <header className="ccc-panel-head">
      <h2 className="ccc-panel-title">{file.name}</h2>
      {profile && <ProfileMenu profile={profile} model={model} onUpdate={onUpdate} />}
    </header>
    {configured && <div className="ccc-profile-rules">
      {stale && <p className="ccc-control-caption is-warn">档案有更新，请重新确认关联规则。</p>}
      <Button icon="power" variant="ghost" size="compact" onClick={onOpenRules}>查看电源规则</Button>
    </div>}
    {review.blocked && <div className="ccc-actions">
      <p className="ccc-control-error" role="alert">{review.blocked}</p>
      <Button icon="refresh" variant="ghost" size="compact" disabled={model.busy} onClick={() => model.select(file.name)}>重新预览</Button>
    </div>}
    <OemConflictAction issues={[review.blocked]} disabled={model.busy} onClosed={() => model.select(file.name)} />
    {!review.blocked && !model.pending && !model.loading && !model.busy
      ? <ConfigurationAction label="档案设置" applyLabel="应用档案"
        revision={JSON.stringify([file, review])} onConfirm={() => void model.apply(selection)}>{changes}</ConfigurationAction>
      : changes}
    <ProfileParameters key={file.name} settings={file.settings} review={review} />
    {model.report && <ProfileReport report={model.report} />}
  </Surface>;
}

function ProfileMenu({ profile, model, onUpdate }: { profile: ProfileMeta; model: Model; onUpdate: (profile: ProfileMeta) => void }) {
  const [expanded, setExpanded] = useState(false);
  const toggle = useRef<HTMLButtonElement>(null);
  const id = useId();
  return <div className="ccc-profile-more" onBlur={event => {
    if (event.relatedTarget instanceof Node && !event.currentTarget.contains(event.relatedTarget)) setExpanded(false);
  }} onKeyDown={event => {
    if (event.key === "Escape" && expanded && (event.target as HTMLElement).closest("dialog") == null) {
      event.preventDefault(); setExpanded(false); toggle.current?.focus();
    }
  }}>
    <Button ref={toggle} variant="ghost" size="compact" icon={expanded ? "chevron-up" : "chevron-down"}
      aria-expanded={expanded} aria-controls={id} onClick={() => setExpanded(value => !value)}>更多</Button>
    <Surface variant="inset" className="ccc-profile-menu" id={id} hidden={!expanded}>
      <Button icon="edit" variant="ghost" size="compact" disabled={model.busy}
        onClick={() => onUpdate(profile)}>用当前配置更新</Button>
      <Button icon="download" variant="ghost" size="compact" disabled={model.busy}
        onClick={() => void model.download(profile.name)}>导出档案</Button>
      <ConfirmButton icon="delete" key={profile.revision} label="删除档案" confirmLabel="删除档案"
        busy={model.busy} disabled={model.rulesLoading} onConfirm={async () => { await model.remove(profile); }}>
        <p>删除「{profile.name}」？</p>
        <ProfileRuleImpact name={profile.name} rules={model.rules} operation="delete" loading={model.rulesLoading} error={model.rulesError} />
      </ConfirmButton>
    </Surface>
  </div>;
}

export function ProfileImport({ model, file }: { model: Model; file: ProfileFile }) {
  const name = file.name.trim();
  const existing = model.profiles.find(item => item.name === name);
  const confirm = async () => { await model.persist(name, file.settings, existing, "import"); };
  return <Surface as="section" variant="group" className="ccc-panel ccc-profile-detail" aria-label="档案预览">
    <header className="ccc-panel-head"><h2 className="ccc-panel-title">{name}</h2><span className="ccc-control-caption">待导入</span></header>
    {model.busy || !!existing && model.rulesLoading ? <p role="status" className="ccc-control-caption">正在核对档案…</p> :
      <ConfigurationAction label="档案设置" revision={JSON.stringify([file, existing?.revision])}
        applyLabel={existing ? "覆盖同名档案" : "导入档案"} onConfirm={confirm}>
        <ProfileValues rows={profileDifferences(file.settings, null)} targetOnly />
        <ProfileRuleImpact name={name} rules={model.rules} operation="replace" loading={false} error={existing ? model.rulesError : null} />
        <Button variant="ghost" size="compact" onClick={model.dismissImport}>取消导入</Button>
      </ConfigurationAction>}
    <ProfileParameters key={name} settings={file.settings} />
  </Surface>;
}

function ProfileValues({ rows, targetOnly }: { rows: ProfileDifference[]; targetOnly?: boolean }) {
  if (!rows.length) return null;
  return <dl className="ccc-profile-values-list">{rows.map((row, index) => <div key={`${row.label}:${index}`}>
    <dt>{row.label}</dt>
    <dd>
      {row.before !== null && <><span className="ccc-control-caption">{row.before}</span><Icon name="arrow-right" size={16} /></>}
      <strong>{row.target}</strong>
      {!targetOnly && row.before === null && <span className="ccc-control-caption">当前未读取</span>}
      {row.note && <span className="ccc-control-caption is-warn">{row.note}</span>}
    </dd>
  </div>)}</dl>;
}

function ProfileReport({ report }: { report: ProfileApplication }) {
  const failed = report.steps.filter(step => !step.ok);
  const recovery = report.steps.filter(step => step.step.startsWith("恢复"));
  const state = report.pending ? report.pending.error ? "显示恢复待处理" : "等待显示确认"
    : report.targetsApplied ? failed.length ? "已应用 · 部分收尾待处理" : "已应用"
    : recovery.some(step => !step.ok) ? "恢复待处理" : recovery.length ? "失败 · 已恢复" : "应用未完成";
  return <Surface as="section" variant="inset" className="ccc-profile-results" aria-label="最近应用记录">
    <div className="ccc-panel-head"><h3 className="ccc-panel-title"><Icon name="logs" />上次应用 · {report.name}</h3><strong>{state}</strong></div>
    {failed.map(step => <p key={step.step} className="ccc-control-error">{step.step} · {step.message}</p>)}
    {!!report.steps.length && <DetailsDialog title={`执行明细 · ${report.steps.length} 项`} icon="logs">
      <ol className="ccc-profile-steps" aria-label="执行结果">{report.steps.map((step, index) => <li key={index}>
        <span className="ccc-step-status"><Icon name={step.ok ? "check-circle" : "error"} label={step.ok ? "成功" : "失败"} size={16} />
          {step.step}</span><span>{step.message}</span>
      </li>)}</ol>
    </DetailsDialog>}
  </Surface>;
}
import { useId, useRef, useState } from "react";
