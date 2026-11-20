import { invoke } from "./client";

export interface FnKeyIdentity { pid: number; creationTime: [number, number] }
export interface OemConflict { message: string; executable: string }

export const readFnKeyCloseTarget = (): Promise<FnKeyIdentity | null> => invoke("read_fn_key_close_target");
export const closeFnKey = (expected: FnKeyIdentity): Promise<void> => invoke("close_fn_key", { expected });

const OWNER_DIAGNOSTIC = /原厂 ([\w.-]+\.exe) 正在运行，可能同时修改硬件设置。请退出后重试。/;

export function findOemConflict(issues: readonly (string | null | undefined)[]): OemConflict | undefined {
  // The owner diagnostic can be nested in a domain status or a transaction report.
  for (const issue of issues) {
    const match = issue?.match(OWNER_DIAGNOSTIC);
    if (match) return { message: match[0], executable: match[1] };
  }
}
