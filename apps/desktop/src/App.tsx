import { useEffect, useState } from "react";
import {
  Activity,
  Boxes,
  Braces,
  ChevronRight,
  CircleGauge,
  FileSearch,
  FolderOpen,
  GitBranch,
  History,
  Languages,
  ListChecks,
  Network,
  RefreshCw,
  RotateCcw,
  SearchCode,
  Settings,
  Shield,
  Sparkles,
  Wrench
} from "lucide-react";
import { DiffPreview } from "./components/DiffPreview";
import { ProblemList } from "./components/ProblemList";
import { VerificationPill } from "./components/StatusPill";
import { useAppStore } from "./store";
import type {
  Diagnostic,
  FleetRunSummary,
  RepoRunSummary,
  Theme,
  OptimizationReport,
  VerificationResult,
  View,
  WorkspaceSummary
} from "./types";

const navItems: Array<{ id: View; label: string; icon: typeof Boxes }> = [
  { id: "workspace", label: "Workspace", icon: Boxes },
  { id: "fleet", label: "Fleet", icon: Network },
  { id: "review", label: "Review", icon: FileSearch },
  { id: "refactor", label: "Refactor", icon: Wrench },
  { id: "optimize", label: "Optimize", icon: Sparkles },
  { id: "rules", label: "Rules", icon: ListChecks },
  { id: "engines", label: "Engines", icon: Activity },
  { id: "settings", label: "Settings", icon: Settings }
];

export default function App() {
  const store = useAppStore();
  const selected = store.diagnostics.find((diagnostic) => diagnostic.id === store.selectedDiagnosticId);

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: light)");
    const apply = () => {
      const resolved: Exclude<Theme, "system"> = store.theme === "system" ? (media.matches ? "light" : "dark") : store.theme;
      document.documentElement.dataset.theme = resolved;
    };
    apply();
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [store.theme]);

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark"><Braces size={21} /></div>
          <div><strong>CodeForge</strong><span>ENGINE</span></div>
        </div>
        <nav>
          {navItems.map((item) => {
            const Icon = item.icon;
            return (
              <button
                key={item.id}
                className={store.view === item.id ? "nav-item active" : "nav-item"}
                onClick={() => store.setView(item.id)}
              >
                <Icon size={17} />
                <span>{item.label}</span>
                {item.id === "review" && store.diagnostics.length > 0 && (
                  <span className="nav-count">{store.diagnostics.length}</span>
                )}
              </button>
            );
          })}
        </nav>
        <div className="sidebar-footer">
          <Shield size={15} />
          <span>Local only. No telemetry.</span>
        </div>
      </aside>

      <main className="main-shell">
        <TopBar />
        {store.error && (
          <div className="error-banner" role="alert">
            <span>{store.error}</span>
            <button onClick={store.clearError}>Dismiss</button>
          </div>
        )}
        <div className="workspace-content">
          {store.view === "workspace" && <WorkspaceView summary={store.workspace} />}
          {store.view === "fleet" && (
            <FleetView
              summary={store.fleet}
              configPath={store.fleetConfigPath}
              loading={store.loading}
              onChooseConfig={store.chooseFleetConfig}
              onRun={store.runFleet}
            />
          )}
          {store.view === "review" && (
            <ReviewView
              diagnostics={store.diagnostics}
              selected={selected}
              selectedId={store.selectedDiagnosticId}
              preview={store.preview}
              verification={store.verification}
              loading={store.loading}
            />
          )}
          {store.view === "refactor" && <RefactorView diagnostics={store.diagnostics} />}
          {store.view === "optimize" && <OptimizeView report={store.optimization} />}
          {store.view === "rules" && <RulesView />}
          {store.view === "engines" && <EnginesView />}
          {store.view === "settings" && <SettingsView />}
        </div>
        <BottomPanel />
      </main>
    </div>
  );
}

