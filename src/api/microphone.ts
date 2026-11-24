import { invoke } from "./client";

export interface MicrophoneEndpoint { id: string; name: string | null; muted: boolean }
export interface MicrophoneSnapshot { endpoints: MicrophoneEndpoint[]; saved: number }
export interface MicrophoneTarget { endpoints: MicrophoneEndpoint[]; savedMuted: boolean }
export interface MicrophoneProfile { revision: string; current: MicrophoneSnapshot; blockedReason: string | null }
export interface MicrophoneRequest { expectedRevision: string; muted: boolean }
export const readMicrophones = (): Promise<MicrophoneProfile> => invoke("read_microphones");
export const applyMicrophones = (request: MicrophoneRequest): Promise<void> => invoke("apply_microphones", { request });
