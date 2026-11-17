import { Icon } from "../Icon";
import { applyUsbPolicy, restoreUsbPolicy, type UsbPlan, type UsbTargets } from "../../api/client";
import { Button } from "../Button";
import { Collapsible } from "../Collapsible";
import { ConfirmButton } from "../ConfirmButton";
import { FactList } from "../FactList";
import { Select } from "../Select";
import { Surface } from "../Surface";
import { useUsbPowerPolicy } from "./useUsbPowerPolicy";

import { USB_PLAN_NAMES as NAMES } from "./platform-values";
const text = (value: boolean | null) => value === null ? "未知" : value ? "开启" : "关闭";
const plans = (values: UsbPlan[], initial = false) => values.map(plan => ({ label: NAMES[plan.id],
  value: <div className="ccc-rule-facts">{(["ac", "dc"] as const).map(source => <span className="ccc-state-symbol" data-state={plan[source] === null ? "unknown" : plan[source] ? "on" : "off"} key={source}>
    <Icon name={source === "ac" ? "plug" : "battery"} size={16} />{source === "ac" ? "插电" : "电池"} · {initial && plan[source] === null ? "本次未修改" : text(plan[source])}
  </span>)}</div> }));
function changes(plans: UsbPlan[], draft: UsbTargets) {
  return plans.flatMap(plan => (["ac", "dc"] as const).flatMap(field => draft[field] === null || draft[field] === plan[field] ? [] : [{
    label: `${NAMES[plan.id]} · ${field === "ac" ? "插电" : "电池"}`, value: `${text(plan[field])} → ${text(draft[field])}`,
  }]));
}
const OPTIONS = [{ value: "keep", label: "保留各计划当前值" }, { value: "on", label: "开启选择性暂停" }, { value: "off", label: "关闭选择性暂停" }];
const selected = (value: boolean | null) => value === null ? "keep" : value ? "on" : "off";

export function UsbPowerPolicy({ active, revision }: { active: boolean; revision: number }) {
  const model = useUsbPowerPolicy(active, revision), { profile, draft, busy, error, run } = model;
  if (model.loaded && !profile && !error) return null;
  const preview = profile ? changes(profile.plans, draft) : [];
  const unknown = !!profile?.plans.some(plan => (draft.ac !== null && plan.ac === null) || (draft.dc !== null && plan.dc === null));
  const canApply = !!profile && profile.supported && !profile.blockedReason && preview.length > 0 && !unknown;
  const update = (field: keyof UsbTargets, value: string) => model.setDraft(previous => ({ ...previous, [field]: value === "keep" ? null : value === "on" }));
  return <Surface as="section" variant="group" className="ccc-panel ccc-usb-policy" aria-label="USB选择性暂停">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="usb" />USB 选择性暂停</h2>
      <Button icon="refresh" variant="ghost" size="compact" disabled={busy} onClick={model.reread}>重新读取USB策略</Button></div>
    {error && <p className="ccc-control-error" role="alert">{error}</p>}
    {!profile ? <p className="ccc-control-caption">{error ? "无法读取，请重试。" : "正在读取系统计划。"}</p> : <div className="ccc-usb-columns">
      <Surface variant="inset" className="ccc-usb-section">
        <strong>当前系统设置</strong>
        <span className="ccc-chip">活动计划 · {profile.activePlan ? NAMES[profile.activePlan] : "自定义计划"}</span>
        <p className="ccc-control-caption">原厂建议：{profile.recommendation === null ? "未要求干预" : text(profile.recommendation)}</p>
        {profile.plans.length > 0 && <FactList compact facts={plans(profile.plans)} />}
        {profile.missing.length > 0 && <p className="ccc-control-caption">未安装：{profile.missing.map(id => NAMES[id]).join("、")}。不创建或修改缺失计划。</p>}
        {profile.initial.length > 0 && <><strong>初始恢复目标</strong><FactList compact facts={plans(profile.initial, true)} /></>}
        {profile.restoreBlockedReason && <p className="ccc-control-caption">{profile.restoreBlockedReason}</p>}
        <div className="ccc-actions"><ConfirmButton icon="undo" key={`restore-${profile.revision}`} variant="secondary" label="恢复初始USB策略" confirmLabel="确认恢复本次修改的USB字段？"
          busy={busy} disabled={!profile.initial.length || !!profile.restoreBlockedReason}
          onConfirm={() => run(() => restoreUsbPolicy(profile.revision), "USB初始字段已恢复", true)} /></div>
      </Surface>
      <Surface variant="inset" className="ccc-usb-section">
        <strong>待应用设置</strong>
        <label className="ccc-usb-field">插电时<Select label="插电USB策略" value={selected(draft.ac)} options={OPTIONS} disabled={busy} onChange={value => update("ac", value)} /></label>
        <label className="ccc-usb-field">电池供电时<Select label="电池USB策略" value={selected(draft.dc)} options={OPTIONS} disabled={busy} onChange={value => update("dc", value)} /></label>
        <Button variant="ghost" disabled={busy || profile.recommendation === null || !profile.supported}
          onClick={() => model.setDraft({ ac: profile.recommendation, dc: profile.recommendation })}>将原厂建议填入草稿</Button>
        {preview.length > 0 && <div aria-label="USB变更预览"><FactList compact facts={preview} /></div>}
        {unknown && <p className="ccc-control-caption">所选字段含未知现值，无法保证恢复，请保留该字段或重新读取。</p>}
        {profile.blockedReason && <p className="ccc-control-caption">{profile.blockedReason}</p>}
        <div className="ccc-actions"><ConfirmButton key={`${profile.revision}-${JSON.stringify(draft)}`} label="应用USB策略" confirmLabel="确认修改这些计划的USB策略？"
          busy={busy} disabled={!canApply} onConfirm={() => run(() => applyUsbPolicy(profile.revision, draft), "USB策略已回读，当前计划已重载")} /></div>
      </Surface>
    </div>}
    {profile && <Collapsible summary="USB策略说明与诊断"><p>只修改USB选择性暂停设置，不代表USB充电开关或实际外设已进入休眠。选择“保留”不修改对应字段，应用后保持当前计划。</p>
      {profile.warnings.map(warning => <p key={warning}>{warning}</p>)}</Collapsible>}
  </Surface>;
}
