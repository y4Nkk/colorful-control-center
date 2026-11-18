import { useAutostart } from "../../app/useAutostart";
import type { AutostartLevel } from "../../api/client";
import { Button } from "../Button";
import { ConfigurationAction } from "../ConfigurationAction";
import { Select } from "../Select";
import { SettingRow } from "./PreferenceCards";
import { DetailsDialog } from "../DetailsDialog";

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
    <SettingRow label="登录自启">
      <Select label="登录自启方式" value={model.level} options={OPTIONS} disabled={locked}
        onChange={value => model.setLevel(value as AutostartLevel)} />
    </SettingRow>
    <DetailsDialog title="自启权限说明">
      <p>当前进程 · {model.snapshot ? model.snapshot.elevated ? "管理员权限" : "普通权限" : "未知"}</p>
      <p>管理员登录启动会在登录后沿用管理员权限。更改自启需要 Windows 授权。</p>
    </DetailsDialog>
    <ConfigurationAction label="登录自启" revision={JSON.stringify([model.snapshot, model.level])}
      disabled={locked || !changed} onConfirm={() => model.apply()}>
      <p>{OPTIONS.find(option => option.value === model.snapshot?.level)?.label ?? "未知"} → {label}</p>
      <p>确认后请求 Windows 授权。{model.snapshot?.legacy && "旧登录项将被替换。"}</p>
    </ConfigurationAction>
    <ConfigurationAction label="登录自启" revision={JSON.stringify(model.snapshot)} restore
      disabled={locked || !model.snapshot?.canRestore} onConfirm={() => model.apply(true)}>
      <p>恢复首次修改前的登录自启设置，需要 Windows 授权。</p>
    </ConfigurationAction>
    {model.error && <div className="ccc-settings-feedback"><p className="ccc-settings-error" role="alert">{model.error}</p>
      <Button icon="refresh" disabled={model.pending !== null} onClick={() => void model.reload()}>重新读取启动状态</Button></div>}
  </section>;
}
