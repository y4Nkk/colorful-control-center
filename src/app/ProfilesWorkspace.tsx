import { useId, useLayoutEffect, useRef, useState } from "react";
import type { ProfileFile, ProfileMeta } from "../api/client";
import { Button } from "../components/Button";
import { Input } from "../components/Input";
import { OemConflictAction } from "../components/OemConflictAction";
import { ProfileSaveDialog } from "../components/profiles/ProfileSaveDialog";
import { ProfilePreviewPane } from "../components/profiles/ProfileDetail";
import { ProfileCopyDialog } from "../components/profiles/BuiltinProfileControls";
import { ProfileIndex } from "../components/profiles/ProfileIndex";
import { useProfiles } from "./useProfiles";
import type { ProfileRequest, ProfileTarget } from "./useProfilePreview";

export function ProfilesWorkspace({ active, requestedProfile, onOpenRules }: {
  active: boolean;
  requestedProfile?: ProfileRequest | null;
  onOpenRules: () => void;
}) {
  const model = useProfiles(active, requestedProfile);
  const { target, imported } = model;
  const previewId = useId();
  const selectedRow = useRef<HTMLButtonElement>(null);
  const fileInput = useRef<HTMLInputElement>(null);
  const importTrigger = useRef<HTMLButtonElement>(null);
  const saveTrigger = useRef<HTMLButtonElement>(null);
  const [editor, setEditor] = useState<{ updating: ProfileMeta | null }>();
  const [copy, setCopy] = useState<ProfileFile | null>(null);
  const { heading, focusNextPreview } = useProfilePreviewFocus(active && !editor && !copy, model.loading, imported, target, requestedProfile);

  const select = (target: ProfileTarget) => { focusNextPreview.current = true; model.select(target); };
  const persist: typeof model.persist = async (...args) => {
    focusNextPreview.current = true;
    const saved = await model.persist(...args);
    if (!saved) focusNextPreview.current = false;
    return saved;
  };
  const stageImport = async (file: File) => {
    focusNextPreview.current = true;
    if (!await model.stageImport(file)) focusNextPreview.current = false;
  };
  const dismiss = () => {
    if (imported) model.dismissImport();
    else model.dismissPreview();
    (imported ? importTrigger : selectedRow).current?.focus();
  };
  const startEditor = (updating: ProfileMeta | null) => { model.clearError(); setEditor({ updating }); };
  const closeEditor = () => {
    setEditor(undefined);
    model.clearError();
    if (active) requestAnimationFrame(() => {
      if (document.activeElement === document.body) saveTrigger.current?.focus();
    });
  };
  const actionError = model.error === model.selection?.review.blocked ? null : model.error;
  return <div className="ccc-control" data-module-layout="profiles">
    <Input ref={fileInput} type="file" aria-label="导入配置档案" accept=".json,application/json" hidden onChange={event => {
      const file = event.target.files?.[0];
      event.target.value = "";
      if (file) void stageImport(file);
    }} />
    {!editor && !copy && actionError && <p className="ccc-control-error" role="alert">{actionError}</p>}
    {!editor && !copy && <OemConflictAction issues={[actionError]} disabled={model.busy} onClosed={model.retry} />}
    <div className="ccc-profile-layout" data-preview-open={!!(target || imported) || undefined}>
      <ProfileIndex model={model} previewId={previewId} selectedRow={selectedRow} onSelect={select} actions={<>
        <Button ref={saveTrigger} icon="add" variant="secondary" size="compact" disabled={model.busy || model.status === "loading"}
          onClick={() => startEditor(null)}>保存当前配置</Button>
        <Button ref={importTrigger} icon="upload" variant="ghost" size="compact" disabled={model.busy}
          onClick={() => fileInput.current?.click()}>导入档案</Button>
      </>} />
      <ProfilePreviewPane id={previewId} heading={heading} model={model} onUpdate={startEditor} onPersist={persist}
        onOpenRules={onOpenRules} onDismiss={dismiss} onCopy={file => { model.clearError(); setCopy(file); }} />
    </div>
    {editor && <ProfileSaveDialog updating={editor.updating} profiles={model.profiles}
      rules={model.rules} rulesLoading={model.rulesLoading} rulesError={model.rulesError} busy={model.busy} error={model.error}
      onClose={closeEditor}
      onSave={(name, settings, existing) => persist(name, settings, existing, "save")} />}
    {copy && <ProfileCopyDialog file={copy} profiles={model.profiles} busy={model.busy} error={model.error}
      onClose={() => { setCopy(null); model.clearError(); }}
      onCopy={name => persist(name, copy.settings, undefined, "copy")} />}
  </div>;
}

function useProfilePreviewFocus(visible: boolean, loading: boolean, imported: ProfileFile | null, target: ProfileTarget | null,
  requested?: ProfileRequest | null) {
  const heading = useRef<HTMLHeadingElement>(null);
  const focusNextPreview = useRef(false);
  const focusedRequest = useRef<ProfileRequest | null>(null);
  useLayoutEffect(() => {
    if (!visible || !heading.current) return;
    const fromTray = requested && requested !== focusedRequest.current && !imported
      && target?.kind === "saved" && target.name === requested.name;
    if (fromTray) focusedRequest.current = requested;
    if (focusNextPreview.current || fromTray) {
      focusNextPreview.current = false;
      heading.current.focus({ preventScroll: true });
    } else if (loading || document.activeElement !== heading.current) return;
    heading.current.scrollIntoView({ block: "start", inline: "nearest" });
  }, [visible, loading, imported, requested, target]);
  return { heading, focusNextPreview };
}
