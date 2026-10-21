import { Channel, invoke } from "@tauri-apps/api/core";
import { computed, reactive, readonly } from "vue";

type Phase = "idle" | "checking" | "current" | "available" | "downloading" | "installing" | "error";
interface Metadata { version: string; notes: string | null }
interface Progress { phase: "downloading" | "installing"; downloaded: number; total: number | null; attempt: number }
// Shared across settings tabs so navigating away does not lose an active download.
const state = reactive({ phase: "idle" as Phase, update: null as Metadata | null,
  downloaded: 0, total: null as number | null, attempt: 1,
  errorCode: "", errorDetail: "", checkedAt: null as Date | null });
const busy = computed(() => ["checking", "downloading", "installing"].includes(state.phase));
const percent = computed(() => state.total ? Math.min(100, Math.floor(state.downloaded / state.total * 100)) : null);

function fail(error: unknown) {
  const native = error as { code?: string; detail?: string } | null;
  state.errorCode = native?.code || "unknown";
  state.errorDetail = native?.detail || (error instanceof Error ? error.message : String(error));
  state.phase = "error";
}

async function check() {
  if (busy.value) return;
  state.phase = "checking";
  state.update = null;
  state.errorCode = state.errorDetail = "";
  try {
    state.update = await invoke<Metadata | null>("app_update_check");
    state.checkedAt = new Date();
    state.phase = state.update ? "available" : "current";
  } catch (error) { fail(error); }
}

async function install() {
  if (busy.value || !state.update) return;
  state.phase = "downloading";
  state.downloaded = 0;
  state.total = null;
  state.attempt = 1;
  state.errorCode = state.errorDetail = "";
  const progress = new Channel<Progress>();
  let receiving = true;
  progress.onmessage = (event) => {
    if (!receiving) return;
    state.phase = event.phase;
    state.downloaded = event.downloaded;
    state.total = event.total;
    state.attempt = event.attempt;
  };
  try { await invoke("app_update_install", { progress }); }
  catch (error) { fail(error); }
  finally { receiving = false; }
}

export function useAppUpdate() {
  return { state: readonly(state), busy, percent, check, install };
}
