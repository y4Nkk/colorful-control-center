/**
 * 性能档案（Phase 3）：把当前设置（风扇曲线 + 模式 + PL + 电压偏移）打包
 * 保存/应用/删除。应用流水线每步独立报告成败。
 */
import { useCallback, useEffect, useRef, useState } from "react";
import {
  applyProfile,
  captureProfileSettings,
  deleteProfile,
  exportProfile,
  importProfile,
  listProfiles,
  saveProfile,
  type ApplyStep,
  type ProfileMeta,
} from "../api/client";
import { ConfirmButton } from "../components/ConfirmButton";
import { toast } from "../components/use-toast";

export function ProfilesWorkspace({ active }: { active: boolean }) {
  const [profiles, setProfiles] = useState<ProfileMeta[]>([]);
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [lastSteps, setLastSteps] = useState<ApplyStep[] | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const reload = useCallback(() => {
    if (!active) return;
    listProfiles()
      .then((list) => {
        setProfiles(list);
        setError(null);
      })
      .catch((cause: unknown) => setError(cause instanceof Error ? cause.message : String(cause)));
  }, [active]);

  useEffect(() => {
    reload();
  }, [reload]);

  const save = () => {
    captureProfileSettings()
      .then((settings) =>
        saveProfile(name, settings).then((result) => ({ settings, result })),
      )
      .then(({ result }) => {
        if (result.ok) {
          toast({ kind: "success", title: `档案已保存：${name}` });
          reload();
        } else {
          toast({ kind: "error", title: "保存失败", description: result.message });
        }
      })
      .catch((cause: unknown) =>
        toast({ kind: "error", title: "保存失败", description: cause instanceof Error ? cause.message : String(cause) }),
      );
  };

  const apply = (target: string) => {
    applyProfile(target)
      .then((steps) => {
        setLastSteps(steps);
        const failed = steps.filter((step) => !step.ok);
        if (failed.length === 0) {
          toast({ kind: "success", title: `档案已应用：${target}` });
        } else {
          toast({
            kind: "warning",
            title: `应用完成，${failed.length} 步失败`,
            description: failed.map((step) => `${step.step}: ${step.message}`).join("；"),
          });
        }
      })
      .catch((cause: unknown) =>
        toast({ kind: "error", title: "应用失败", description: cause instanceof Error ? cause.message : String(cause) }),
      );
  };

  const remove = (target: string) => {
    deleteProfile(target).then((result) => {
      if (result.ok) {
        toast({ kind: "success", title: `档案已删除：${target}` });
        reload();
      } else {
        toast({ kind: "error", title: "删除失败", description: result.message });
      }
    });
  };

  return (
    <div className="ccc-dashboard">
      <div className="ccc-placeholder" style={{ display: "flex", gap: 12, alignItems: "center" }}>
        <label style={{ display: "flex", alignItems: "center", gap: 8, flex: 1 }}>
          <span style={{ color: "var(--ccc-text-muted)" }}>档案名</span>
          <input
            className="ccc-input"
            style={{ width: 220 }}
            value={name}
            placeholder="例如：游戏满血 / 静音办公"
            onChange={(e) => setName(e.target.value)}
          />
        </label>
        <ConfirmButton
          label="保存当前设置为档案"
          confirmLabel="确认保存？"
          onConfirm={() => {
            if (!name.trim()) {
              toast({ kind: "error", title: "请先输入档案名" });
              return;
            }
            save();
          }}
        />
      </div>

      {error && <div className="ccc-warnings">读取失败：{error}</div>}
      {profiles.length === 0 && !error && (
        <div className="ccc-warnings">还没有档案——先在「性能调校 / 风扇曲线」调好参数，再回来保存。</div>
      )}

      {profiles.length > 0 && (
        <div className="ccc-facts">
          {profiles.map((profile) => (
            <div key={profile.name} className="ccc-fact-row">
              <span className="ccc-fact-label">{profile.name}</span>
              <span className="ccc-fact-value" style={{ display: "flex", gap: 8 }}>
                <span style={{ color: "var(--ccc-text-muted)" }}>
                  {new Date(profile.savedAtMs).toLocaleString("zh-CN")}
                </span>
                <button type="button" className="ccc-button" onClick={() => apply(profile.name)}>
                  应用
                </button>
                <button
                  type="button"
                  className="ccc-button"
                  style={{ background: "var(--ccc-action-hover)" }}
                  onClick={() => {
                    exportProfile(profile.name).then((json) => {
                      const blob = new Blob([json], { type: "application/json" });
                      const url = URL.createObjectURL(blob);
                      const anchor = document.createElement("a");
                      anchor.href = url;
                      anchor.download = `${profile.name}.json`;
                      anchor.click();
                      URL.revokeObjectURL(url);
                      toast({ kind: "success", title: `已导出：${profile.name}.json` });
                    });
                  }}
                >
                  导出
                </button>
                <ConfirmButton
                  label="删除"
                  confirmLabel="确认删除？"
                  onConfirm={() => remove(profile.name)}
                />
              </span>
            </div>
          ))}
        </div>
      )}

      <div style={{ display: "flex", gap: 12, alignItems: "center" }}>
        <button type="button" className="ccc-button" onClick={() => fileInputRef.current?.click()}>
          导入档案（.json）
        </button>
        <input
          ref={fileInputRef}
          type="file"
          accept="application/json,.json"
          style={{ display: "none" }}
          onChange={(event) => {
            const file = event.target.files?.[0];
            event.target.value = "";
            if (!file) return;
            file
              .text()
              .then((json) => importProfile(json))
              .then((result) => {
                if (result.ok) {
                  toast({ kind: "success", title: result.message });
                  reload();
                } else {
                  toast({ kind: "error", title: "导入失败", description: result.message });
                }
              })
              .catch((cause: unknown) =>
                toast({
                  kind: "error",
                  title: "导入失败",
                  description: cause instanceof Error ? cause.message : String(cause),
                }),
              );
          }}
        />
        <span style={{ color: "var(--ccc-text-muted)", fontSize: "var(--ccc-text-sm)" }}>
          导入的档案与本地档案同等可用（重名覆盖）。
        </span>
      </div>

      {lastSteps && (
        <div className="ccc-facts">
          {lastSteps.map((step) => (
            <div key={step.step} className="ccc-fact-row">
              <span className="ccc-fact-label">{step.step}</span>
              <span className={step.ok ? "ccc-fact-value" : "ccc-fact-value is-danger"}>
                {step.ok ? "成功" : step.message}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
