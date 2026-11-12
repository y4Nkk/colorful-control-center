/**
 * 设置（Phase 4）：主题覆盖、开机自启、关于信息。
 * 主题经 lib/theme.ts 唯一入口；自启走注册表命令；关闭窗口 = 隐藏到托盘，
 * 真正退出在托盘菜单。
 */
import { useEffect, useState } from "react";
import { getAutostart, getBackendInfo, setAutostart, type BackendInfo } from "../api/client";
import { FactList } from "../components/FactList";
import { getThemePreference, setThemePreference, type ThemePreference } from "../lib/theme";
import { toast } from "../components/use-toast";

const THEME_OPTIONS: ReadonlyArray<{ id: ThemePreference; label: string }> = [
  { id: "system", label: "跟随系统" },
  { id: "light", label: "浅色" },
  { id: "dark", label: "深色" },
];

export function SettingsWorkspace({ active }: { active: boolean }) {
  const [theme, setTheme] = useState<ThemePreference>(() => getThemePreference());
  const [autostart, setAutostartState] = useState<boolean | null>(null);
  const [info, setInfo] = useState<BackendInfo | null>(null);

  useEffect(() => {
    if (!active) return;
    getAutostart()
      .then(setAutostartState)
      .catch((cause: unknown) =>
        toast({ kind: "error", title: "读取自启状态失败", description: cause instanceof Error ? cause.message : String(cause) }),
      );
    getBackendInfo().then(setInfo).catch(() => {});
  }, [active]);

  const changeTheme = (preference: ThemePreference) => {
    setTheme(preference);
    setThemePreference(preference);
  };

  const toggleAutostart = () => {
    const target = !autostart;
    setAutostart(target).then((result: { ok: boolean; message: string }) => {
      if (result.ok) {
        setAutostart(target);
        toast({ kind: "success", title: result.message });
      } else {
        toast({ kind: "error", title: "设置失败", description: result.message });
      }
    });
  };

  return (
    <div className="ccc-dashboard">
      <h3 className="ccc-group-title">外观</h3>
      <div className="ccc-facts" style={{ display: "flex", gap: 8, alignItems: "center" }}>
        <span style={{ color: "var(--ccc-text-muted)" }}>主题：</span>
        {THEME_OPTIONS.map((option) => (
          <button
            key={option.id}
            type="button"
            className={theme === option.id ? "ccc-button" : "ccc-nav-item"}
            style={theme === option.id ? undefined : { width: "auto", height: 32 }}
            onClick={() => changeTheme(option.id)}
          >
            {option.label}
          </button>
        ))}
      </div>

      <h3 className="ccc-group-title">启动与常驻</h3>
      <div className="ccc-facts" style={{ display: "flex", gap: 12, alignItems: "center" }}>
        <button type="button" className="ccc-button" onClick={toggleAutostart}>
          {autostart ? "关闭开机自启" : "开启开机自启"}
        </button>
        <span style={{ color: "var(--ccc-text-muted)", fontSize: "var(--ccc-text-sm)" }}>
          状态：{autostart === null ? "读取中…" : autostart ? "已开启（注册表 HKCU Run）" : "未开启"}
        </span>
      </div>
      <div className="ccc-warnings">
        关闭窗口会隐藏到托盘（保持后台语义）；完全退出请使用托盘菜单「退出」。
      </div>

      <h3 className="ccc-group-title">关于</h3>
      <FactList
        facts={[
          { label: "后端", value: info ? `${info.name} v${info.version}` : "…" },
          { label: "硬件通道", value: "InsydeDCHU（OEM AcpiBridge 驱动）+ NVML + PawnIO(MSR)" },
          { label: "非官方声明", value: "社区作品，与七彩虹官方无关；写入有风险，使用自负" },
        ]}
      />
    </div>
  );
}
