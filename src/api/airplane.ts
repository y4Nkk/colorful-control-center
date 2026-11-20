import { invoke } from "./client";
export interface AirplaneSnapshot { device: string; enabled: boolean }
export interface AirplaneProfile {
  revision: string | null; identity: string | null; current: AirplaneSnapshot | null; error: string | null;
  blockedReason: string | null;
  initial: AirplaneSnapshot | null; restoreRevision: string | null; restoreError: string | null;
}
export const readAirplane = (): Promise<AirplaneProfile> => invoke("read_airplane");
export const applyAirplane = (request: { expectedRevision: string; enabled: boolean }): Promise<void> => invoke("apply_airplane", { request });
export const restoreAirplane = (expectedRevision: string): Promise<void> => invoke("restore_airplane", { expectedRevision });
