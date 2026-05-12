import { invoke } from "@tauri-apps/api/core";

export async function invokeCommand<TResponse>(command: string, payload?: Record<string, unknown>) {
  return invoke<TResponse>(command, payload);
}
