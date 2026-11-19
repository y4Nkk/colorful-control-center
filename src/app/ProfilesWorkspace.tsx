import { useRef, useState } from "react";
import type { ProfileMeta } from "../api/client";
import { Button } from "../components/Button";
import { Input } from "../components/Input";
import { Surface } from "../components/Surface";
import { ProfileSaveDialog } from "../components/profiles/ProfileSaveDialog";
import { ProfileDetail, ProfileImport } from "../components/profiles/ProfileDetail";
import { PROFILE_GROUP_LABELS } from "../components/profiles/profile-groups";
import { useProfiles } from "./useProfiles";

export function ProfilesWorkspace({ active, requestedProfile, onOpenRules }: {
  active: boolean;
  requestedProfile?: { name: string } | null;
  onOpenRules: () => void;
}) {
  const model = useProfiles(active, requestedProfile);
  const fileInput = useRef<HTMLInputElement>(null);
  const saveTrigger = useRef<HTMLButtonElement>(null);
  const [editor, setEditor] = useState<{ updating: ProfileMeta | null }>();
  const startEditor = (updating: ProfileMeta | null) => { model.clearError(); setEditor({ updating }); };
  const closeEditor = () => {
    setEditor(undefined);
    model.clearError();
    if (active) requestAnimationFrame(() => {
      if (document.activeElement === document.body) saveTrigger.current?.focus();
    });
  };
  const empty = model.status === "ready" && !model.profiles.length;
  const actionError = model.error === model.selection?.review.blocked ? null : model.error;
  const createAndImport = <div className="ccc-actions">
    <Button ref={saveTrigger} icon="add" disabled={model.busy || model.status === "loading"} onClick={() => startEditor(null)}>保存当前配置</Button>
    <Button icon="upload" variant="secondary" disabled={model.busy} onClick={() => fileInput.current?.click()}>导入档案</Button>
  </div>;
  return <div className="ccc-control">
    {empty && !model.imported
      ? <Surface as="section" variant="group" className="ccc-panel ccc-profile-empty" aria-label="首次创建档案">
        <div>
          <h2 className="ccc-panel-title">还没有配置档案</h2>
          <p className="ccc-control-caption">保存常用的设备设置，下次一起应用。</p>
        </div>
        {createAndImport}
      </Surface>
      : <div className="ccc-profile-toolbar">{createAndImport}</div>}
    <Input ref={fileInput} type="file" aria-label="导入配置档案" accept=".json,application/json" hidden onChange={event => {
      const file = event.target.files?.[0];
      event.target.value = "";
      if (file) void model.stageImport(file);
    }} />
    {!editor && actionError && <p className="ccc-control-error" role="alert">{actionError}</p>}
    {model.status === "loading" && <p className="ccc-control-caption" role="status">正在读取档案…</p>}
    {model.listError && model.status === "error" && <div className="ccc-actions">
      <p className="ccc-control-error" role="alert">{model.listError}</p>
      <Button icon="refresh" variant="ghost" onClick={() => void model.reload()}>重新读取档案</Button>
    </div>}
    {model.rulesError && <p className="ccc-control-caption is-warn">部分规则引用无法读取 · {model.rulesError}</p>}
    {(model.profiles.length > 0 || model.imported) && <div className="ccc-module-grid" data-module-layout={model.profiles.length ? "profiles" : "profile-import"}>
      {model.profiles.length > 0 && <Surface as="section" variant="group" className="ccc-panel ccc-profile-list" aria-label="档案列表">
        {model.profiles.map(profile => <ProfileChoice key={profile.name} profile={profile} model={model} />)}
      </Surface>}
      {model.imported
        ? <ProfileImport model={model} file={model.imported} />
        : <ProfileDetail model={model} onUpdate={startEditor} onOpenRules={onOpenRules} />}
    </div>}
    {editor && <ProfileSaveDialog updating={editor.updating} profiles={model.profiles}
      rules={model.rules} rulesLoading={model.rulesLoading} rulesError={model.rulesError} busy={model.busy} error={model.error}
      onClose={closeEditor}
      onSave={(name, settings, existing) => model.persist(name, settings, existing, "save")} />}
  </div>;
}

function ProfileChoice({ profile, model }: { profile: ProfileMeta; model: ReturnType<typeof useProfiles> }) {
  return <Button variant="selection" aria-pressed={!model.imported && model.selectedName === profile.name}
    className="ccc-profile-choice" disabled={model.busy} onClick={() => model.select(profile.name)}>
    <strong>{profile.name}</strong>
    <span className="ccc-control-caption">{profile.groups.map(group => PROFILE_GROUP_LABELS[group]).join(" · ")}</span>
    <ProfileRuleLabels profile={profile} model={model} />
  </Button>;
}

function ProfileRuleLabels({ profile, model }: { profile: ProfileMeta; model: ReturnType<typeof useProfiles> }) {
  const rules = model.rules.filter(rule => rule.name === profile.name);
  if (!rules.length) return null;
  return <span className="ccc-profile-rule-labels">{rules.map(rule =>
    <span key={rule.label} data-stale={rule.revision !== profile.revision}>
      {rule.label}{rule.revision !== profile.revision ? " · 待确认" : ""}
    </span>)}</span>;
}
