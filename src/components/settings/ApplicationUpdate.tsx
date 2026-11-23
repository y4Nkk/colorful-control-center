import { lazy, Suspense, useState } from "react";
import { openExternalUrl, toCommandError } from "../../api/client";
import { Button } from "../Button";
import { Dialog } from "../Dialog";
import { toast } from "../use-toast";
import { useApplicationUpdate } from "./useApplicationUpdate";

const ChangelogNotes = lazy(() => import("./ChangelogNotes"));
const releases = "https://github.com/y4Nkk/colorful-control-center/releases";

const statusText = {
  idle: "", checking: "正在检查更新…", current: "当前已是最新版本。", available: "", error: "",
  starting: "正在开始下载…", downloading: "正在下载安装包…", verifying: "正在校验安装包签名…",
  launching: "正在打开 Windows 安装向导…", installerOpen: "请在安装向导中确认升级。",
  cancelled: "更新已取消。", finished: "安装向导已完成。",
};

function openReleases() {
  void openExternalUrl(releases).catch(error => {
    toast({ kind: "error", title: "无法打开发布页", description: toCommandError(error).message });
  });
}

export function ApplicationUpdate({ currentVersion }: { currentVersion?: string }) {
  const [open, setOpen] = useState(false);
  const { candidate, state, cancelling, cancelError, check, download, cancel } = useApplicationUpdate();
  const { phase } = state;
  const canCancel = phase === "downloading" || phase === "verifying";
  const busy = canCancel || phase === "checking" || phase === "starting" || phase === "launching";
  const running = busy || phase === "installerOpen";
  const status = cancelling && canCancel ? "正在取消下载…" : statusText[phase];

  return <>
    <Button icon="refresh" variant="ghost" size="compact" aria-haspopup="dialog" onClick={() => {
      setOpen(true);
      if (!running) void check();
    }}>{phase === "installerOpen" ? "查看更新状态" : "检查更新"}</Button>
    <Dialog title="软件更新" open={open} busy={busy} onClose={() => setOpen(false)} actions={<>
      {!running && <Button variant="ghost" onClick={openReleases}>GitHub 发布页</Button>}
      {!running && phase === "error" && <Button variant="secondary" onClick={() => void check()}>重新检查</Button>}
      {canCancel && <Button disabled={cancelling} onClick={() => void cancel()}>取消下载</Button>}
      {!running && candidate && <Button onClick={() => void download()}>
        {phase === "error" ? "重新下载" : "下载并打开安装向导"}
      </Button>}
    </>}>
      <div className="ccc-update-content">
        {currentVersion && <p className="ccc-settings-detail">当前版本 v{currentVersion}</p>}
        {candidate && <p className="ccc-settings-label">新版本 v{candidate.version}</p>}
        {phase === "error" && <p className="ccc-settings-error" role="alert">{state.message}</p>}
        {cancelError && <p className="ccc-settings-error" role="alert">{cancelError}</p>}
        {status && <p className="ccc-settings-detail" role="status">{status}</p>}
        {phase === "downloading" && <progress className="ccc-update-progress" aria-label="下载进度"
          max={state.totalBytes ?? undefined} value={state.totalBytes ? state.downloadedBytes : undefined} />}
        {candidate?.notes && <Suspense fallback={<p role="status">正在读取更新说明…</p>}>
          <ChangelogNotes label="新版本更新说明" notes={candidate.notes} />
        </Suspense>}
      </div>
    </Dialog>
  </>;
}
