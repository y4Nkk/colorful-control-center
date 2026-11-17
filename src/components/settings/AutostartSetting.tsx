import { useAutostart } from "../../app/useAutostart";
import type { AutostartLevel } from "../../api/client";
import { Button } from "../Button";
import { ConfirmButton } from "../ConfirmButton";
import { Icon } from "../Icon";
import { Select } from "../Select";
import { SettingRow } from "./PreferenceCards";
import { Collapsible } from "../Collapsible";

const OPTIONS = [
  { value: "disabled", label: "不自动启动" },
  { value: "standard", label: "普通登录启动" },
  { value: "administrator", label: "管理员登录启动" },
] as const;

export function AutostartSetting({ active, busy }: { active: boolean; busy: boolean }) {
  const model = useAutostart(active);
  const locked = busy || model.pending !== null || !model.snapshot;
  const label = OPTIONS.find(option => option.value === model.level)!.label;
  const changed = model.snapshot && (model.level !== model.snapshot.level || model.snapshot.legacy);
  return <section className="ccc-autostart-setting" aria-label="登录自启" aria-busy={model.pending !== null}>
    <SettingRow label="登录自启" detail={!model.snapshot ? "当前进程 · 权限未知" : model.snapshot.elevated ? "当前进程 · 管理员" : "当前进程 · 普通权限"}>
      <Select label="登录自启方式" value={model.level} options={OPTIONS} disabled={locked}
        onChange={value => model.setLevel(value as AutostartLevel)} />
    </SettingRow>
    <div className="ccc-autostart-preview">
      <Icon name={model.level === "disabled" ? "shutdown" : "window"} size={24} />
      <span><strong>{label}</strong>{changed && <small>需系统授权</small>}</span>
    </div>
    <Collapsible summary="自启权限说明"><p>保存和恢复时请求系统授权；管理员方式在登录后沿用管理员权限，普通方式的后台操作不临时提权。停用后下次登录不运行。</p></Collapsible>
    {model.snapshot?.legacy && <p className="ccc-settings-detail">确认后替换旧登录项，保留首次前态。</p>}
    <div className="ccc-setting-control">
      <ConfirmButton label={`保存 · ${label}`} confirmLabel={`确认 · ${label}`} disabled={locked || !changed}
        busy={model.pending === "write"} onConfirm={() => model.apply()} />
      <ConfirmButton label="恢复初始自启" confirmLabel="确认恢复初始自启" variant="secondary"
        disabled={locked || !model.snapshot?.canRestore} onConfirm={() => model.apply(true)} />
    </div>
    {model.error && <div className="ccc-settings-feedback"><p className="ccc-settings-error" role="alert">{model.error}</p>
      <Button icon="refresh" disabled={model.pending !== null} onClick={() => void model.reload()}>重新读取启动状态</Button></div>}
  </section>;
}
