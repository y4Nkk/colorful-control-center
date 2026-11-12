/*
 * 全局通知单例 store（模式：模块级 memoryState + listeners + dispatch，
 * 命令式 toast() 供任意模块直接调用，useToast() 供渲染方订阅；Toaster
 * 在 app 层唯一挂载。禁止模块自建浮层——DESIGN.md 组件所有权）。
 */
import * as React from "react";

export type ToastKind = "info" | "success" | "warning" | "error";

export interface Toast {
  id: string;
  title: string;
  description?: string;
  kind: ToastKind;
  /** 自动关闭时间（毫秒）；不传按 kind 默认值，0 表示不自动关闭。 */
  durationMs?: number;
}

const TOAST_LIMIT = 2;
const DEFAULT_TOAST_REMOVE_DELAY: Record<ToastKind, number> = {
  info: 4500,
  success: 3200,
  warning: 6000,
  error: 10000,
};

let count = 0;

function genId() {
  count = (count + 1) % Number.MAX_SAFE_INTEGER;
  return count.toString();
}

type Action =
  | { type: "ADD_TOAST"; toast: Toast }
  | { type: "REMOVE_TOAST"; toastId: string };

interface State {
  toasts: Toast[];
}

const listeners: Array<(state: State) => void> = [];
const toastTimers = new Map<string, ReturnType<typeof setTimeout>>();
let memoryState: State = { toasts: [] };

function clearToastTimer(toastId: string): void {
  const timer = toastTimers.get(toastId);
  if (timer !== undefined) clearTimeout(timer);
  toastTimers.delete(toastId);
}

function dismissToast(toastId: string): void {
  if (!memoryState.toasts.some((toast) => toast.id === toastId)) return;
  clearToastTimer(toastId);
  dispatch({ type: "REMOVE_TOAST", toastId });
}

function reducer(state: State, action: Action): State {
  switch (action.type) {
    case "ADD_TOAST": {
      // 错误置顶，超出上限时挤掉最旧的普通通知
      const toasts = [action.toast, ...state.toasts]
        .sort((left, right) => Number(right.kind === "error") - Number(left.kind === "error"))
        .slice(0, TOAST_LIMIT);
      return { toasts };
    }
    case "REMOVE_TOAST":
      return { toasts: state.toasts.filter((toast) => toast.id !== action.toastId) };
  }
}

function dispatch(action: Action) {
  memoryState = reducer(memoryState, action);
  listeners.forEach((listener) => listener(memoryState));
}

export interface ToastOptions {
  title: string;
  description?: string;
  kind: ToastKind;
  durationMs?: number;
}

/** 任意模块可直接调用的命令式入口。 */
function toast(props: ToastOptions) {
  if (!props.title.trim()) throw new TypeError("Toast requires a title");
  if (props.durationMs !== undefined && (!Number.isFinite(props.durationMs) || props.durationMs < 0)) {
    throw new RangeError("durationMs must be a finite non-negative number");
  }

  const id = genId();
  dispatch({ type: "ADD_TOAST", toast: { ...props, id } });
  const delay = props.durationMs ?? DEFAULT_TOAST_REMOVE_DELAY[props.kind];
  if (delay > 0) {
    toastTimers.set(
      id,
      setTimeout(() => {
        toastTimers.delete(id);
        dismissToast(id);
      }, delay),
    );
  }
  return () => dismissToast(id);
}

function useToast() {
  const [state, setState] = React.useState<State>(memoryState);

  React.useEffect(() => {
    listeners.push(setState);
    return () => {
      const index = listeners.indexOf(setState);
      if (index > -1) listeners.splice(index, 1);
    };
  }, []);

  return { ...state, toast, dismiss: dismissToast };
}

/** 测试隔离：模块级 store 生命周期超出单次渲染。 */
function clearToasts(): void {
  for (const timer of toastTimers.values()) clearTimeout(timer);
  toastTimers.clear();
  memoryState = { toasts: [] };
  listeners.forEach((listener) => listener(memoryState));
}

export { useToast, toast, clearToasts };
