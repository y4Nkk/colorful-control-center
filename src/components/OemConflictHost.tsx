import { createContext, useCallback, useContext, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { closeFnKey, findOemConflict, readFnKeyCloseTarget, type FnKeyIdentity } from "../api/client";
import { Button } from "./Button";
import { Dialog, WorkspaceActivity } from "./Dialog";
import { Icon } from "./Icon";
import { Loading } from "./Loading";
import { toast } from "./use-toast";

type Prompt = { conflict: string; onClosed?: () => void } | { mode: string; confirm: () => Promise<void> };
type Inspection = { checking?: boolean; target?: FnKeyIdentity; message?: string; failed?: boolean };
const ConflictContext = createContext<((prompt: Prompt) => void) | null>(null);

export const fnKeyConflict = (issues: Parameters<typeof findOemConflict>[0]) => findOemConflict(issues, "FnKey.exe");

export function useOemConflict() {
  const present = useContext(ConflictContext);
  const active = useContext(WorkspaceActivity);
  if (!present) throw new Error("原厂软件冲突确认未挂载");
  return {
    review: (issues: Parameters<typeof findOemConflict>[0], onClosed?: () => void) => {
      const conflict = active ? fnKeyConflict(issues) : undefined;
      if (conflict) present({ conflict, onClosed });
    },
    switchMode: (mode: string, confirm: () => Promise<void>) => {
      if (active) present({ mode, confirm });
    },
  };
}

export function OemConflictHost({ workspace, children }: { workspace: string; children: ReactNode }) {
  const scope = useMemo(() => Symbol(workspace), [workspace]);
  const activeScope = useRef(scope);
  const current = useRef<Prompt | null>(null);
  const closing = useRef(false);
  const [prompt, setPrompt] = useState<Prompt | null>(null);
  const [inspection, setInspection] = useState<Inspection>({});
  const [busy, setBusy] = useState(false);
  const dismiss = useCallback(() => {
    if (closing.current || !current.current) return;
    current.current = null;
    setPrompt(null);
  }, []);
  useLayoutEffect(() => {
    activeScope.current = scope;
    dismiss();
  }, [scope, dismiss]);

  const present = useCallback((next: Prompt) => {
    if (activeScope.current !== scope || current.current) return;
    current.current = next;
    const checking = "conflict" in next;
    setInspection({ checking });
    setPrompt(next);
    if (!checking) return;
    void readFnKeyCloseTarget().then(target => {
      if (current.current !== next) return;
      if (target) setInspection({ target });
      else {
        next.onClosed?.();
        setInspection({ message: "原厂 FnKey 已退出。请重新尝试刚才的操作。" });
      }
    }, error => {
      if (current.current === next) setInspection({ failed: true, message: String(error) });
    });
  }, [scope]);

  const confirm = async () => {
    if (!prompt || current.current !== prompt || closing.current) return;
    const { target } = inspection;
    const exit = "mode" in prompt ? prompt.confirm : target && (() => closeFnKey(target));
    if (!exit) return;
    closing.current = true;
    setBusy(true);
    try {
      await exit();
      if ("conflict" in prompt) {
        prompt.onClosed?.();
        toast({ kind: "success", title: "原厂 FnKey 已退出", description: "请重新尝试刚才的操作。" });
      }
    } catch (error) {
      setInspection({ failed: true, message: String(error) });
      return;
    } finally {
      closing.current = false;
      setBusy(false);
    }
    dismiss();
  };

  return <ConflictContext.Provider value={present}>
    {children}
    <OemConflictDialog prompt={prompt} inspection={inspection} busy={busy} onClose={dismiss} onConfirm={confirm} />
  </ConflictContext.Provider>;
}

function OemConflictDialog({ prompt, inspection, busy, onClose, onConfirm }: {
  prompt: Prompt | null; inspection: Inspection; busy: boolean; onClose: () => void; onConfirm: () => Promise<void>;
}) {
  const mode = prompt && "mode" in prompt ? prompt.mode : null;
  const conflict = prompt && "conflict" in prompt ? prompt.conflict : null;
  const ready = !!mode || !!inspection.target;
  const actionLabel = mode ? "关闭并切换" : "关闭 FnKey";
  return <Dialog title="关闭原厂 FnKey" width="narrow" open={prompt !== null} busy={busy} onClose={onClose}
    actions={<><Button variant="secondary" disabled={busy} onClick={onClose}>{ready || inspection.checking ? "取消" : "知道了"}</Button>
      {ready && <Button busy={busy} onClick={() => void onConfirm()}>
        {busy ? `正在${actionLabel}…` : actionLabel}
      </Button>}</>}>
    {mode && <p>关闭原厂 FnKey 后，控制台会切换到「{mode}」。</p>}
    {conflict && !inspection.message && <p>{conflict}</p>}
    {inspection.checking && <Loading label="正在核对原厂进程…" />}
    {inspection.message && <p role={inspection.failed ? "alert" : "status"} className={inspection.failed ? "ccc-control-error" : undefined}>{inspection.failed && <Icon name="error" size={16} />}{inspection.message}</p>}
    {ready && <p>部分原厂 Fn 热键和屏显会暂停。要恢复，请在设置中关闭「自动关闭原厂软件」，再从开始菜单打开「Fn hotkeys and OSD」。</p>}
  </Dialog>;
}
