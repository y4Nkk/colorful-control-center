import { Icon } from "../components/Icon";
import { Surface } from "../components/Surface";
import { Input } from "../components/Input";
import { Button } from "../components/Button";
import { Collapsible } from "../components/Collapsible";
import { useRef } from "react";
import { ConfirmButton } from "../components/ConfirmButton";
import { ProfilePreview } from "../components/profiles/ProfilePreview";
import { useProfiles } from "./useProfiles";

export function ProfilesWorkspace({ active, requestedProfile }: { active: boolean; requestedProfile?: { name: string } | null }) {
  const model = useProfiles(active, requestedProfile);
  const fileInput = useRef<HTMLInputElement>(null);
  return (
    <div className="ccc-control ccc-profiles">
      <Surface as="section" variant="group" className="ccc-panel">
        <div className="ccc-actions">
          <label className="ccc-profile-name">档案名
            <Input value={model.name} disabled={model.busy} maxLength={60}
              onChange={(e) => model.setName(e.target.value)} placeholder="游戏 / 静音" />
          </label>
          <ConfirmButton icon="save" key={model.name} label="保存当前" confirmLabel={model.profiles.some((p) => p.name === model.name.trim()) ? "覆盖同名档案？" : "保存当前设置？"}
            disabled={!model.name.trim()} busy={model.busy} onConfirm={model.save} />
          <Button icon="upload" type="button" disabled={model.busy} onClick={() => fileInput.current?.click()}>导入</Button>
          <Input ref={fileInput} type="file" aria-label="导入性能档案" accept=".json,application/json" hidden onChange={(e) => {
            const file = e.target.files?.[0]; e.target.value = ""; if (file) void model.stageImport(file);
          }} />
        </div>
        {model.error && <p className="ccc-control-error" role="alert">{model.error}</p>}
      </Surface>
      <div className="ccc-profile-layout">
        <Surface as="section" variant="group" className="ccc-panel" aria-label="档案列表">
          {model.profiles.length === 0 ? <span className="ccc-chip">暂无档案</span> : model.profiles.map((p) => (
            <Button variant="selection" aria-pressed={model.preview?.name === p.name} key={p.name} className="ccc-profile-choice"
              disabled={model.busy} onClick={() => { void model.select(p.name); }}>
              <strong>{p.name}</strong><time>{new Date(p.savedAtMs).toLocaleDateString("zh-CN")}</time>
            </Button>
          ))}
        </Surface>
        <Surface as="section" variant="group" className="ccc-panel" aria-label="档案预览">
          {model.preview ? <>
            <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="profiles" />{model.preview.name}</h2>
              <span className="ccc-chip">{model.imported ? "待导入" : "预览"}</span></div>
            <ProfilePreview settings={model.preview.settings} modeTarget={model.modeTarget} displayReview={model.displayReview} gpuPowerReview={model.gpuPowerReview} graphicsModeReview={model.graphicsModeReview} />
            <div className="ccc-actions">
              {model.imported ? <ConfirmButton key={JSON.stringify(model.preview)} label="确认导入" confirmLabel="导入并覆盖同名档案？" busy={model.busy} onConfirm={model.confirmImport} /> : <>
                <ConfirmButton key={JSON.stringify([model.preview, model.modeTarget, model.displayReview, model.gpuPowerReview, model.graphicsModeReview])} label="应用" confirmLabel={`应用「${model.preview.name}」？`} disabled={model.applicationPending || (model.preview.settings.oemPowerMode != null && !model.modeTarget) || (model.preview.settings.display != null && !model.displayReview) || (model.preview.settings.gpuPower != null && !model.gpuPowerReview) || (model.preview.settings.graphicsMode != null && !model.graphicsModeReview)} busy={model.busy} onConfirm={model.apply} />
                <Button icon="download" variant="secondary" type="button" disabled={model.busy} onClick={() => { void model.download(); }}>导出</Button>
                <ConfirmButton icon="delete" key={`delete:${JSON.stringify(model.preview)}`} variant="secondary" label="删除" confirmLabel={`删除「${model.preview.name}」及其开机应用设置？`} busy={model.busy} onConfirm={model.remove} />
              </>}
            </div>
          </> : <span className="ccc-chip">选择档案预览</span>}
          {model.application && model.application.steps.length > 0 && <Surface as="section" variant="group" className="ccc-profile-results" aria-label="最近应用记录">
            <div className="ccc-panel-head"><h3 className="ccc-panel-title"><Icon name="logs" />最近执行 · {model.application.name}</h3>
              <span className="ccc-chip">{model.application.targetsApplied ? "目标已确认" : "未完成全部目标"}</span></div>
            {model.application.targetsApplied && model.application.steps.some(step => !step.ok) && <p className="ccc-control-caption">目标设置已应用，部分收尾待处理。</p>}
            {model.application.steps.filter(step => !step.ok).map((step, i) => <p key={i} className="ccc-control-error">{step.step}：{step.message}</p>)}
            <Collapsible summary={`执行明细 · ${model.application.steps.length} 项`}><ol className="ccc-profile-steps" aria-label="执行结果">
            {model.application.steps.map((step, index) => <li key={`${index}-${step.step}`}>
              <span className="ccc-step-status"><Icon name={step.ok ? "check-circle" : "error"} label={step.ok ? "成功" : "失败"} size={16} />{step.step}</span><span>{step.message}</span>
            </li>)}
            </ol></Collapsible>
          </Surface>}
        </Surface>
      </div>
    </div>
  );
}
