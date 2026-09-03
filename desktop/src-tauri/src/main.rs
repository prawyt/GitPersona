use gitpersona::{
    api::{
        ApiError, DoctorReport, NamedProfile, ProfileDraft, RepositoryScanEvent, RepositoryStatus,
        RepositorySummary, SshTestReport,
    },
    config::Profile,
    github::Account,
    process::SystemRunner,
    service::GitPersonaService,
};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tauri::{AppHandle, State, ipc::Channel};
use tauri_plugin_dialog::DialogExt;

#[derive(Default)]
struct ScanState(Mutex<Option<Arc<AtomicBool>>>);

#[derive(Default)]
struct ApprovedPaths(Mutex<HashSet<PathBuf>>);

fn service() -> Result<GitPersonaService<'static>, ApiError> {
    static RUNNER: SystemRunner = SystemRunner;
    GitPersonaService::discover(&RUNNER).map_err(Into::into)
}

#[tauri::command]
async fn list_profiles() -> Result<Vec<NamedProfile>, ApiError> {
    service()?.list_profiles().map_err(Into::into)
}

#[tauri::command]
async fn create_profile(name: String, profile: Profile) -> Result<NamedProfile, ApiError> {
    service()?
        .create_profile(&name, profile)
        .map_err(Into::into)
}

#[tauri::command]
async fn update_profile(name: String, profile: Profile) -> Result<NamedProfile, ApiError> {
    service()?
        .update_profile(&name, profile)
        .map_err(Into::into)
}

#[tauri::command]
async fn remove_profile(name: String) -> Result<(), ApiError> {
    service()?.remove_profile(&name).map_err(Into::into)
}

#[tauri::command]
async fn rename_profile(old_name: String, new_name: String) -> Result<NamedProfile, ApiError> {
    service()?
        .rename_profile(&old_name, &new_name)
        .map_err(Into::into)
}

#[tauri::command]
async fn import_profile_preview(
    repository: PathBuf,
    approved: State<'_, ApprovedPaths>,
) -> Result<ProfileDraft, ApiError> {
    let resolved = ensure_approved(&repository, &approved)?;
    service()?
        .import_preview(&resolved, "origin")
        .map_err(Into::into)
}

#[tauri::command]
fn choose_folder(
    app: AppHandle,
    approved: State<'_, ApprovedPaths>,
) -> Result<Option<PathBuf>, ApiError> {
    app.dialog()
        .file()
        .blocking_pick_folder()
        .map(|path| {
            path.into_path()
                .map_err(|error| ApiError {
                    kind: "usage".into(),
                    message: error.to_string(),
                    exit_code: 2,
                    field: Some("path".into()),
                })
                .and_then(|path| {
                    let canonical = std::fs::canonicalize(&path).map_err(ApiError::from_io)?;
                    let normalized = strip_unc_prefix(&canonical);
                    approved
                        .0
                        .lock()
                        .map_err(|_| ApiError::internal("approved-folder state is unavailable"))?
                        .insert(normalized.clone());
                    Ok(normalized)
                })
        })
        .transpose()
}

#[tauri::command]
fn choose_key_file(app: AppHandle) -> Result<Option<PathBuf>, ApiError> {
    app.dialog()
        .file()
        .blocking_pick_file()
        .map(|path| {
            path.into_path().map_err(|error| ApiError {
                kind: "usage".into(),
                message: error.to_string(),
                exit_code: 2,
                field: Some("ssh_key".into()),
            })
        })
        .transpose()
}

#[tauri::command]
async fn list_repository_roots() -> Result<Vec<PathBuf>, ApiError> {
    service()?.repository_roots().map_err(Into::into)
}

#[tauri::command]
async fn add_repository_root(
    path: PathBuf,
    approved: State<'_, ApprovedPaths>,
) -> Result<PathBuf, ApiError> {
    let resolved = ensure_session_approved(&path, &approved)?;
    service()?
        .add_repository_root(&resolved)
        .map_err(Into::into)
}

#[tauri::command]
async fn remove_repository_root(path: PathBuf) -> Result<(), ApiError> {
    service()?.remove_repository_root(&path).map_err(Into::into)
}

#[tauri::command]
async fn scan_repositories(
    state: State<'_, ScanState>,
    events: Channel<RepositoryScanEvent>,
) -> Result<Vec<RepositorySummary>, ApiError> {
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut guard = state
            .0
            .lock()
            .map_err(|_| ApiError::internal("scan-state lock poisoned"))?;
        if let Some(prev) = guard.replace(cancel.clone()) {
            prev.store(true, Ordering::Relaxed);
        }
    }
    let cancel_for_worker = cancel.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        service()?
            .scan_repositories(&cancel_for_worker, |event| {
                let _ = events.send(event);
            })
            .map_err(ApiError::from)
    })
    .await
    .map_err(|error| ApiError {
        kind: "dependency".into(),
        message: error.to_string(),
        exit_code: 3,
        field: None,
    })?;

    let Ok(mut guard) = state.0.lock() else {
        return result;
    };
    if guard
        .as_ref()
        .is_some_and(|current| Arc::ptr_eq(current, &cancel))
    {
        *guard = None;
    }

    result
}

