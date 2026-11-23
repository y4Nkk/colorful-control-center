import { useRef, useState } from "react";
import { cancelApplicationUpdate, checkApplicationUpdate, downloadApplicationUpdate, toCommandError,
  type AppUpdate, type UpdateProgress } from "../../api/client";

type UpdateState = UpdateProgress | { phase: "idle" | "checking" | "current" | "available" | "starting" | "cancelled" | "finished" }
  | { phase: "error"; message: string };

export function useApplicationUpdate() {
  const [candidate, setCandidate] = useState<AppUpdate | null>(null);
  const [state, setState] = useState<UpdateState>({ phase: "idle" });
  const [cancelling, setCancelling] = useState(false);
  const [cancelError, setCancelError] = useState<string>();
  const pending = useRef(false);

  async function check() {
    if (pending.current) return;
    pending.current = true;
    setCandidate(null);
    setCancelError(undefined);
    setState({ phase: "checking" });
    try {
      const update = await checkApplicationUpdate();
      setCandidate(update);
      setState({ phase: update ? "available" : "current" });
    } catch (error) {
      setState({ phase: "error", message: toCommandError(error).message });
    } finally { pending.current = false; }
  }

  async function download() {
    if (!candidate || pending.current) return;
    pending.current = true;
    setCancelError(undefined);
    setState({ phase: "starting" });
    let receivingProgress = true;
    try {
      const installed = await downloadApplicationUpdate(candidate.id, progress => {
        if (receivingProgress) setState(progress);
      });
      if (installed) setCandidate(null);
      setState({ phase: installed ? "finished" : "cancelled" });
    } catch (error) {
      setState({ phase: "error", message: toCommandError(error).message });
    } finally {
      receivingProgress = false;
      pending.current = false;
      setCancelling(false);
    }
  }

  async function cancel() {
    if (!candidate || cancelling) return;
    setCancelling(true);
    setCancelError(undefined);
    try {
      if (!await cancelApplicationUpdate(candidate.id)) setCancelling(false);
    } catch (error) {
      setCancelError(toCommandError(error).message);
      setCancelling(false);
    }
  }

  return { candidate, state, cancelling, cancelError, check, download, cancel };
}