function TopBar() {
  const { workspace, loading, openRepository, refreshWorkspace, runReview } = useAppStore();
  return (
    <header className="topbar">
      <div className="repo-path">
        <GitBranch size={15} />
        <span>{workspace?.root ?? "No repository open"}</span>
        {workspace?.git_branch && <em>{workspace.git_branch}</em>}
      </div>
      <div className="topbar-actions">
        {workspace && (
          <button className="button ghost" onClick={() => void refreshWorkspace()} disabled={loading}>
            <RefreshCw size={15} /> Refresh
          </button>
        )}
        {workspace && (
          <button className="button primary" onClick={() => void runReview(false, true)} disabled={loading}>
            <SearchCode size={15} /> Run review
          </button>
        )}
        <button className="button secondary" onClick={() => void openRepository()} disabled={loading}>
          <FolderOpen size={15} /> Open Repository
        </button>
      </div>
    </header>
  );
}

function WorkspaceView({ summary }: { summary?: WorkspaceSummary }) {
  const { openRepository, runReview } = useAppStore();
  if (!summary) {
    return (
      <section className="welcome">
        <div className="welcome-mark"><Braces size={46} /></div>
        <span className="eyebrow">Local-first code transformation runtime</span>
        <h1>Review, transform, verify, and benchmark in one workflow.</h1>
        <p>
          CodeForge keeps each language’s native AST and tooling while sharing workspace,
          diagnostics, transactions, verification, and benchmark protocols.
        </p>
        <button className="button primary large" onClick={() => void openRepository()}>
          <FolderOpen size={18} /> Open Repository
        </button>
        <div className="workflow-line">
          Open <ChevronRight size={14} /> Detect <ChevronRight size={14} /> Review <ChevronRight size={14} /> Preview <ChevronRight size={14} /> Verify
        </div>
      </section>
    );
  }

  const languages = Object.entries(summary.languages);
  const maxBytes = Math.max(1, ...languages.map(([, stats]) => stats.bytes));
  return (
    <section className="page">
      <PageHeading eyebrow="Workspace" title={summary.name} description="Detected project structure and available language adapters." />
      <div className="workspace-grid">
        <div className="surface">
          <header className="section-heading"><Languages size={17} /><h2>Languages</h2></header>
          <div className="language-table">
            {languages.map(([language, stats]) => (
              <div className="language-row" key={language}>
                <strong>{language}</strong>
                <span>{stats.files} files</span>
                <div className="language-bar"><i style={{ width: `${Math.max(6, (stats.bytes / maxBytes) * 100)}%` }} /></div>
                <span>{formatBytes(stats.bytes)}</span>
              </div>
            ))}
            {languages.length === 0 && <p className="muted">No supported source files were detected.</p>}
          </div>
        </div>
        <div className="surface compact">
          <header className="section-heading"><Boxes size={17} /><h2>Project markers</h2></header>
          <div className="marker-list">
            {summary.project_markers.map((marker) => <code key={marker}>{marker}</code>)}
            {summary.project_markers.length === 0 && <p className="muted">No standard project marker found.</p>}
          </div>
          <button className="button primary full" onClick={() => void runReview(false, true)}>
            <SearchCode size={16} /> Analyze workspace
          </button>
        </div>
      </div>
    </section>
  );
}