#[tauri::command]
fn cancel_repository_scan(state: State<'_, ScanState>) {
    let Ok(guard) = state.0.lock() else { return };
    if let Some(cancel) = guard.as_ref() {
        cancel.store(true, Ordering::Relaxed);
    }
}

#[tauri::command]
async fn inspect_repository(
    repository: PathBuf,
    network: bool,
    approved: State<'_, ApprovedPaths>,
) -> Result<RepositoryStatus, ApiError> {
    let resolved = ensure_approved(&repository, &approved)?;
    service()?
        .inspect_repository(&resolved, "origin", network)
        .map_err(Into::into)
}

#[tauri::command]
async fn bind_repository(
    repository: PathBuf,
    profile: String,
    force: bool,
    approved: State<'_, ApprovedPaths>,
) -> Result<(), ApiError> {
    let resolved = ensure_approved(&repository, &approved)?;
    service()?
        .bind_repository(&resolved, &profile, "origin", force)
        .map_err(Into::into)
}

#[tauri::command]
async fn unbind_repository(
    repository: PathBuf,
    approved: State<'_, ApprovedPaths>,
) -> Result<(), ApiError> {
    let resolved = ensure_approved(&repository, &approved)?;
    service()?.unbind_repository(&resolved).map_err(Into::into)
}

#[tauri::command]
async fn switch_github_account(profile: String) -> Result<(), ApiError> {
    service()?
        .switch_github_account(&profile)
        .map_err(Into::into)
}

#[tauri::command]
async fn github_accounts(hostname: String) -> Result<Vec<Account>, ApiError> {
    service()?.github_accounts(&hostname).map_err(Into::into)
}

#[tauri::command]
async fn test_ssh(profile: String) -> Result<SshTestReport, ApiError> {
    service()?.ssh_test(&profile).map_err(Into::into)
}

#[tauri::command]
async fn doctor() -> Result<DoctorReport, ApiError> {
    service()?.doctor().map_err(Into::into)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(ScanState::default())
        .manage(ApprovedPaths::default())
        .invoke_handler(tauri::generate_handler![
            list_profiles,
            create_profile,
            update_profile,
            rename_profile,
            remove_profile,
            import_profile_preview,
            choose_folder,
            choose_key_file,
            list_repository_roots,
            add_repository_root,
            remove_repository_root,
            scan_repositories,
            cancel_repository_scan,
            inspect_repository,
            bind_repository,
            unbind_repository,
            switch_github_account,
            github_accounts,
            test_ssh,
            doctor
        ])
        .run(tauri::generate_context!())
        .expect("error while running GitPersona desktop");
}

fn ensure_session_approved(
    path: &PathBuf,
    approved: &State<'_, ApprovedPaths>,
) -> Result<PathBuf, ApiError> {
    let canonical = std::fs::canonicalize(path).map_err(ApiError::from_io)?;
    let normalized = strip_unc_prefix(&canonical);
    if approved
        .0
        .lock()
        .map_err(|_| ApiError::internal("approved-folder state is unavailable"))?
        .contains(&normalized)
    {
        Ok(normalized)
    } else {
        Err(ApiError {
            kind: "usage".into(),
            message: "Choose this folder in GitPersona before using it.".into(),
            exit_code: 2,
            field: Some("path".into()),
        })
    }
}

fn ensure_approved(
    path: &PathBuf,
    approved: &State<'_, ApprovedPaths>,
) -> Result<PathBuf, ApiError> {
    let canonical = std::fs::canonicalize(path).map_err(ApiError::from_io)?;
    let normalized = strip_unc_prefix(&canonical);
    if approved
        .0
        .lock()
        .map_err(|_| ApiError::internal("approved-folder state is unavailable"))?
        .contains(&normalized)
    {
        return Ok(normalized);
    }
    if service()?
        .repository_roots()?
        .iter()
        .any(|root| normalized.starts_with(strip_unc_prefix(root)))
    {
        return Ok(normalized);
    }
    Err(ApiError {
        kind: "usage".into(),
        message: "This repository is outside the folders approved in GitPersona.".into(),
        exit_code: 2,
        field: Some("repository".into()),
    })
}

/// Strip the Windows extended-length path prefix (`\\?\`) so that
/// `starts_with` comparisons work uniformly whether both sides were
/// canonicalized or not.
fn strip_unc_prefix(path: &std::path::Path) -> PathBuf {
    #[cfg(windows)]
    {
        let s = path.to_string_lossy();
        if let Some(stripped) = s.strip_prefix(r"\\?\") {
            return PathBuf::from(stripped);
        }
    }
    path.to_path_buf()
}

trait DesktopApiError {
    fn from_io(error: std::io::Error) -> Self;
    fn internal(message: &str) -> Self;
}

impl DesktopApiError for ApiError {
    fn from_io(error: std::io::Error) -> Self {
        Self {
            kind: "usage".into(),
            message: error.to_string(),
            exit_code: 2,
            field: Some("path".into()),
        }
    }
    fn internal(message: &str) -> Self {
        Self {
            kind: "dependency".into(),
            message: message.into(),
            exit_code: 3,
            field: None,
        }
    }
}
