import { invoke, listenEvent, type AppPreferences } from "./client";

export type Appearance = Pick<AppPreferences, "theme" | "interfaceFont" | "interfaceScale" | "motion">;
export interface OsdMessage {
  title: string;
  detail: string;
  error: boolean;
  level: { value: number; max: number } | null;
}
export interface OsdSnapshot {
  sequence: number;
  revision: number;
  message: OsdMessage | null;
  visible: boolean;
  error: string | null;
  appearance: Appearance | null;
  appearanceError: string | null;
}
export const readOsd = (): Promise<OsdSnapshot> => invoke("read_osd");
export const previewOsd = (): Promise<void> => invoke("preview_osd");
export const onOsd = (handler: (snapshot: OsdSnapshot) => void): Promise<() => void> => listenEvent("osd-state", handler);
