/**
 * 应用根（composition root）：主题初始化 + AppShell 外框 + 工作区 +
 * 全局通知 Host。页面状态在根持有（持久化于 Phase 4 设置接入）。
 */
import { useEffect, useState } from "react";
import { AppShell } from "./app/AppShell";
import { AppWorkspace } from "./app/AppWorkspace";
import type { PageId } from "./app/navigation";
import { Toaster } from "./components/Toaster";
import { initializeTheme } from "./lib/theme";

export default function App() {
  const [page, setPage] = useState<PageId>("dashboard");
  useEffect(() => initializeTheme(), []);

  return (
    <>
      <AppShell page={page} onPageChange={setPage}>
        <AppWorkspace page={page} />
      </AppShell>
      <Toaster />
    </>
  );
}
