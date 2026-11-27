import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { readAdministratorAccess, restartAsAdministrator, toCommandError, type AdministratorAccess } from "../api/client";
import { Button } from "./Button";
import { Dialog } from "./Dialog";
import { toast } from "./use-toast";

export type AccessState = AdministratorAccess | "loading" | "unavailable";
type ReportFailure = (message: string) => void;
interface AdministratorModel {
  access: AccessState;
  pending: boolean;
  request: (report?: ReportFailure) => void;
}

export const AdministratorContext = createContext<AdministratorModel | null>(null);

export function useAdministrator() {
  const administrator = useContext(AdministratorContext);
  if (!administrator) throw new Error("AdministratorHost is missing");
  return administrator;
}

function reportRestartFailure(description: string) {
  toast({ kind: "error", title: "以管理员运行失败", description });
}

export function AdministratorHost({ children }: { children: ReactNode }) {
  const [access, setAccess] = useState<AccessState>("loading");
  const [prompt, setPrompt] = useState<{ report: ReportFailure } | null>(null);
  const [pending, setPending] = useState(false);
  const restarting = useRef(false);
  useEffect(() => {
    let active = true;
    void readAdministratorAccess().then(value => { if (active) setAccess(value); }).catch(error => {
      if (!active) return;
      setAccess("unavailable");
      toast({ kind: "error", title: "无法读取管理员权限", description: toCommandError(error).message });
    });
    return () => { active = false; };
  }, []);

  const request = useCallback((report = reportRestartFailure) => {
    if (access !== "canElevate" || restarting.current) return;
    setPrompt(current => current ?? { report });
  }, [access]);
  const administrator = useMemo(() => ({ access, pending, request }), [access, pending, request]);
  const close = () => setPrompt(null);
  const confirm = async () => {
    if (!prompt || access !== "canElevate" || restarting.current) return;
    restarting.current = true;
    setPending(true);
    close();
    try { await restartAsAdministrator(); }
    catch (error) {
      restarting.current = false;
      setPending(false);
      prompt.report(toCommandError(error).message);
    }
  };

  return <AdministratorContext value={administrator}>
    {children}
    <Dialog title="以管理员运行" width="narrow" open={prompt !== null} onClose={close}
      actions={<>
        <Button variant="secondary" onClick={close}>取消</Button>
        <Button icon="confirm" onClick={() => void confirm()}>请求 Windows 授权</Button>
      </>}>
      <p>请使用当前 Windows 账户授权。授权成功后，应用会安全结束当前会话，并以管理员身份重新打开。</p>
    </Dialog>
  </AdministratorContext>;
}
