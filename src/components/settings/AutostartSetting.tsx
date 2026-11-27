import { useAutostart } from "../../app/useAutostart";
import type { AutostartLevel } from "../../api/client";
import { Button } from "../Button";
import { Select } from "../Select";
import { SettingRow } from "./SettingsSection";
import { DetailsDialog } from "../DetailsDialog";

const AUTOSTART_LABEL = "登录自启";
const OPTIONS = [
  { value: "disabled", label: "不自动启动" },
  { value: "standard", label: "普通登录启动" },
  { value: "administrator", label: "管理员登录启动" },
] as const;

export function AutostartSetting({ active, busy }: { active: boolean; busy: boolean }) {
  const model = useAutostart(active);
  const locked = busy || model.pending !== null || !model.snapshot;
  const level = model.snapshot?.level;
  const legacy = level && model.snapshot?.legacy;
  return <section className="ccc-settings-item ccc-autostart-setting" aria-label={AUTOSTART_LABEL} aria-busy={model.pending !== null}>
    <SettingRow label={AUTOSTART_LABEL}>
      <div className="ccc-settings-select ccc-autostart-field">
        <DetailsDialog title="自启权限说明">
          <p>本软件当前的权限 · {model.snapshot ? model.snapshot.elevated ? "管理员权限" : "普通权限" : "未知"}</p>
          <p>选择「管理员登录启动」后，软件在登录 Windows 后会以管理员权限运行。修改登录自启方式时，需要通过 Windows 的授权确认。</p>
        </DetailsDialog>
        <Select label="登录自启方式" value={level ?? ""} placeholder="状态未知" options={OPTIONS} disabled={locked}
          onChange={value => void model.apply(value as AutostartLevel)} />
      </div>
    </SettingRow>
    {legacy && <div className="ccc-setting-control">
      <Button size="compact" variant="secondary" disabled={locked}
        onClick={() => void model.apply(level)}>转换旧登录项</Button>
    </div>}
    {model.error && <div className="ccc-settings-feedback"><p className="ccc-settings-error" role="alert">{model.error}</p>
      <Button icon="refresh" variant="secondary" size="compact" disabled={model.pending !== null}
        onClick={() => void model.reload()}>重新读取启动状态</Button></div>}
  </section>;
}
