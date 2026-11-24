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
  const level = model.snapshot?.level;
  return <section className="ccc-autostart-setting" aria-label="登录自启" aria-busy={model.pending !== null}>
    <SettingRow label="登录自启">
      <Select label="登录自启方式" value={level ?? ""} placeholder="状态未知" options={OPTIONS} disabled={locked}
        onChange={value => void model.apply(value as AutostartLevel)} />
    </SettingRow>
    <DetailsDialog title="自启权限说明">
      <p>本软件当前的权限 · {model.snapshot ? model.snapshot.elevated ? "管理员权限" : "普通权限" : "未知"}</p>
      <p>选择「管理员登录启动」后，软件在登录 Windows 后会以管理员权限运行。修改登录自启方式时，需要通过 Windows 的授权确认。</p>
    </DetailsDialog>
    {level && model.snapshot?.legacy && <Button size="compact" variant="secondary" disabled={locked}
      onClick={() => void model.apply(level)}>转换旧登录项</Button>}
    <ConfigurationAction label="登录自启" revision={JSON.stringify(model.snapshot)} restore
      disabled={locked || !model.snapshot?.canRestore} onConfirm={() => model.apply(level!, true)}>
      <p>恢复首次修改前的登录自启设置，需要 Windows 授权。</p>
    </ConfigurationAction>
    {model.error && <div className="ccc-settings-feedback"><p className="ccc-settings-error" role="alert">{model.error}</p>
      <Button icon="refresh" disabled={model.pending !== null} onClick={() => void model.reload()}>重新读取启动状态</Button></div>}
  </section>;
}
