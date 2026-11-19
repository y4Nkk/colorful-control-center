import { Icon } from "../Icon";
import { useState } from "react";
import { applyKeymap, restoreKeymap, retryKeymapPrograms, type KeymapBinding, type KeymapDevice } from "../../api/client";
import { changedDevices, configurationError, keymapChanges } from "../../lib/keymap";
import { Button } from "../Button";
import { DetailsDialog } from "../DetailsDialog";
import { ConfigurationAction } from "../ConfigurationAction";
import { FactList } from "../FactList";
import { SegmentedControl } from "../SegmentedControl";
import { Surface } from "../Surface";
import { KeymapPreview } from "./KeymapPreview";
import { KeymapActionEditor } from "./KeymapActionEditor";
import { KeymapBindingList } from "./KeymapBindingList";
import { KeymapFiles } from "./KeymapFiles";
import { useKeymap } from "./useKeymap";

export function KeymapSettings({ active, revision = 0 }: { active: boolean; revision?: number }) {
  const model = useKeymap(active, revision), { profile, draft, busy, error } = model;
  const [device, setDevice] = useState<KeymapDevice>("keyboard"), [selected, setSelected] = useState<number | null>(null), [recording, setRecording] = useState(false);
  if (model.loaded && !profile && !error) return null;
  const hasDraft = !!profile?.configuration && !!draft;
  const group = draft?.devices.find(group => group.device === device), binding = group?.bindings.find(binding => binding.trigger === selected);
  const locked = busy || recording || !profile?.configuration || !!profile.blockedReason || profile.recoveryRequired;
  const invalid = draft ? configurationError(draft) : null, changes = profile && draft ? keymapChanges(profile, draft) : [];
  const target = profile && draft ? changedDevices(profile, draft) : null;
  const changeBinding = (next: KeymapBinding) => {
    if (!group) return;
    model.changeGroup({ ...group, bindings: group.bindings.map(binding => binding.trigger === selected ? next : binding) }); setSelected(next.trigger);
  };
  return <Surface as="section" variant="group" className="ccc-panel" aria-label="按键映射与宏">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="keyboard" />按键映射与宏</h2>
      <div className="ccc-module-tools">
        <DetailsDialog title="映射详情" label="详情">
          {profile && <FactList compact facts={[
            { label: "来源", value: profile.sourceName },
            { label: "键盘映射", value: profile.enabledFlags[0] ? "开启" : "关闭" },
            { label: "鼠标映射", value: profile.enabledFlags[1] ? "开启" : "关闭" },
            { label: "恢复目标", value: profile.baselineName ?? "首次应用前记录原配置" },
          ]} />}
          {profile?.blockedReason && <p>{profile.blockedReason}</p>}
          {profile?.warnings.map(warning => <p key={warning}>{warning}</p>)}
          {error && <p className="ccc-control-error">{error}</p>}
        </DetailsDialog>
        <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取映射" disabled={busy || recording} onClick={model.reread} />
      </div>
    </div>
    <div className="ccc-keyboard-summary">
      <div className="ccc-module-summary" role="status">
        <strong>{error ? "操作失败 · 查看详情" : !profile ? "读取中…" : !hasDraft ? "缺少可恢复的原配置"
          : `${draft.devices.reduce((count, item) => count + item.bindings.length, 0)} 项映射`}</strong>
        {profile?.blockedReason && draft && <span className="ccc-control-caption">暂不可修改 · 查看详情</span>}
        {profile?.recoveryRequired && <span className="ccc-control-error">请先恢复初始配置</span>}
      </div>
      {hasDraft && <div className="ccc-actions"><DetailsDialog title="编辑按键映射与宏" label="编辑映射" icon="edit" editor wide busy={recording}>
        <div className="ccc-keymap-columns">
          <Surface as="div" variant="inset" className="ccc-keymap-section">
            {profile.programsRequired && <div className="ccc-keymap-program-status">
              {profile.recoveryRequired && <p className="ccc-control-caption" role="status">程序宏需先恢复配置</p>}
              {profile.programs.error && <p className="ccc-control-error" role="alert">{profile.programs.error}</p>}
              <Button icon="play" variant="secondary" size="compact" disabled={!active || locked || profile.programs.running}
                onClick={() => void model.run(() => retryKeymapPrograms(profile.revision), "程序宏监听已启用", false)}>
                {profile.programs.running ? "程序宏监听中" : "启用程序宏监听"}
              </Button>
            </div>}
            {profile.programsRequired && <span className="ccc-control-caption">程序宏需应用保持运行</span>}
            <SegmentedControl label="映射设备" value={device} options={[{ value: "keyboard", label: "键盘" }, { value: "mouse", label: "鼠标" }]}
              disabled={busy || recording} onChange={value => { setDevice(value as KeymapDevice); setSelected(null); }} />
            {group && <KeymapBindingList group={group} selected={selected} enabled={profile.enabledFlags[device === "keyboard" ? 0 : 1] === 1}
              disabled={locked} onSelect={setSelected} onChange={model.changeGroup}
              onToggle={enabled => { if (!locked) void model.toggle(device, enabled); }} />}
            <KeymapFiles draft={draft} disabled={locked || !active} onImport={next => { model.setDraft(next); setSelected(null); }} />
          </Surface>
          {binding && <Surface as="div" variant="inset" className="ccc-keymap-section">
            <KeymapActionEditor active={active} binding={binding} device={device} configuration={draft}
              disabled={locked && !recording} recording={recording} onRecordingChange={setRecording} onChange={changeBinding}
              onRemove={() => { if (group) model.changeGroup({ ...group, bindings: group.bindings.filter(binding => binding.trigger !== selected) }); setSelected(null); }} />
          </Surface>}
        </div>
      </DetailsDialog></div>}
    </div>
    {hasDraft && <>
      {invalid && <p className="ccc-control-error" role="status">{invalid}</p>}
      <ConfigurationAction revision={JSON.stringify([profile.revision, target])} label="按键映射与宏" applyLabel="应用按键映射"
        disabled={!active || locked || !!invalid || !target?.devices.length}
        onConfirm={() => target ? model.run(() => applyKeymap(profile.revision, target), "映射已提交并保存") : undefined}>
        <KeymapPreview items={changes} />
      </ConfigurationAction>
    </>}
    {profile && <ConfigurationAction revision={profile.revision} label="按键映射与宏" restore
      disabled={!active || busy || recording || !!profile.blockedReason || !profile.canRestore}
      onConfirm={() => model.run(() => restoreKeymap(profile.revision), "初始映射已恢复")}>
      <p>恢复最初接管前的映射。</p>
      {profile.baselineName && <p className="ccc-control-caption">{profile.baselineName}</p>}
    </ConfigurationAction>}
  </Surface>;
}
