/**
 * 工作区挂载唯一所有者：keep-alive 模式——已访问页面保持挂载，hidden 只
 * 切换可见性，保住草稿/滚动/展开状态；active prop 控制轮询等副作用启停。
 * 新页面接入：PAGES 注册 + 此处新增分支（AGENTS.md 接入契约）。
 */
import { useEffect, useState } from "react";
import type { PageId } from "./navigation";
import { DashboardWorkspace } from "./DashboardWorkspace";
import { FansWorkspace } from "./FansWorkspace";
import { PerformanceWorkspace } from "./PerformanceWorkspace";
import { ProfilesWorkspace } from "./ProfilesWorkspace";
import { SettingsWorkspace } from "./SettingsWorkspace";

export function AppWorkspace({ page }: { page: PageId }) {
  const [opened, setOpened] = useState<ReadonlySet<PageId>>(() => new Set([page]));
  useEffect(() => {
    setOpened((current) => (current.has(page) ? current : new Set([...current, page])));
  }, [page]);
  const visited = (target: PageId) => page === target || opened.has(target);

  return (
    <>
      {visited("dashboard") && (
        <div hidden={page !== "dashboard"} className="ccc-page">
          <DashboardWorkspace active={page === "dashboard"} />
        </div>
      )}
      {visited("fans") && (
        <div hidden={page !== "fans"} className="ccc-page">
          <FansWorkspace active={page === "fans"} />
        </div>
      )}
      {visited("performance") && (
        <div hidden={page !== "performance"} className="ccc-page">
          <PerformanceWorkspace active={page === "performance"} />
        </div>
      )}
      {visited("profiles") && (
        <div hidden={page !== "profiles"} className="ccc-page">
          <ProfilesWorkspace active={page === "profiles"} />
        </div>
      )}
      {visited("settings") && (
        <div hidden={page !== "settings"} className="ccc-page">
          <SettingsWorkspace active={page === "settings"} />
        </div>
      )}
    </>
  );
}
