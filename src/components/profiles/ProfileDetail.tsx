import { useId, useRef, useState, type Ref } from "react";
import type { ProfileApplication, ProfileFile, ProfileMeta } from "../../api/client";
import type { useProfiles } from "../../app/useProfiles";
import { Button } from "../Button";
import { ConfigurationAction } from "../ConfigurationAction";
import { ConfirmButton } from "../ConfirmButton";
import { DetailsDialog } from "../DetailsDialog";
import { FactList } from "../FactList";
import { Icon } from "../Icon";
import { Loading } from "../Loading";
import { OemConflictAction } from "../OemConflictAction";
import { Surface } from "../Surface";
import { BuiltinRefreshControls } from "./BuiltinProfileControls";
import { ProfileCurves } from "./ProfileCurves";
import { ProfileParameters } from "./ProfileParameters";
import { ProfileRuleImpact } from "./ProfileRuleImpact";
import { profileDifferences, type ProfileDifference } from "./profile-differences";

type Model = ReturnType<typeof useProfiles>;
type DetailActions = {
  onUpdate: (profile: ProfileMeta) => void; onOpenRules: () => void; onCopy: (file: ProfileFile) => void;
};

export function ProfilePreviewPane({ id, heading, model, onDismiss, onPersist, ...actions }: DetailActions & {
  id: string; heading: Ref<HTMLHeadingElement>; model: Model; onDismiss: () => void; onPersist: Model["persist"];
}) {
  const { target, imported } = model;
  const visible = !!(target || imported);
  const builtinName = target?.kind === "builtin" ? model.builtin.catalog?.profiles.find(item => item.id === target.id)?.name : null;
  const title = imported?.name.trim() ?? (target?.kind === "saved" ? target.name : builtinName ?? "档案预览");
  return <section id={id} className="ccc-profile-preview-pane" aria-label="档案预览" hidden={!visible}>
    {visible && <>
      <header className="ccc-profile-section-head">
        <div className="ccc-profile-title-copy">
          {imported && <span className="ccc-control-caption">待导入</span>}
          <h2 ref={heading} tabIndex={-1} className="ccc-panel-title ccc-profile-preview-title">{title}</h2>
        </div>
        <Button variant="ghost" size="compact" icon="close" disabled={model.busy || !imported && model.pending}
          onClick={onDismiss}>{imported ? "取消导入" : "收起预览"}</Button>
      </header>
      <Surface variant="group" className="ccc-profile-detail">
        {imported ? <ProfileImport model={model} file={imported} onPersist={onPersist} />
          : <ProfileDetail model={model} {...actions} />}
      </Surface>
      {!imported && model.report && <ProfileReport report={model.report} />}
    </>}
  </section>;
}

function ProfileDetail({ model, onUpdate, onOpenRules, onCopy }: DetailActions & { model: Model }) {
  const selection = model.selection;
  const target = model.target;
  const builtin = target?.kind === "builtin";
  const saved = target?.kind === "saved" ? model.profiles.find(item => item.name === target.name) : undefined;
  const unavailable = model.busy || model.pending;
  const builtinControls = builtin && <BuiltinRefreshControls model={model.builtin} disabled={unavailable} />;
  if (!selection) return <>
    {builtinControls}
    {model.readError ? <div className="ccc-profile-feedback">
      <p className="ccc-control-error" role="alert">{model.readError}</p>
      <Button icon="refresh" variant="secondary" size="compact" disabled={unavailable} onClick={model.retry}>重新读取档案</Button>
    </div>
      : <Loading label="正在读取档案设置…" />}
  </>;
  const { file, review } = selection;
  const configured = saved && model.rules.some(rule => rule.name === saved.name);
  const stale = saved && model.rules.some(rule => rule.name === saved.name && rule.revision !== saved.revision);
  const ready = !review.blocked && !unavailable && !model.loading;
  const rows = profileDifferences(file.settings, review);
  const changes = <>
    <div className="ccc-profile-comparison">
      <h3>应用预览</h3>
      <ProfileValues rows={rows} />
      <ProfileCurves settings={file.settings} before={review.before}
        emptyMessage={rows.length ? undefined : "档案设置与已读取值一致。"} />
    </div>
    <ProfileParameters key={file.name} settings={file.settings} review={review} />
  </>;
  const actions = <>
    {configured && <Button icon="power" variant="ghost" size="compact" onClick={onOpenRules}>查看电源规则</Button>}
    {saved && <ProfileMenu profile={saved} model={model} onUpdate={onUpdate} />}
    {builtin && <Button icon="add" variant="secondary" size="compact" disabled={!ready}
      onClick={() => onCopy(file)}>复制为我的档案</Button>}
  </>;
  return <>
    {builtinControls}
    {stale && <p className="ccc-control-caption is-warn">档案有更新，请重新确认关联规则。</p>}
    {review.blocked && <div className="ccc-profile-feedback">
      <p className="ccc-control-error" role="alert">{review.blocked}</p>
      <Button icon="refresh" variant="ghost" size="compact" disabled={model.busy} onClick={model.retry}>重新预览</Button>
    </div>}
    <OemConflictAction issues={[review.blocked]} disabled={model.busy} onClosed={model.retry} />
    {ready
      ? <ConfigurationAction label="档案设置" applyLabel="应用档案"
        revision={JSON.stringify([file, review])} secondaryActions={actions} onConfirm={() => void model.apply(selection)}>{changes}</ConfigurationAction>
      : <>{changes}<div className="ccc-actions ccc-profile-detail-actions">{actions}</div></>}
  </>;
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

function ProfileImport({ model, file, onPersist }: { model: Model; file: ProfileFile; onPersist: Model["persist"] }) {
  const name = file.name.trim();
  const existing = model.profiles.find(item => item.name === name);
  if (model.busy || !!existing && model.rulesLoading) return <Loading label="正在核对档案…" />;
  return <ConfigurationAction label="档案设置" revision={JSON.stringify([file, existing?.revision])}
    applyLabel={existing ? "覆盖同名档案" : "导入档案"}
    onConfirm={() => onPersist(name, file.settings, existing, "import")}>
    <ProfileValues rows={profileDifferences(file.settings, null)} targetOnly />
    <ProfileRuleImpact name={name} rules={model.rules} operation="replace" loading={false} error={existing ? model.rulesError : null} />
    <ProfileParameters key={name} settings={file.settings} />
  </ConfigurationAction>;
}

function ProfileValues({ rows, targetOnly }: { rows: ProfileDifference[]; targetOnly?: boolean }) {
  if (!rows.length) return null;
  if (targetOnly) return <FactList label="档案目标值"
    facts={rows.map(({ label, note, target }) => ({ label, note, value: <strong>{target}</strong> }))} />;
  return <FactList layout="comparison" label="档案设置差异"
    facts={rows.map(({ label, note, before, target }) => ({ label, note, current: before ?? "未读取", value: target }))} />;
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
