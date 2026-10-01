import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  BenchmarkSnapshot,
  FixPreviewResponse,
  OptimizationReport,
  ReviewReport,
  TransactionRecord,
  VerificationResult,
  WorkspaceResponse
} from "./types";

export const isTauri = () => Boolean(window.__TAURI_INTERNALS__);

async function command<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) {
    throw new Error("Desktop commands are available when the app runs inside Tauri.");
  }
  return invoke<T>(name, args);
}

export async function chooseWorkspace(): Promise<string | null> {
  if (!isTauri()) return null;
  const selected = await open({ directory: true, multiple: false, title: "Open repository" });
  return typeof selected === "string" ? selected : null;
}

export const api = {
  openWorkspace: (path: string) => command<WorkspaceResponse>("open_workspace", { path }),
  getWorkspace: () => command<WorkspaceResponse>("get_workspace"),
  runReview: (changedOnly: boolean, includeExternal: boolean) =>
    command<ReviewReport>("run_review", { changedOnly, includeExternal }),
  previewFix: (diagnosticId: string, fixIndex = 0) =>
    command<FixPreviewResponse>("preview_fix", { diagnosticId, fixIndex }),
  applyFix: (previewId: string, force = false) =>
    command<TransactionRecord>("apply_fix", { previewId, force }),
  undoFix: (transactionId: string) => command<void>("undo_fix", { transactionId }),
  listHistory: () => command<TransactionRecord[]>("list_history"),
  verify: (full = true) => command<VerificationResult>("run_verification", { full }),
  optimize: () => command<OptimizationReport>("run_optimization"),
  benchmark: (samples = 5) => command<BenchmarkSnapshot>("run_benchmark", { samples })
};
