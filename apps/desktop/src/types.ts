export type Theme = "dark" | "light" | "system";
export type View = "workspace" | "fleet" | "review" | "refactor" | "optimize" | "rules" | "engines" | "settings";
export type Severity = "hint" | "info" | "warning" | "error";
export type VerificationStatus = "not_run" | "passed" | "failed" | "unavailable" | "skipped";

export interface SourceRange {
  start_byte: number;
  end_byte: number;
  start_line: number;
  start_column: number;
  end_line: number;
  end_column: number;
}

export interface TextEdit {
  range: SourceRange;
  replacement: string;
  description?: string;
}

export interface Fix {
  id: string;
  title: string;
  edits: TextEdit[];
  safe: boolean;
}

export interface Diagnostic {
  id: string;
  engine: string;
  language: string;
  rule_id: string;
  severity: Severity;
  category: string;
  confidence: string;
  file: string;
  range: SourceRange;
  message: string;
  explanation?: string;
  source?: string;
  fixes: Fix[];
  tags: string[];
}

export interface LanguageStats { files: number; bytes: number; }

export interface WorkspaceSummary {
  root: string;
  name: string;
  languages: Record<string, LanguageStats>;
  project_markers: string[];
  git_repository: boolean;
  git_branch?: string;
}

export interface EngineMetadata {
  id: string;
  name: string;
  version: string;
  languages: string[];
  capabilities: string[];
  executable?: string;
  dependencies: string[];
  timeout_ms?: number;
  permissions: string[];
}

export interface EngineStatus {
  metadata: EngineMetadata;
  available: boolean;
  reason?: string;
}

export interface WorkspaceResponse {
  summary: WorkspaceSummary;
  engines: EngineStatus[];
}

export interface ReviewReport {
  diagnostics: Diagnostic[];
  files_analyzed: number;
  languages: string[];
  engines_used: string[];
  duration_ms: number;
}

export interface FilePatch {
  file: string;
  before_hash: string;
  after_hash: string;
  additions: number;
  deletions: number;
  unified_diff: string;
}

export interface Patch {
  id: string;
  transformation_ids: string[];
  files: FilePatch[];
  additions: number;
  deletions: number;
}

export interface PreviewFile {
  path: string;
  before: string;
  after: string;
}

export interface FixPreviewResponse {
  preview_id: string;
  diagnostic: Diagnostic;
  patch: Patch;
  files: PreviewFile[];
}

export interface VerificationCheck {
  status: VerificationStatus;
  message?: string;
  duration_ms?: number;
  command?: string[];
}

export interface VerificationResult {
  syntax: VerificationCheck;
  typecheck: VerificationCheck;
  build: VerificationCheck;
  tests: VerificationCheck;
  fuzz: VerificationCheck;
  differential: VerificationCheck;
  equivalence: VerificationCheck;
  benchmark: VerificationCheck;
}

export interface TransactionFile {
  path: string;
  before_hash: string;
  after_hash: string;
  before: string;
  after: string;
}

export interface TransactionRecord {
  id: string;
  created_at: string;
  title: string;
  patch: Patch;
  verification: VerificationResult;
  benchmarks: BenchmarkResult[];
  files: TransactionFile[];
}

export interface TimingSummary {
  unit: string;
  samples: number[];
  mean: number;
  median: number;
  variance: number;
  min: number;
  max: number;
}

export interface BenchmarkResult {
  metric: string;
  before: TimingSummary;
  after: TimingSummary;
  delta_percent: number;
  environment: Record<string, string>;
}

export interface BenchmarkSnapshot {
  metric: string;
  command: string[];
  summary: TimingSummary;
}

export interface OptimizationCandidate {
  diagnostic_id: string;
  rule_id: string;
  title: string;
  file: string;
  line: number;
}

export interface OptimizationReport {
  candidates: OptimizationCandidate[];
  applied: boolean;
  patch?: Patch;
  verification: VerificationResult;
  benchmark?: BenchmarkResult;
  message: string;
}

export interface ProjectProfile {
  root: string;
  name: string;
  ecosystems: Array<{
    ecosystem: string;
    languages: string[];
    markers: string[];
    confidence: string;
    suggested_commands: Record<string, string[]>;
  }>;
  languages: string[];
}

export interface EvidenceBundle {
  id: string;
  repository: string;
  commit?: string;
  branch?: string;
  risk: string;
  transformation_classes: string[];
  baseline: VerificationResult;
  after: VerificationResult;
  evidence: Record<string, VerificationStatus>;
  benchmark?: BenchmarkResult;
  patch?: Patch;
  report_dir: string;
}

export interface RepoRunSummary {
  repository: string;
  path: string;
  status: string;
  languages: string[];
  project_profile?: ProjectProfile;
  findings: number;
  pending_transformations: number;
  risk: string;
  verification?: VerificationResult;
  evidence?: EvidenceBundle;
  report_path?: string;
  message: string;
}

export interface FleetRunSummary {
  run_id: string;
  fleet_name: string;
  status: string;
  started_at: string;
  finished_at: string;
  repositories: RepoRunSummary[];
  report_dir: string;
}
