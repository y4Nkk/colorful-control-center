import { applyKeyboardSettings, readKeyboardSettings, type KeyboardValues } from "../../api/client";
import { Button } from "../Button";
import { DetailsDialog } from "../DetailsDialog";
import { Icon } from "../Icon";
import { Loading } from "../Loading";
import { Surface } from "../Surface";
import { Switch } from "../Switch";
import { KeyboardFunctionArtwork } from "./KeyboardFunctionArtwork";
import { FIELDS } from "./keyboard-values";
import { useDeviceSettings } from "./useDeviceSettings";

export function KeyboardSettings({ active, revision }: { active: boolean; revision: number }) {
  const { profile, busy, error, run, reread } = useDeviceSettings(readKeyboardSettings, active, revision);
  const change = (key: keyof KeyboardValues, value: boolean) => {
    if (!profile || profile.current[key] === null || profile.current[key] === value) return;
    const request = { expectedRevision: profile.revision, values: { ...profile.current, [key]: value } };
    void run(() => applyKeyboardSettings(request), "键盘设置已应用");
  };
  return <Surface as="section" variant="group" className="ccc-panel" aria-label="键盘功能" aria-busy={busy || undefined}>
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="keyboard" />键盘功能</h2>
      <div className="ccc-module-tools">
        <DetailsDialog title="键盘功能说明" label="详情">{FIELDS.map(field => <p key={field.key}><strong>{field.title}</strong> · {field.description}</p>)}</DetailsDialog>
        <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取键盘" disabled={busy} onClick={reread} />
      </div>
    </div>
    {error && <p className="ccc-control-error" role="alert">{error}</p>}
    {profile ? <div className="ccc-setting-tiles">{FIELDS.map(({ key, title }) => <Surface as="div" variant="inset" className="ccc-keyboard-setting" key={key}>
      <KeyboardFunctionArtwork setting={key} />
      <div><strong>{title}</strong>
        {profile.current[key] === null && <p className="ccc-control-caption">此设备未提供</p>}</div>
      <Switch label={title} checked={profile.current[key] ?? false}
        disabled={busy || profile.current[key] === null} onChange={value => change(key, value)} />
    </Surface>)}</div> : error ? <p className="ccc-control-caption">请重新读取键盘功能。</p> : <Loading label="正在读取键盘功能…" />}
  </Surface>;
}
