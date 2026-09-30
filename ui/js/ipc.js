/**
 * TruthBeacon IPC Bridge
 * Communicates with Tauri Rust backend or falls back to local reactive mock state
 */

export const isTauriEnvironment = () => {
  return typeof window !== "undefined" && window.__TAURI_INTERNALS__ !== undefined;
};

export async function invokeCommand(cmd, args = {}) {
  if (isTauriEnvironment() && window.__TAURI__?.core?.invoke) {
    try {
      return await window.__TAURI__.core.invoke(cmd, args);
    } catch (err) {
      console.warn(`[TruthBeacon IPC] Failed invoking ${cmd}:`, err);
      throw err;
    }
  }

  // Graceful browser fallback for UI preview and dev scaffolding
  console.info(`[TruthBeacon IPC] Mock fallback for command: ${cmd}`, args);
  return null;
}
