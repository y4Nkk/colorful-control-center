import { Icon } from "../Icon";
import { applyLanPolicy, restoreLanPolicy, type LanField } from "../../api/client";
import { Button } from "../Button";
import { Checkbox } from "../Checkbox";
import { Collapsible } from "../Collapsible";
import { ConfirmButton } from "../ConfirmButton";
import { FactList } from "../FactList";
import { Select } from "../Select";
import { Surface } from "../Surface";
import { LAN_FIELDS, lanString, lanValue, lanMeaning } from "./lan-values";
import { useLanPolicy } from "./useLanPolicy";
const OPTIONS = [{ value: "keep", label: "保留当前节能设置" }, { value: "on", label: "开启网卡节能" }, { value: "off", label: "关闭网卡节能" }];

export function LanPolicy({ active, revision }: { active: boolean; revision: number }) {
  const model = useLanPolicy(active, revision), { profile, draft, busy, error, request, preview, run } = model;
  if (model.loaded && !profile && !error) return null;
  const device = profile?.frame.devices.find(value => value.identity.instance === draft.deviceId);
  const initial = profile?.initial.find(value => value.identity.instance === draft.deviceId);
  const sameIdentity = !!device && !!initial && JSON.stringify(device.identity) === JSON.stringify(initial.identity);
  const currentPower = lanString(device?.values.powerSavingMode), canEditPower = !!profile?.frame.capabilities.powerSaving && ["0", "1"].includes(currentPower ?? "");
  const canWake = !!profile?.frame.capabilities.wakePatch && !!device && ["s5WakeOnLan", "shutdownWake", "wakeOnSlot"].some(field => lanString(device.values[field as LanField]) !== null);
  const devices = profile?.frame.devices.map(value => ({ value: value.identity.instance, label: value.name })) ?? [];
  for (const value of profile?.initial ?? []) if (!devices.some(option => option.value === value.identity.instance)) devices.push({ value: value.identity.instance, label: "已移除网卡 · 保留恢复记录" });
  return <Surface as="section" variant="group" className="ccc-panel ccc-lan-policy" aria-label="网络驱动策略">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="network" />网络驱动策略</h2>
      <Button icon="refresh" variant="ghost" size="compact" disabled={busy} onClick={model.reread}>重新读取网络策略</Button></div>
    {error && <p className="ccc-control-error" role="alert">{error}</p>}
    {!profile ? <p className="ccc-control-caption">{error ? "无法读取，请重试。" : "正在读取网卡配置。"}</p> : <>
      <Select label="网络策略设备" value={draft.deviceId} options={devices} disabled={busy || !devices.length}
        onChange={deviceId => model.setDraft({ deviceId, powerSaving: null, applyWakePatch: false })} />
      <div className="ccc-lan-columns">
        <Surface variant="inset" className="ccc-lan-section"><strong>当前驱动设置</strong>
          {device ? <FactList compact layout="tiles" facts={(["powerSavingMode", "pnpCapabilities", "wakeOnLink"] as const).map(field => ({ label: LAN_FIELDS[field], value: <span className="ccc-state-symbol"><Icon name={field === "powerSavingMode" ? "sleep" : "network"} />{lanMeaning(field, device.values[field])}</span> }))} /> : <p className="ccc-control-caption">当前设备已移除，初始记录保留。</p>}
          {initial && <><strong>初始恢复目标</strong><FactList compact facts={Object.entries(initial.fields).map(([field, value]) => ({ label: LAN_FIELDS[field as LanField], value: lanMeaning(field as LanField, value) }))} /></>}
          {initial && !sameIdentity && <p className="ccc-control-caption">设备或驱动身份已变化，不能向新驱动写入旧记录。</p>}
          {profile.lastError && <p className="ccc-control-error">上次操作：{profile.lastError}</p>}
          <div className="ccc-actions"><ConfirmButton icon="undo" key={`restore-${profile.revision}-${draft.deviceId}`} variant="secondary" label="恢复初始网络策略" confirmLabel="确认恢复这张网卡的初始字段？"
            busy={busy} disabled={!sameIdentity || !!profile.blockedReason}
            onConfirm={() => run(() => restoreLanPolicy(profile.revision, draft.deviceId), "网卡初始字段已回读", true)} /></div>
        </Surface>
        <Surface variant="inset" className="ccc-lan-section"><strong>待应用策略</strong>
          <Select label="网卡节能目标" value={draft.powerSaving === null ? "keep" : draft.powerSaving ? "on" : "off"} options={OPTIONS} disabled={busy || !canEditPower}
            onChange={value => model.setDraft(previous => ({ ...previous, powerSaving: value === "keep" ? null : value === "on" }))} />
          {!canEditPower && <p className="ccc-control-caption">节能字段未提供、未知或未声明支持，保留现值。</p>}
          <Checkbox label="应用原厂关机唤醒驱动补丁" checked={draft.applyWakePatch} disabled={busy || !canWake}
            onChange={event => model.setDraft(previous => ({ ...previous, applyWakePatch: event.target.checked }))} />
          {!canWake && <p className="ccc-control-caption">所选设备不符合已确认的原厂唤醒补丁条件。</p>}
          {profile.frame.recommendation !== null && <Button variant="ghost" disabled={busy || !canEditPower}
            onClick={() => model.setDraft(previous => ({ ...previous, powerSaving: profile.frame.recommendation }))}>将原厂节能建议填入草稿</Button>}
          {preview.changes.length > 0 && <div aria-label="网络策略变更预览"><FactList compact facts={preview.changes.map(change => ({ label: LAN_FIELDS[change.field], value: `${lanMeaning(change.field, change.before)} → ${lanMeaning(change.field, change.target)}` }))} /></div>}
          {profile.blockedReason && <p className="ccc-control-caption">{profile.blockedReason}</p>}
          <div className="ccc-actions"><ConfirmButton key={JSON.stringify(request)} label="应用网络策略" confirmLabel="确认这些字段并请求系统授权？" busy={busy}
            disabled={!request || !preview.ready || !preview.changes.length || !!profile.blockedReason || !!initial && !sameIdentity}
            onConfirm={() => run(() => applyLanPolicy(request!), "网卡驱动配置已回读")} /></div>
        </Surface>
      </div>
      <Collapsible summary="网络策略说明与诊断"><p>确认后由系统请求管理员授权，只修改所选网卡，不重启网卡。驱动配置回读不代表物理网卡已采用策略或关机唤醒已生效。初始恢复只修改本应用尝试过的字段，原先缺失的值会删除。</p>
        {device && <><FactList compact facts={[{ label: "设备实例", value: device.identity.instance }, { label: "驱动绑定", value: device.identity.driver }]} />
          <FactList compact facts={(Object.keys(LAN_FIELDS) as LanField[]).map(field => ({ label: LAN_FIELDS[field],
            value: `当前 ${lanValue(device.values[field])}；初始 ${lanValue(initial?.fields[field])}` }))} /></>}</Collapsible>
    </>}
  </Surface>;
}
