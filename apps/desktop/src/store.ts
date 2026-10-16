import { create } from "zustand";
import { api, chooseFleetConfig, chooseWorkspace } from "./api";
import type {
  BenchmarkSnapshot,
  Diagnostic,
  EngineStatus,
  FleetRunSummary,
  FixPreviewResponse,
  OptimizationReport,
  TransactionRecord,
  VerificationResult,
  Theme,
  View,
  WorkspaceSummary
} from "./types";

interface AppStore {
  view: View;
  theme: Theme;
  workspace?: WorkspaceSummary;
  engines: EngineStatus[];
  diagnostics: Diagnostic[];
  selectedDiagnosticId?: string;
  preview?: FixPreviewResponse;
  verification?: VerificationResult;
  optimization?: OptimizationReport;
  benchmark?: BenchmarkSnapshot;
  fleet?: FleetRunSummary;
  fleetConfigPath?: string;
  history: TransactionRecord[];
  loading: boolean;
  error?: string;
  setView: (view: View) => void;
  setTheme: (theme: Theme) => void;
  openRepository: () => Promise<void>;
  refreshWorkspace: () => Promise<void>;
  runReview: (changedOnly?: boolean, includeExternal?: boolean) => Promise<void>;
  selectDiagnostic: (id: string) => void;
  previewFix: (diagnosticId: string, fixIndex?: number) => Promise<void>;
  applyPreview: () => Promise<void>;
  undoTransaction: (id: string) => Promise<void>;
  runVerification: () => Promise<void>;
  runOptimization: () => Promise<void>;
  runBenchmark: () => Promise<void>;
  chooseFleetConfig: () => Promise<void>;
  runFleet: (command: string) => Promise<void>;
  saveDisposition: (disposition: string, reason?: string) => Promise<void>;
  clearError: () => void;
}

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export const useAppStore = create<AppStore>((set, get) => ({
  view: "workspace",
  theme: "dark",
  engines: [],
  diagnostics: [],
  history: [],
  loading: false,
  setView: (view) => set({ view }),
  setTheme: (theme) => set({ theme }),
  clearError: () => set({ error: undefined }),

  openRepository: async () => {
    const path = await chooseWorkspace();
    if (!path) return;
    set({ loading: true, error: undefined });
    try {
      const response = await api.openWorkspace(path);
      set({ workspace: response.summary, engines: response.engines, diagnostics: [], preview: undefined, view: "workspace" });
    } catch (error) {
      set({ error: message(error) });
    } finally {
      set({ loading: false });
    }
  },

  refreshWorkspace: async () => {
    if (!get().workspace) return;
    try {
      const response = await api.getWorkspace();
      set({ workspace: response.summary, engines: response.engines });
    } catch (error) {
      set({ error: message(error) });
    }
  },

  runReview: async (changedOnly = false, includeExternal = false) => {
    if (!get().workspace) return;
    set({ loading: true, error: undefined });
    try {
      const report = await api.runReview(changedOnly, includeExternal);
      set({
        diagnostics: report.diagnostics,
        selectedDiagnosticId: report.diagnostics[0]?.id,
        preview: undefined,
        view: "review"
      });
    } catch (error) {
      set({ error: message(error) });
    } finally {
      set({ loading: false });
    }
  },

  selectDiagnostic: (id) => set({ selectedDiagnosticId: id, preview: undefined }),

  previewFix: async (diagnosticId, fixIndex = 0) => {
    set({ loading: true, error: undefined });
    try {
      const preview = await api.previewFix(diagnosticId, fixIndex);
      set({ preview });
    } catch (error) {
      set({ error: message(error) });
    } finally {
      set({ loading: false });
    }
  },

  applyPreview: async () => {
    const preview = get().preview;
    if (!preview) return;
    set({ loading: true, error: undefined });
    try {
      const transaction = await api.applyFix(preview.preview_id, false);
      set((state) => ({ history: [transaction, ...state.history], preview: undefined }));
      await get().runReview();
    } catch (error) {
      set({ error: message(error) });
    } finally {
      set({ loading: false });
    }
  },

  undoTransaction: async (id) => {
    set({ loading: true, error: undefined });
    try {
      await api.undoFix(id);
      set((state) => ({ history: state.history.filter((item) => item.id !== id) }));
      await get().runReview();
    } catch (error) {
      set({ error: message(error) });
    } finally {
      set({ loading: false });
    }
  },

  runVerification: async () => {
    if (!get().workspace) return;
    set({ loading: true, error: undefined });
    try {
      set({ verification: await api.verify(true) });
    } catch (error) {
      set({ error: message(error) });
    } finally {
      set({ loading: false });
    }
  },

  runOptimization: async () => {
    if (!get().workspace) return;
    set({ loading: true, error: undefined, view: "optimize" });
    try {
      set({ optimization: await api.optimize() });
    } catch (error) {
      set({ error: message(error) });
    } finally {
      set({ loading: false });
    }
  },

  runBenchmark: async () => {
    if (!get().workspace) return;
    set({ loading: true, error: undefined });
    try {
      set({ benchmark: await api.benchmark(5) });
    } catch (error) {
      set({ error: message(error) });
    } finally {
      set({ loading: false });
    }
  },

  chooseFleetConfig: async () => {
    const path = await chooseFleetConfig();
    if (path) set({ fleetConfigPath: path, error: undefined });
  },

  runFleet: async (command) => {
    const configPath = get().fleetConfigPath;
    if (!configPath) {
      set({ error: "Choose a fleet.toml before running a fleet operation." });
      return;
    }
    set({ loading: true, error: undefined, view: "fleet" });
    try {
      set({ fleet: await api.runFleet(command, configPath) });
    } catch (error) {
      set({ error: message(error) });
    } finally {
      set({ loading: false });
    }
  },

  saveDisposition: async (disposition, reason) => {
    const diagnostic = get().diagnostics.find((item) => item.id === get().selectedDiagnosticId);
    if (!diagnostic) return;
    set({ loading: true, error: undefined });
    try {
      await api.saveDisposition(diagnostic, disposition, reason);
    } catch (error) {
      set({ error: message(error) });
    } finally {
      set({ loading: false });
    }
  }
}));
