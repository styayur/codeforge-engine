use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use codeforge_core::{
    BenchmarkSnapshot, CodeForgeEngine, FixPreview, OptimizationReport, ReviewOptions, ReviewReport,
};
use codeforge_fleet::{FleetCommand, FleetConfig, FleetRunOptions, FleetRunner};
use codeforge_protocol::{
    Diagnostic, EngineStatus, FleetRunSummary, Patch, RiskLevel, TransactionRecord,
    VerificationResult, WorkspaceSummary,
};
use serde::Serialize;
use tauri::State;
use tokio::sync::{Mutex, RwLock};

#[derive(Default)]
struct DesktopState {
    engine: RwLock<Option<Arc<CodeForgeEngine>>>,
    previews: Mutex<HashMap<String, codeforge_transform::PreparedChange>>,
}

#[derive(Debug, Serialize)]
struct WorkspaceResponse {
    summary: WorkspaceSummary,
    engines: Vec<EngineStatus>,
}

#[derive(Debug, Serialize)]
struct PreviewFile {
    path: PathBuf,
    before: String,
    after: String,
}

#[derive(Debug, Serialize)]
struct FixPreviewResponse {
    preview_id: String,
    diagnostic: Diagnostic,
    patch: Patch,
    files: Vec<PreviewFile>,
}

#[tauri::command]
async fn open_workspace(
    path: PathBuf,
    state: State<'_, DesktopState>,
) -> Result<WorkspaceResponse, String> {
    let engine = tokio::task::spawn_blocking(move || CodeForgeEngine::open(path))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    let response = WorkspaceResponse {
        summary: engine.summary(),
        engines: engine.engine_statuses(),
    };
    *state.engine.write().await = Some(Arc::new(engine));
    Ok(response)
}

#[tauri::command]
async fn get_workspace(state: State<'_, DesktopState>) -> Result<WorkspaceResponse, String> {
    let engine = current_engine(&state).await?;
    Ok(WorkspaceResponse {
        summary: engine.summary(),
        engines: engine.engine_statuses(),
    })
}

#[tauri::command]
async fn run_review(
    changed_only: bool,
    include_external: bool,
    state: State<'_, DesktopState>,
) -> Result<ReviewReport, String> {
    let engine = current_engine(&state).await?;
    engine
        .review(ReviewOptions {
            languages: Vec::new(),
            changed_only,
            include_external,
        })
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn preview_fix(
    diagnostic_id: String,
    fix_index: usize,
    state: State<'_, DesktopState>,
) -> Result<FixPreviewResponse, String> {
    let engine = current_engine(&state).await?;
    let FixPreview {
        diagnostic,
        preview,
    } = engine
        .preview_fix(&diagnostic_id, fix_index)
        .map_err(|error| error.to_string())?;
    let preview_id = preview.patch.id.clone();
    let patch = preview.patch.clone();
    let files = preview
        .files
        .iter()
        .map(|file| PreviewFile {
            path: file.relative_path.clone(),
            before: file.before.clone(),
            after: file.after.clone(),
        })
        .collect();
    state
        .previews
        .lock()
        .await
        .insert(preview_id.clone(), preview);
    Ok(FixPreviewResponse {
        preview_id,
        diagnostic,
        patch,
        files,
    })
}

#[tauri::command]
async fn apply_fix(
    preview_id: String,
    force: bool,
    state: State<'_, DesktopState>,
) -> Result<TransactionRecord, String> {
    let engine = current_engine(&state).await?;
    let preview = state
        .previews
        .lock()
        .await
        .remove(&preview_id)
        .ok_or_else(|| format!("preview not found: {preview_id}"))?;
    let title = preview
        .patch
        .transformation_ids
        .first()
        .map(|id| format!("Apply fix {id}"))
        .unwrap_or_else(|| "Apply reviewed fix".to_owned());
    engine
        .apply_preview(&preview, title, force)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn undo_fix(transaction_id: String, state: State<'_, DesktopState>) -> Result<(), String> {
    let engine = current_engine(&state).await?;
    engine
        .undo(&transaction_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn list_history(state: State<'_, DesktopState>) -> Result<Vec<TransactionRecord>, String> {
    let engine = current_engine(&state).await?;
    engine.history().map_err(|error| error.to_string())
}

#[tauri::command]
async fn run_verification(
    full: bool,
    state: State<'_, DesktopState>,
) -> Result<VerificationResult, String> {
    let engine = current_engine(&state).await?;
    engine.verify(full).await.map_err(|error| error.to_string())
}

#[tauri::command]
async fn run_optimization(state: State<'_, DesktopState>) -> Result<OptimizationReport, String> {
    let engine = current_engine(&state).await?;
    engine.optimize().await.map_err(|error| error.to_string())
}

#[tauri::command]
async fn run_benchmark(
    samples: usize,
    state: State<'_, DesktopState>,
) -> Result<BenchmarkSnapshot, String> {
    let engine = current_engine(&state).await?;
    engine
        .benchmark(samples)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn run_fleet(
    command: String,
    config_path: PathBuf,
    risk: Option<String>,
) -> Result<FleetRunSummary, String> {
    let operation = match command.to_ascii_lowercase().as_str() {
        "audit" => FleetCommand::Audit,
        "format" | "beautify" => FleetCommand::Format,
        "review" => FleetCommand::Review,
        "refactor" => FleetCommand::Refactor,
        "optimize" => FleetCommand::Optimize,
        "verify" => FleetCommand::Verify,
        "report" => FleetCommand::Report,
        other => return Err(format!("unsupported fleet command: {other}")),
    };
    let config = FleetConfig::load(&config_path).map_err(|error| error.to_string())?;
    let mut options = FleetRunOptions::new(operation);
    options.dry_run = true;
    options.apply = false;
    options.open_pr = false;
    if let Some(risk) = risk {
        options.risk = match risk.to_ascii_lowercase().as_str() {
            "low" => RiskLevel::Low,
            "medium" => RiskLevel::Medium,
            "high" => RiskLevel::High,
            "very-high" | "very_high" => RiskLevel::VeryHigh,
            other => return Err(format!("invalid risk level: {other}")),
        };
    }
    FleetRunner::new(config)
        .run(options)
        .await
        .map_err(|error| error.to_string())
}

async fn current_engine(state: &State<'_, DesktopState>) -> Result<Arc<CodeForgeEngine>, String> {
    state
        .engine
        .read()
        .await
        .clone()
        .ok_or_else(|| "open a repository before running this command".to_owned())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(DesktopState::default())
        .invoke_handler(tauri::generate_handler![
            open_workspace,
            get_workspace,
            run_review,
            preview_fix,
            apply_fix,
            undo_fix,
            list_history,
            run_verification,
            run_optimization,
            run_benchmark,
            run_fleet,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run CodeForge desktop");
}