function FleetView({
  summary,
  configPath,
  loading,
  onChooseConfig,
  onRun
}: {
  summary?: FleetRunSummary;
  configPath?: string;
  loading: boolean;
  onChooseConfig: () => Promise<void>;
  onRun: (command: string) => Promise<void>;
}) {
  const [selectedName, setSelectedName] = useState<string>();
  const selected = summary?.repositories.find((repository) => repository.repository === selectedName)
    ?? summary?.repositories[0];
  const actions = ["audit", "beautify", "review", "refactor", "optimize", "verify"];
  return (
    <section className="page fleet-page">
      <PageHeading
        eyebrow="Fleet"
        title={summary?.fleet_name ?? "Repository fleet"}
        description="Independent repository transactions, policy checks, and evidence reports. Desktop actions are preview-only."
      />
      <div className="fleet-toolbar surface">
        <button className="button secondary" onClick={() => void onChooseConfig()} disabled={loading}>
          <FolderOpen size={15} /> Choose fleet.toml
        </button>
        <code>{configPath ?? "No fleet configuration selected"}</code>
        <div className="fleet-actions">
          {actions.map((action) => (
            <button
              key={action}
              className={action === "audit" ? "button primary" : "button secondary"}
              onClick={() => void onRun(action)}
              disabled={loading || !configPath}
            >
              {action}
            </button>
          ))}
        </div>
      </div>

      {!summary ? (
        <EmptyState title="No fleet run" text="Choose fleet.toml, then run a read-only audit, review, or preview." />
      ) : (
        <div className="fleet-layout">
          <div className="fleet-repositories surface">
            <header className="section-heading">
              <Network size={17} />
              <h2>Repositories</h2>
              <span className="muted">{summary.status}</span>
            </header>
            <div className="fleet-repo-list">
              {summary.repositories.map((repository) => (
                <button
                  key={repository.repository}
                  className={selected?.repository === repository.repository ? "fleet-repo active" : "fleet-repo"}
                  onClick={() => setSelectedName(repository.repository)}
                >
                  <span className="repo-status-dot" data-status={repository.status} />
                  <span>
                    <strong>{repository.repository}</strong>
                    <small>{repository.languages.join(", ") || "no source languages"}</small>
                  </span>
                  <span className="repo-metrics">{repository.findings} findings</span>
                </button>
              ))}
            </div>
          </div>
          <div className="fleet-detail">
            {selected && <FleetRepositoryDetail repository={selected} />}
          </div>
        </div>
      )}
    </section>
  );
}

