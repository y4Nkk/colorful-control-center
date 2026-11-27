import { lazy, Suspense, type ReactNode } from "react";
import { openExternalUrl, toCommandError, type AppUpdate } from "../api/client";
import { ApplicationUpdateContext, useApplicationUpdate, useApplicationUpdateController } from "../app/useApplicationUpdate";
import { Button } from "./Button";
import { Dialog } from "./Dialog";
import { FactStatus } from "./FactList";
import { Loading } from "./Loading";
import { Progress } from "./Progress";
import { toast } from "./use-toast";

const ChangelogNotes = lazy(() => import("./settings/ChangelogNotes"));
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

export function ApplicationUpdateHost({ children }: { children: ReactNode }) {
  const update = useApplicationUpdateController();
  return <ApplicationUpdateContext value={update}>
    {children}
    <ApplicationUpdateDialog />
  </ApplicationUpdateContext>;
}

function ApplicationUpdateDialog() {
  const { candidate, state, canCheck, canCancel, download, open, close } = useApplicationUpdate();
  const release = state.phase === "available" ? candidate : null;
  return (
    <Dialog title={release ? "发现新版本" : "软件更新"} open={open} onClose={close} width="narrow"
      layout={release ? "confirmation" : undefined} dismissOnBackdrop={Boolean(release)} actions={release ? <>
        <Button variant="neutral" onClick={close}>取消</Button>
        <Button onClick={() => void download()}>更新</Button>
      </> : (canCheck || canCancel) && <UpdateStatusActions />}>
      {release ? <UpdateRelease candidate={release} /> : <UpdateStatus />}
    </Dialog>
  );
}

function UpdateRelease({ candidate }: { candidate: AppUpdate }) {
  const notes = candidate.notes?.trim();
  return <div className="ccc-update-release">
    <p className="ccc-update-version ccc-mono">v{candidate.version}</p>
    {notes ? <Suspense fallback={<Loading label="正在读取更新说明…" />}>
      <ChangelogNotes label="新版本更新说明" notes={notes} />
    </Suspense> : <p className="ccc-settings-detail">暂无更新说明。</p>}
  </div>;
}

function UpdateStatusActions() {
  const { candidate, state, canCancel, cancelling, recheck, download, cancel, close } = useApplicationUpdate();
  if (canCancel) return <Button disabled={cancelling} onClick={() => void cancel()}>取消下载</Button>;
  if (state.phase === "current") return <Button onClick={close}>确定</Button>;
  return <>
    {state.phase === "error" && <Button variant="ghost" onClick={openReleases}>GitHub 发布页</Button>}
    <Button variant={candidate ? "secondary" : "primary"} onClick={() => void recheck()}>重新检查</Button>
    {candidate && <Button onClick={() => void download()}>重新下载</Button>}
  </>;
}

function UpdateStatus() {
  const { version, candidate, state, canCancel, cancelling, cancelError } = useApplicationUpdate();
  const { phase } = state;
  const cancelPending = cancelling && canCancel;
  const status = cancelPending ? "正在取消下载…" : statusText[phase];
  const waiting = cancelPending || phase === "checking" || phase === "starting" || phase === "verifying" || phase === "launching";

  return <>
    {candidate ? <p className="ccc-settings-detail">新版本 <code className="ccc-mono">v{candidate.version}</code></p>
      : version.status === "ready" && phase !== "finished" &&
        <p className="ccc-settings-detail">当前版本 <code className="ccc-mono">v{version.version}</code></p>}
    {phase === "error" && <p className="ccc-settings-error" role="alert">{state.message}</p>}
    {cancelError && <p className="ccc-settings-error" role="alert">{cancelError}</p>}
    {status && <p className="ccc-settings-detail" role="status">
      {waiting ? <Loading embedded label={status} />
        : phase === "current" || phase === "finished" ? <FactStatus tone="success">{status}</FactStatus> : status}
    </p>}
    {phase === "downloading" && <Progress label="下载进度" max={state.totalBytes ?? undefined}
      value={state.totalBytes ? state.downloadedBytes : undefined} />}
  </>;
}
