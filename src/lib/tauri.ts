import { invoke as tauriInvoke } from "@tauri-apps/api/core";

/**
 * Typed wrapper around tauri.invoke.
 */
export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return tauriInvoke<T>(cmd, args);
}

export interface BuildInfo {
  version: string;
  build: "appstore" | "devid";
  privileged: boolean;
  sandboxed: boolean;
}
