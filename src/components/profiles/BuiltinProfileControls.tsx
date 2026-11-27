import { useId, useState } from "react";
import type { ProfileFile, ProfileMeta } from "../../api/client";
import type { useBuiltinProfiles } from "../../app/useBuiltinProfiles";
import { Button } from "../Button";
import { Dialog } from "../Dialog";
import { Input } from "../Input";
import { Select } from "../Select";
import { Switch } from "../Switch";

export function BuiltinRefreshControls({ model, disabled }: { model: ReturnType<typeof useBuiltinProfiles>; disabled: boolean }) {
  const id = useId();
  const { catalog, choice, setChoice } = model;
  const refresh = catalog?.refresh;
  const issue = catalog?.refreshIssue;
  const stale = choice && refresh && choice.revision !== refresh.revision;
  return <div className="ccc-builtin-refresh">
    <div className="ccc-setting-row">
      <div className="ccc-setting-copy">
        <span className="ccc-settings-label">内屏刷新率</span>
        <p id={id}>{issue ?? "随内置档案切换"}</p>
      </div>
      <Switch label="随内置档案切换内屏刷新率" checked={!!choice} describedBy={id}
        disabled={disabled || !choice && (!refresh || model.loading)}
        onChange={enabled => setChoice(enabled && refresh ? { revision: refresh.revision, officeHz: refresh.current } : null)} />
    </div>
    {choice && refresh && <div className="ccc-setting-row">
      <div className="ccc-setting-copy">
        <label className="ccc-settings-label" htmlFor={`${id}-office`}>办公刷新率</label>
        <p>续航 {refresh.low} Hz · 游戏 {refresh.high} Hz</p>
        {stale && <p className="ccc-control-caption is-warn">内屏候选已变化，请重新选择办公刷新率。</p>}
      </div>
      <div className="ccc-builtin-refresh-select">
        <Select id={`${id}-office`} label="办公刷新率" size="compact" value={stale ? "" : String(choice.officeHz)}
          disabled={disabled || model.loading} options={refresh.candidates.map(hz => ({ value: String(hz), label: `${hz} Hz` }))}
          onChange={value => setChoice({ revision: refresh.revision, officeHz: Number(value) })} />
      </div>
    </div>}
  </div>;
}

export function ProfileCopyDialog({ file, profiles, busy, error, onClose, onCopy }: {
  file: ProfileFile; profiles: ProfileMeta[]; busy: boolean; error: string | null;
  onClose: () => void; onCopy: (name: string) => Promise<boolean>;
}) {
  const [name, setName] = useState(`${file.name} 副本`);
  const trimmed = name.trim();
  const occupied = profiles.some(profile => profile.name === trimmed);
  const save = async () => {
    if (!busy && trimmed && !occupied && await onCopy(trimmed)) onClose();
  };
  return <Dialog open title="复制为我的档案" busy={busy} onClose={onClose} actions={<>
    <Button variant="secondary" disabled={busy} onClick={onClose}>取消</Button>
    <Button icon="save" disabled={busy || !trimmed || occupied} onClick={() => void save()}>保存副本</Button>
  </>}>
    <div className="ccc-profile-save">
      <label>档案名<Input value={name} disabled={busy} maxLength={60} onChange={event => setName(event.target.value)} /></label>
      {occupied && <p className="ccc-control-error" role="alert">已有同名个人档案，请更换名称。</p>}
      {error && <p className="ccc-control-error" role="alert">{error}</p>}
    </div>
  </Dialog>;
}