function FleetRepositoryDetail({ repository }: { repository: RepoRunSummary }) {
  const verification = repository.verification;
  return (
    <>
      <div className="surface compact">
        <header className="section-heading"><SearchCode size={17} /><h2>{repository.repository}</h2></header>
        <div className="fleet-summary-grid">
          <Metric label="Status" value={repository.status} />
          <Metric label="Risk" value={repository.risk} />
          <Metric label="Source findings" value={String(repository.source_findings ?? repository.findings)} />
          <Metric label="Tool gaps" value={String(repository.tool_gaps?.length ?? 0)} />
          <Metric label="Finding status" value={repository.finding_status ?? "unknown"} />
          <Metric label="Pending" value={String(repository.pending_transformations)} />
        </div>
        <p className="muted">{repository.message}</p>
        {repository.tool_gaps && repository.tool_gaps.length > 0 && (
          <div className="patch-file-list">
            {repository.tool_gaps.map((gap) => (
              <code key={`${gap.tool}-${gap.required_for}`}>{gap.tool}: {gap.status}</code>
            ))}
          </div>
        )}
        {repository.report_path && <code className="report-path">{repository.report_path}</code>}
      </div>
      <div className="surface">
        <header className="section-heading"><ListChecks size={17} /><h2>Verification</h2></header>
        {verification ? <VerificationStrip result={verification} /> : <p className="muted">No verification run for this repository operation.</p>}
      </div>
      {repository.evidence?.benchmark && <BenchmarkBlock result={repository.evidence.benchmark} />}
      {repository.evidence?.patch && (
        <div className="surface compact">
          <header className="section-heading"><FileSearch size={17} /><h2>Preview</h2></header>
          <p className="muted">{repository.evidence.patch.files.length} files, {repository.evidence.patch.additions} additions, {repository.evidence.patch.deletions} deletions.</p>
          <div className="patch-file-list">
            {repository.evidence.patch.files.map((file) => <code key={file.file}>{file.file}</code>)}
          </div>
        </div>
      )}
    </>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return <div className="metric"><span>{label}</span><strong>{value}</strong></div>;
}

function ReviewView({
  diagnostics,
  selected,
  selectedId,
  preview,
  verification,
  loading
}: {
  diagnostics: Diagnostic[];
  selected?: Diagnostic;
  selectedId?: string;
  preview: ReturnType<typeof useAppStore.getState>["preview"];
  verification?: VerificationResult;
  loading: boolean;
}) {
  const { selectDiagnostic, previewFix, applyPreview, runVerification, undoTransaction, history, saveDisposition } = useAppStore();
  const saveReviewDisposition = (disposition: "accepted" | "false_positive" | "human_review") => {
    const reason = disposition === "human_review" ? window.prompt("Reason for human review?") ?? undefined : window.prompt("Why should this finding be saved with this disposition?") ?? undefined;
    if (disposition !== "human_review" && !reason?.trim()) return;
    void saveDisposition(disposition, reason?.trim() || undefined);
  };
  return (
    <section className="review-layout">
      <ProblemList diagnostics={diagnostics} selectedId={selectedId} onSelect={selectDiagnostic} />
      <div className="review-main">
        <div className="review-toolbar">
          <div>
            <span className="eyebrow">Review</span>
            <strong>{diagnostics.length} findings</strong>
          </div>
          <div className="toolbar-actions">
            {selected && selected.fixes.length > 0 && !preview && (
              <button className="button primary" disabled={loading} onClick={() => void previewFix(selected.id, 0)}>
                <SearchCode size={15} /> Preview fix
              </button>
            )}
            {preview && (
              <button className="button primary" disabled={loading} onClick={() => void applyPreview()}>
                <Wrench size={15} /> Apply fix
              </button>
            )}
            {history[0] && (
              <button className="button ghost" disabled={loading} onClick={() => void undoTransaction(history[0].id)}>
                <RotateCcw size={15} /> Undo
              </button>
            )}
            <button className="button secondary" disabled={loading} onClick={() => void runVerification()}>
              <CircleGauge size={15} /> Verify
            </button>
            {selected && (
              <>
                <button className="button ghost" disabled={loading} onClick={() => saveReviewDisposition("accepted")}>
                  Save accepted
                </button>
                <button className="button ghost" disabled={loading} onClick={() => saveReviewDisposition("false_positive")}>
                  Save false positive
                </button>
                <button className="button ghost" disabled={loading} onClick={() => saveReviewDisposition("human_review")}>
                  Human review
                </button>
              </>
            )}
          </div>
        </div>
        <DiffPreview diagnostic={selected} preview={preview} />
        <VerificationStrip result={verification} />
      </div>
    </section>
  );
}

function RefactorView({ diagnostics }: { diagnostics: Diagnostic[] }) {
  const { previewFix, setView } = useAppStore();
  const fixable = diagnostics.filter((diagnostic) => diagnostic.fixes.length > 0);
  return (
    <section className="page">
      <PageHeading eyebrow="Refactor" title="Preview-first transformations" description="Every edit is materialized as a patch and remains reversible." />
      <div className="refactor-list">
        {fixable.map((diagnostic) => (
          <article className="refactor-row" key={diagnostic.id}>
            <div>
              <span className="rule-id">{diagnostic.rule_id}</span>
              <h3>{diagnostic.message}</h3>
              <p>{diagnostic.file}:{diagnostic.range.start_line}</p>
            </div>
            <div className="refactor-actions">
              {diagnostic.fixes.map((fix, index) => (
                <button
                  key={fix.id}
                  className="button secondary"
                  onClick={() => void previewFix(diagnostic.id, index).then(() => setView("review"))}
                >
                  {fix.title} {fix.safe ? "· safe" : "· review"}
                </button>
              ))}
            </div>
          </article>
        ))}
        {fixable.length === 0 && <EmptyState title="No transformations available" text="Run a review first or install optional refactoring engines." />}
      </div>
    </section>
  );
}

function OptimizeView({ report }: { report?: OptimizationReport }) {
  const { runOptimization, runBenchmark, benchmark, loading } = useAppStore();
  return (
    <section className="page">
      <PageHeading eyebrow="Optimize" title="Candidate → sandbox → verification" description="Performance claims are shown only when a measured benchmark exists." />
      <div className="optimize-actions">
        <button className="button primary" onClick={() => void runOptimization()} disabled={loading}>
          <Sparkles size={16} /> Find and verify candidates
        </button>
        <button className="button secondary" onClick={() => void runBenchmark()} disabled={loading}>
          <Activity size={16} /> Benchmark current workspace
        </button>
      </div>
      {report && (
        <div className="optimization-report">
          <div className="notice">{report.message}</div>
          {report.candidates.map((candidate) => (
            <div className="candidate-row" key={candidate.diagnostic_id}>
              <strong>{candidate.title}</strong>
              <span>{candidate.file}:{candidate.line}</span>
              <code>{candidate.rule_id}</code>
            </div>
          ))}
          <VerificationStrip result={report.verification} />
          {report.benchmark && report.benchmark.before.samples.length > 0 && (
            <BenchmarkBlock result={report.benchmark} />
          )}
        </div>
      )}
      {benchmark && <BenchmarkSnapshotBlock snapshot={benchmark} />}
    </section>
  );
}

function RulesView() {
  const rules = [
    ["PY-CORRECTNESS-001", "Python", "Use identity comparison with None", "Safe fix"],
    ["PY-STYLE-001", "Python", "Bare except catches BaseException", "Safe fix"],
    ["RS-CORRECTNESS-001", "Rust", "unwrap may panic", "Review"],
    ["C-SECURITY-00x", "C", "Unbounded string and command APIs", "Review"],
    ["JS-MODERNIZE-001", "JavaScript", "var scope and redeclaration", "Review"],
    ["JS-CORRECTNESS-001", "JavaScript", "Loose equality coercion", "Review"],
    ["JAVA-CORRECTNESS-001", "Java", "Empty catch swallows failures", "Review"],
    ["GO-PERFORMANCE-001", "Go", "strings.Index used only for presence", "Measured candidate"]
  ];
  return (
    <section className="page">
      <PageHeading eyebrow="Rules" title="Built-in Tree-sitter rules" description="Rules are heuristic unless a stronger verification stage is recorded." />
      <div className="rules-table surface">
        {rules.map(([id, language, rule, action]) => (
          <div className="rule-row" key={id}>
            <code>{id}</code><span>{language}</span><strong>{rule}</strong><em>{action}</em>
          </div>
        ))}
      </div>
    </section>
  );
}

function EnginesView() {
  const { engines } = useAppStore();
  return (
    <section className="page">
      <PageHeading eyebrow="Engines" title="Local discovery" description="Missing optional tools degrade to unavailable; they never prevent the workspace from opening." />
      <div className="engine-grid">
        {engines.map((engine) => (
          <article className={engine.available ? "engine-row available" : "engine-row"} key={engine.metadata.id}>
            <div>
              <strong>{engine.metadata.name}</strong>
              <span>{engine.metadata.id}</span>
            </div>
            <span>{engine.metadata.languages.join(", ")}</span>
            <span>{engine.metadata.capabilities.join(", ")}</span>
            <span className={engine.available ? "availability yes" : "availability no"}>
              {engine.available ? "available" : "missing"}
            </span>
            {engine.reason && <p>{engine.reason}</p>}
          </article>
        ))}
        {engines.length === 0 && <EmptyState title="No repository open" text="Open a repository to discover installed engines." />}
      </div>
    </section>
  );
}

function SettingsView() {
  const { theme, setTheme } = useAppStore();
  return (
    <section className="page">
      <PageHeading eyebrow="Settings" title="Local-first defaults" description="Network-backed AI features are disabled by default." />
      <div className="settings-list surface">
        <div className="setting-row">
          <strong>Theme</strong>
          <div className="theme-switch">
            {(["dark", "light", "system"] as Theme[]).map((value) => (
              <button key={value} className={theme === value ? "active" : ""} onClick={() => setTheme(value)}>{value}</button>
            ))}
          </div>
        </div>
        <SettingRow title="Source upload" value="Disabled" />
        <SettingRow title="Telemetry" value="Disabled" />
        <SettingRow title="Transformation preview" value="Required" />
        <SettingRow title="Undo history" value="Persistent on disk" />
        <SettingRow title="External process execution" value="Structured arguments only" />
      </div>
    </section>
  );
}

function BottomPanel() {
  const { diagnostics, verification, benchmark } = useAppStore();
  return (
    <footer className="bottom-panel">
      <div className="bottom-tab active"><ListChecks size={14} /> Tasks</div>
      <div className="bottom-tab"><Activity size={14} /> Benchmark {benchmark ? benchmark.summary.median.toFixed(2) : "—"}</div>
      <div className="bottom-tab"><SearchCode size={14} /> Engine Output {diagnostics.length ? `${diagnostics.length} findings` : "idle"}</div>
      <div className="bottom-tab"><History size={14} /> Verification {verification ? evidenceLabel(verification) : "not run"}</div>
    </footer>
  );
}

function evidenceLabel(result: VerificationResult) {
  if (result.equivalence.status === "passed") return "verified";
  if (result.tests.status === "passed") return "tested";
  if (result.build.status === "passed") return "compiled";
  if (result.syntax.status === "passed") return "heuristic";
  return "unverified";
}

function VerificationStrip({ result }: { result?: VerificationResult }) {
  if (!result) return <div className="verification-strip muted">Verification has not been run.</div>;
  const checks = [
    ["syntax", result.syntax.status],
    ["typecheck", result.typecheck.status],
    ["build", result.build.status],
    ["tests", result.tests.status],
    ["equivalence", result.equivalence.status]
  ] as const;
  return (
    <div className="verification-strip">
      <strong>Evidence</strong>
      {checks.map(([label, status]) => (
        <span className="verification-item" key={label}>{label}<VerificationPill status={status} /></span>
      ))}
    </div>
  );
}

function BenchmarkBlock({ result }: { result: NonNullable<OptimizationReport["benchmark"]> }) {
  return (
    <div className="benchmark-block">
      <div><span>Before</span><strong>{result.before.median.toFixed(3)} ms</strong></div>
      <div><span>After</span><strong>{result.after.median.toFixed(3)} ms</strong></div>
      <div><span>Delta</span><strong>{result.delta_percent >= 0 ? "+" : ""}{result.delta_percent.toFixed(2)}%</strong></div>
      <div><span>Samples</span><strong>{result.before.samples.length}/{result.after.samples.length}</strong></div>
    </div>
  );
}

function BenchmarkSnapshotBlock({ snapshot }: { snapshot: ReturnType<typeof useAppStore.getState>["benchmark"] }) {
  if (!snapshot) return null;
  return (
    <div className="surface benchmark-snapshot">
      <header className="section-heading"><Activity size={17} /><h2>{snapshot.metric}</h2></header>
      <code>{snapshot.command.join(" ")}</code>
      <div className="benchmark-block">
        <div><span>Median</span><strong>{snapshot.summary.median.toFixed(3)} {snapshot.summary.unit}</strong></div>
        <div><span>Variance</span><strong>{snapshot.summary.variance.toFixed(6)}</strong></div>
        <div><span>Samples</span><strong>{snapshot.summary.samples.length}</strong></div>
      </div>
    </div>
  );
}

function PageHeading({ eyebrow, title, description }: { eyebrow: string; title: string; description: string }) {
  return <header className="page-heading"><span className="eyebrow">{eyebrow}</span><h1>{title}</h1><p>{description}</p></header>;
}

function SettingRow({ title, value }: { title: string; value: string }) {
  return <div className="setting-row"><strong>{title}</strong><span>{value}</span></div>;
}

function EmptyState({ title, text }: { title: string; text: string }) {
  return <div className="empty-state"><SearchCode size={30} /><h2>{title}</h2><p>{text}</p></div>;
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}
