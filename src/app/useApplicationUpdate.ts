import { createContext, useContext, useEffect, useRef, useState } from "react";
import { cancelApplicationUpdate, checkApplicationUpdate, downloadApplicationUpdate, getBackendInfo, toCommandError,
  type AppUpdate, type UpdateProgress } from "../api/client";

type UpdateState = UpdateProgress | { phase: "idle" | "checking" | "current" | "available" | "starting" | "cancelled" | "finished" }
  | { phase: "error"; message: string };

type ApplicationVersion = { status: "loading" } | { status: "ready"; version: string } | { status: "error"; message: string };
type UpdateEntry = "sidebar" | "about";

function useApplicationVersion() {
  const [version, setVersion] = useState<ApplicationVersion>({ status: "loading" });
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let current = true;
    void getBackendInfo().then(info => {
      if (current) setVersion({ status: "ready", version: info.version });
    }).catch(error => {
      if (current) setVersion({ status: "error", message: toCommandError(error).message });
    });
    return () => { current = false; };
  }, [attempt]);

  return { version, retryVersion: () => {
    setVersion({ status: "loading" });
    setAttempt(value => value + 1);
  } };
}

export function useApplicationUpdateController() {
  const [candidate, setCandidate] = useState<AppUpdate | null>(null);
  const [state, setState] = useState<UpdateState>({ phase: "idle" });
  const [cancelling, setCancelling] = useState(false);
  const [cancelError, setCancelError] = useState<string>();
  const [open, setOpen] = useState(false);
  const pending = useRef(false);
  const activeDownload = useRef<{ cancelling: boolean } | null>(null);
  const version = useApplicationVersion();
  const { phase } = state;
  const canCancel = phase === "downloading" || phase === "verifying";
  const busy = canCancel || phase === "checking" || phase === "starting" || phase === "launching";
  const canCheck = !busy && phase !== "installerOpen";
  const checkOnEntry = phase === "idle" || phase === "current";

  async function check(source: UpdateEntry | "dialog") {
    if (pending.current) return;
    pending.current = true;
    setCandidate(null);
    setCancelError(undefined);
    setState({ phase: "checking" });
    try {
      const update = await checkApplicationUpdate();
      setCandidate(update);
      setState({ phase: update ? "available" : "current" });
      if (source === "about" || (source === "sidebar" && update)) setOpen(true);
    } catch (error) {
      setState({ phase: "error", message: toCommandError(error).message });
      if (source === "about") setOpen(true);
    } finally { pending.current = false; }
  }

  function activate(source: UpdateEntry) {
    if (checkOnEntry) void check(source);
    else if (phase !== "checking") setOpen(true);
  }

  async function download() {
    if (!candidate || pending.current) return;
    pending.current = true;
    const task = { cancelling: false };
    activeDownload.current = task;
    setCancelError(undefined);
    if (phase === "available") setOpen(false);
    setState({ phase: "starting" });
    try {
      const installed = await downloadApplicationUpdate(candidate.id, progress => {
        if (activeDownload.current === task) setState(progress);
      });
      if (installed) setCandidate(null);
      setState({ phase: installed ? "finished" : "cancelled" });
    } catch (error) {
      setState({ phase: "error", message: toCommandError(error).message });
    } finally {
      activeDownload.current = null;
      pending.current = false;
      setCancelling(false);
    }
  }

  async function cancel() {
    const task = activeDownload.current;
    if (!candidate || !canCancel || !task || task.cancelling) return;
    task.cancelling = true;
    setCancelling(true);
    setCancelError(undefined);
    try {
      if (await cancelApplicationUpdate(candidate.id)) return;
    } catch (error) {
      if (activeDownload.current === task) setCancelError(toCommandError(error).message);
    }
    if (activeDownload.current === task) {
      task.cancelling = false;
      setCancelling(false);
    }
  }

  return {
    ...version, candidate, state, canCheck, checkOnEntry, canCancel, cancelling, cancelError,
    activate, recheck: () => check("dialog"), download, cancel, open, close: () => setOpen(false),
  };
}

export const ApplicationUpdateContext = createContext<ReturnType<typeof useApplicationUpdateController> | null>(null);

export function useApplicationUpdate() {
  const update = useContext(ApplicationUpdateContext);
  if (!update) throw new Error("ApplicationUpdateHost is missing");
  return update;
}
