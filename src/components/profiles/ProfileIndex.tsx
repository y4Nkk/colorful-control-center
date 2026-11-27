import { useId, type ReactNode, type Ref } from "react";
import type { ProfileMeta } from "../../api/client";
import type { ProfileTarget } from "../../app/useProfilePreview";
import type { useProfiles } from "../../app/useProfiles";
import { Button } from "../Button";
import { Icon } from "../Icon";
import { Loading } from "../Loading";
import { Surface } from "../Surface";
import { PROFILE_GROUP_LABELS } from "./profile-groups";

type Model = ReturnType<typeof useProfiles>;

export function ProfileIndex({ model, actions, previewId, selectedRow, onSelect }: {
  model: Model; actions: ReactNode; previewId: string;
  selectedRow: Ref<HTMLButtonElement>; onSelect: (target: ProfileTarget) => void;
}) {
  const { builtin, target, imported, busy, pending } = model;
  const rowProps = { disabled: busy || pending, previewId, selectedRow };
  return <div className="ccc-profile-index">
    <section className="ccc-profile-section" aria-label="内置档案">
      <h2 className="ccc-panel-title">内置档案</h2>
      <Surface variant="group" className="ccc-profile-list">
        {!builtin.catalog && builtin.loading && <Loading className="ccc-profile-message" label="正在读取内置档案…" />}
        {builtin.error && <ListError error={builtin.error} label="重新读取内置档案" onRetry={builtin.reload} />}
        {builtin.catalog?.profiles.map(profile => <ProfileRow key={profile.id} {...rowProps} title={profile.name}
          description={profile.description} selected={!imported && target?.kind === "builtin" && target.id === profile.id}
          onSelect={() => onSelect({ kind: "builtin", id: profile.id })} />)}
      </Surface>
    </section>
    <section className="ccc-profile-section" aria-label="档案列表">
      <header className="ccc-profile-section-head">
        <h2 className="ccc-panel-title">我的档案</h2>
        <div className="ccc-actions">{actions}</div>
      </header>
      <Surface variant="group" className="ccc-profile-list">
        {model.status === "ready" && !model.profiles.length && <p className="ccc-profile-message ccc-control-caption">还没有个人档案</p>}
        {model.status === "loading" && <Loading className="ccc-profile-message" label="正在读取档案…" />}
        {model.listError && model.status === "error" && <ListError error={model.listError} label="重新读取档案" onRetry={model.reload} />}
        {model.rulesError && <p className="ccc-profile-message ccc-control-caption is-warn">部分规则引用无法读取 · {model.rulesError}</p>}
        {model.profiles.map(profile => <ProfileRow key={profile.name} {...rowProps} title={profile.name}
          description={profile.groups.map(group => PROFILE_GROUP_LABELS[group]).join(" · ")}
          ruleLabels={<ProfileRuleLabels profile={profile} rules={model.rules} />}
          selected={!imported && target?.kind === "saved" && target.name === profile.name}
          onSelect={() => onSelect({ kind: "saved", name: profile.name })} />)}
      </Surface>
    </section>
  </div>;
}

function ProfileRow({ title, description, ruleLabels, selected, disabled, previewId, selectedRow, onSelect }: {
  title: string; description: string; ruleLabels?: ReactNode; selected: boolean; disabled: boolean;
  previewId: string; selectedRow: Ref<HTMLButtonElement>; onSelect: () => void;
}) {
  const descriptionId = useId();
  return <div>
    <Button ref={selected ? selectedRow : undefined} variant="ghost" className="ccc-profile-choice"
      aria-label={`${title}，预览`} aria-describedby={descriptionId} aria-pressed={selected} aria-controls={previewId}
      disabled={disabled} onClick={onSelect}>
      <span className="ccc-profile-row">
        <span className="ccc-setting-copy">
          <span className="ccc-settings-label">{title}</span>
          <span id={descriptionId} className="ccc-control-caption">{description}{ruleLabels}</span>
        </span>
        <Icon name="chevron-right" size={16} />
      </span>
    </Button>
  </div>;
}

function ProfileRuleLabels({ profile, rules }: { profile: ProfileMeta; rules: Model["rules"] }) {
  const linked = rules.filter(rule => rule.name === profile.name);
  if (!linked.length) return null;
  return <span className="ccc-profile-rule-labels">{linked.map(rule =>
    <span key={rule.label} data-stale={rule.revision !== profile.revision}>
      {rule.label}{rule.revision !== profile.revision ? " · 待确认" : ""}
    </span>)}</span>;
}

function ListError({ error, label, onRetry }: { error: string; label: string; onRetry: () => Promise<void> }) {
  return <div className="ccc-setting-row ccc-profile-message">
    <p className="ccc-control-error" role="alert">{error}</p>
    <Button icon="refresh" variant="ghost" size="compact" onClick={() => void onRetry()}>{label}</Button>
  </div>;
}
