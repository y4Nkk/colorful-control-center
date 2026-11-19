import { useEffect, useState } from "react";
import { captureProfileSettings, type CapturedProfile, type ProfileGroup, type ProfileMeta, type ProfileSettings } from "../../api/client";
import type { ProfileReference } from "../../app/useProfileReferences";
import { Button } from "../Button";
import { Checkbox } from "../Checkbox";
import { Dialog } from "../Dialog";
import { Input } from "../Input";
import { PROFILE_GROUPS, PROFILE_GROUP_LABELS } from "./profile-groups";
import { ProfileParameters } from "./ProfileParameters";
import { ProfileRuleImpact } from "./ProfileRuleImpact";
import { profileDifferences } from "./profile-differences";

type Capture = { groups: ProfileGroup[]; data: CapturedProfile | null; error: string | null };

export function ProfileSaveDialog({ updating, profiles, rules, rulesLoading, rulesError, busy, error, onClose, onSave }: {
  updating: ProfileMeta | null;
  profiles: ProfileMeta[];
  rules: ProfileReference[];
  rulesLoading: boolean;
  rulesError: string | null;
  busy: boolean;
  error: string | null;
  onClose: () => void;
  onSave: (name: string, settings: ProfileSettings, existing?: ProfileMeta) => Promise<boolean>;
}) {
  const [name, setName] = useState(updating?.name ?? "");
  const [groups, setGroups] = useState<ProfileGroup[]>(updating?.groups ?? ["performance"]);
  const [capture, setCapture] = useState<Capture | null>(null);
  const [refresh, setRefresh] = useState(0);
  useEffect(() => {
    if (!groups.length) return;
    let disposed = false;
    captureProfileSettings(groups).then(data => {
      if (!disposed) setCapture({ groups, data, error: null });
    }).catch(error => {
      if (!disposed) setCapture({ groups, data: null, error: String(error) });
    });
    return () => { disposed = true; };
  }, [groups, refresh]);

  const current = capture?.groups === groups ? capture : null;
  const trimmed = name.trim();
  const existing = profiles.find(profile => profile.name === trimmed);
  const missing = !!updating && !existing;
  const unavailable = current?.data ? groups.filter(group => !current.data?.groups.includes(group)) : [];
  const summary = current?.data && profileDifferences(current.data.settings, null).filter(row => !row.detailsOnly).slice(0, 3);
  const submit = async () => {
    if (!current?.data || !trimmed || busy || missing) return;
    if (await onSave(trimmed, current.data.settings, existing)) onClose();
  };
  const title = updating ? "用当前配置更新档案" : "保存当前配置";
  return <Dialog open title={title} busy={busy} onClose={onClose} actions={<>
    <Button variant="secondary" disabled={busy} onClick={onClose}>取消</Button>
    <Button icon="save" disabled={busy || !current?.data || !trimmed || missing || !!existing && rulesLoading}
      onClick={() => void submit()}>{updating || existing ? "覆盖档案" : "保存档案"}</Button>
  </>}>
    <div className="ccc-profile-save">
      <label>档案名<Input value={name} disabled={busy || !!updating} maxLength={60}
        onChange={event => setName(event.target.value)} placeholder="例如 夜间静音" /></label>
      <fieldset disabled={busy}>
        <legend>包含的配置</legend>
        <div className="ccc-profile-groups">{PROFILE_GROUPS.map(group => <Checkbox key={group}
          label={PROFILE_GROUP_LABELS[group]} checked={groups.includes(group)} onChange={event =>
            setGroups(event.target.checked ? [...groups, group] : groups.filter(item => item !== group))} />)}</div>
      </fieldset>
      {!groups.length && <p className="ccc-control-caption">请选择至少一组配置。</p>}
      {!!groups.length && !current && <p className="ccc-control-caption" role="status">正在读取所选配置…</p>}
      {current?.error && <div className="ccc-actions">
        <p className="ccc-control-error" role="alert">{current.error}</p>
        <Button variant="ghost" size="compact" icon="refresh" onClick={() => { setCapture(null); setRefresh(value => value + 1); }}>重试读取</Button>
      </div>}
      {current?.data && <>
        {!!unavailable.length && <p className="ccc-control-caption">
          {unavailable.map(group => `${PROFILE_GROUP_LABELS[group]}无可保存项`).join(" · ")}
        </p>}
        {!!summary?.length && <dl className="ccc-profile-capture-values" aria-label="已读取的配置">
          {summary.map(row => <div key={row.label}><dt>{row.label}</dt><dd>{row.target}</dd></div>)}
        </dl>}
        <ProfileParameters settings={current.data.settings} label="查看已读取的参数" />
      </>}
      {existing && <ProfileRuleImpact name={existing.name} rules={rules} operation="replace" loading={rulesLoading} error={rulesError} />}
      {missing && !error && <p className="ccc-control-error" role="alert">此档案已移除。关闭后可保存为新档案。</p>}
      {error && <p className="ccc-control-error" role="alert">{error}</p>}
    </div>
  </Dialog>;
}
