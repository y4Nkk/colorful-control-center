import { Icon } from "../components/Icon";
import { Surface } from "../components/Surface";
import { Input } from "../components/Input";
import { Button } from "../components/Button";
import { DetailsDialog } from "../components/DetailsDialog";
import { useRef } from "react";
import { ConfirmButton } from "../components/ConfirmButton";
import { ConfigurationAction } from "../components/ConfigurationAction";
import { ProfilePreview } from "../components/profiles/ProfilePreview";
import { ProfileArtwork } from "../components/profiles/ProfileArtwork";
import { useProfiles } from "./useProfiles";

export function ProfilesWorkspace({ active, requestedProfile }: { active: boolean; requestedProfile?: { name: string } | null }) {
  const model = useProfiles(active, requestedProfile);
  const fileInput = useRef<HTMLInputElement>(null);
  const settings = model.preview?.settings;
  const preview = settings ? <ProfilePreview settings={settings} modeTarget={model.modeTarget}
    displayReview={model.displayReview} gpuPowerReview={model.gpuPowerReview} graphicsModeReview={model.graphicsModeReview} /> : null;
  return (
    <div className="ccc-control ccc-profiles">
      <Surface as="section" variant="group" className="ccc-panel" aria-label="新建档案">
        <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="profiles" />新建档案</h2></div>
        <div className="ccc-actions">
          <label className="ccc-profile-name">档案名
            <Input value={model.name} disabled={model.busy} maxLength={60}
              onChange={(e) => model.setName(e.target.value)} placeholder="游戏 / 静音" />
          </label>
          <ConfirmButton icon="save" key={model.name} label="保存当前" confirmLabel={model.profiles.some((p) => p.name === model.name.trim()) ? "覆盖同名档案？" : "保存当前设置？"}
            disabled={!model.name.trim()} busy={model.busy} onConfirm={model.save} />
          <Button icon="upload" variant="ghost" type="button" disabled={model.busy} onClick={() => fileInput.current?.click()}>导入</Button>
          <Input ref={fileInput} type="file" aria-label="导入性能档案" accept=".json,application/json" hidden onChange={(e) => {
            const file = e.target.files?.[0]; e.target.value = ""; if (file) void model.stageImport(file);
          }} />
        </div>
        {model.error && <p className="ccc-control-error" role="alert">{model.error}</p>}
      </Surface>
      <div className="ccc-profile-layout">
        <Surface as="section" variant="group" className="ccc-panel" aria-label="档案列表">
          <h2 className="ccc-panel-title">档案</h2>
          {model.profiles.length === 0 ? <span className="ccc-chip">暂无档案</span> : model.profiles.map((p) => (
            <Button variant="selection" aria-pressed={model.preview?.name === p.name} key={p.name} className="ccc-profile-choice"
              disabled={model.busy} onClick={() => { void model.select(p.name); }}>
              <strong>{p.name}</strong><time>{new Date(p.savedAtMs).toLocaleDateString("zh-CN")}</time>
            </Button>
          ))}
        </Surface>
        <Surface as="section" variant="group" className="ccc-panel" aria-label="档案预览">
          {model.preview ? <>
            <div className="ccc-panel-head"><h2 className="ccc-panel-title">{model.preview.name}</h2>
              <div className="ccc-module-tools"><DetailsDialog title="档案详细信息" label="详情" wide>{preview}</DetailsDialog></div>
            </div>
            <div className="ccc-profile-identity"><ProfileArtwork /><div className="ccc-module-summary">
              <strong>{model.imported ? "待导入" : "已保存"}</strong>
              <time className="ccc-control-caption">{new Date(model.preview.savedAtMs).toLocaleDateString("zh-CN")}</time>
            </div></div>
            {!model.imported && <div className="ccc-actions">
              <Button icon="download" variant="ghost" type="button" disabled={model.busy} onClick={() => void model.download()}>导出</Button>
              <ConfirmButton icon="delete" key={`delete:${JSON.stringify(model.preview)}`} label="删除" confirmLabel="删除档案"
                busy={model.busy} onConfirm={model.remove}><p>删除「{model.preview.name}」及其开机应用设置。</p></ConfirmButton>
            </div>}
            <ProfileChanges model={model} preview={preview} />
          </> : <div className="ccc-profile-identity"><ProfileArtwork /><p className="ccc-control-caption">选择档案查看设置</p></div>}
          {model.application && model.application.steps.length > 0 && <Surface as="section" variant="group" className="ccc-profile-results" aria-label="最近应用记录">
            <div className="ccc-panel-head"><h3 className="ccc-panel-title"><Icon name="logs" />最近执行 · {model.application.name}</h3>
              <span className="ccc-chip">{model.application.targetsApplied ? "目标已确认" : "未完成全部目标"}</span></div>
            {model.application.targetsApplied && model.application.steps.some(step => !step.ok) && <p className="ccc-control-caption">目标设置已应用，部分收尾待处理。</p>}
            {model.application.steps.filter(step => !step.ok).map((step, i) => <p key={i} className="ccc-control-error">{step.step}：{step.message}</p>)}
            <DetailsDialog title={`执行明细 · ${model.application.steps.length} 项`}><ol className="ccc-profile-steps" aria-label="执行结果">
            {model.application.steps.map((step, index) => <li key={`${index}-${step.step}`}>
              <span className="ccc-step-status"><Icon name={step.ok ? "check-circle" : "error"} label={step.ok ? "成功" : "失败"} size={16} />{step.step}</span><span>{step.message}</span>
            </li>)}
            </ol></DetailsDialog>
          </Surface>}
        </Surface>
      </div>
    </div>
  );
}

function ProfileChanges({ model, preview }: { model: ReturnType<typeof useProfiles>; preview: React.ReactNode }) {
  const file = model.preview;
  if (!file) return null;
  const { settings } = file;
  const blocked = model.applicationPending || settings.oemPowerMode != null && !model.modeTarget ||
    settings.display != null && !model.displayReview || settings.gpuPower != null && !model.gpuPowerReview ||
    settings.graphicsMode != null && !model.graphicsModeReview;
  const revision = JSON.stringify([file, model.imported, model.modeTarget, model.displayReview, model.gpuPowerReview, model.graphicsModeReview]);
  return <ConfigurationAction label={file.name} revision={revision} disabled={model.busy || !model.imported && blocked}
    confirmLabel={model.imported ? "导入档案" : "应用档案"} onConfirm={model.imported ? model.confirmImport : model.apply}>
    {model.imported && model.profiles.some(profile => profile.name === file.name) && <p>导入将覆盖同名档案。</p>}
    {preview}
  </ConfigurationAction>;
}
