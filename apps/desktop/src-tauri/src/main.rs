#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod authorization;
mod presentation;

use presentation::{operation_view, plan_view, scan_view};
use serde_json::{json, Value};
use std::{path::PathBuf, sync::Arc};
use tauri::{Emitter, Manager, State};
use tokio::sync::Mutex;
use un_git_core::{
    engine::{self, Selection},
    github::GithubClient,
    operations::Application,
    store::default_data_dir,
    CoreError, Result,
};

struct Host {
    app: Application,
    github: Mutex<Option<Arc<GithubClient>>>,
    authorization: authorization::AuthorizationGate,
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|_| CoreError::local("Local operation task ended unexpectedly."))?
}

async fn github(host: &Host) -> Result<Arc<GithubClient>> {
    let mut slot = host.github.lock().await;
    if let Some(client) = slot.as_ref() {
        return Ok(client.clone());
    }
    let id = std::env::var("UN_GIT_INSPECTOR_CLIENT_ID")
        .ok()
        .or_else(|| {
            host.app
                .store
                .get::<String>("setting", "github_client_id")
                .ok()
        })
        .ok_or_else(|| {
            CoreError::blocked(
                "Add the public Inspector GitHub App client ID in Connect to GitHub.",
            )
        })?;
    let client = Arc::new(GithubClient::new(host.app.store.root(), &id)?);
    *slot = Some(client.clone());
    Ok(client)
}

async fn auth_view(host: &Host) -> Value {
    match github(host).await {
        Ok(client) => match client.status().await {
            Ok(status) => {
                json!({"connected":status.connected,"login":status.account.map(|a|a.login),"message":status.authorization_status,"client_id_configured":true})
            }
            Err(error) => {
                json!({"connected":false,"message":error.message,"client_id_configured":true})
            }
        },
        Err(_) => {
            json!({"connected":false,"message":"Configure an Inspector GitHub App to load observed contributions.","client_id_configured":false})
        }
    }
}

#[tauri::command]
async fn get_dashboard(host: State<'_, Host>) -> Result<Value> {
    let app = host.app.clone();
    let mut value = blocking(move || {
        let doctor = engine::doctor();
        let tools = [ ("Git",doctor.git_version), ("Python",doctor.python_version), ("filter-repo",doctor.worker_version) ];
        let operations = app.operations()?.iter().map(|o|operation_view(&app,o)).collect::<Result<Vec<_>>>()?;
        Ok(json!({"operations":operations,"writer_enabled":false,"data_directory":app.store.root(),"toolchain":tools.into_iter().map(|(name,version)|json!({"name":name,"available":version.is_some(),"version":version.unwrap_or_else(||"Unavailable".into())})).collect::<Vec<_>>()}))
    }).await?;
    value["auth"] = auth_view(&host).await;
    Ok(value)
}

#[tauri::command]
async fn create_demo(host: State<'_, Host>) -> Result<Value> {
    let app = host.app.clone();
    blocking(move || {
        Ok(json!({"repository_path":app.create_demo()?,"label":"Synthetic attribution fixture"}))
    })
    .await
}

#[tauri::command]
async fn scan_repository(host: State<'_, Host>, repository_path: String) -> Result<Value> {
    let app = host.app.clone();
    blocking(move || {
        let scan = app.scan(&PathBuf::from(repository_path))?;
        Ok(scan_view(&app, &scan))
    })
    .await
}

#[tauri::command]
async fn plan_selection(
    host: State<'_, Host>,
    scan_id: String,
    selected_oids: Vec<String>,
    confirmed_email: String,
) -> Result<Value> {
    let app = host.app.clone();
    blocking(move || {
        Ok(plan_view(&app.plan(
            &scan_id,
            &Selection {
                commit_oids: selected_oids,
                confirmed_email,
                replacement_email: engine::default_replacement_email(),
            },
        )?))
    })
    .await
}

#[tauri::command]
async fn prepare_plan(
    host: State<'_, Host>,
    handle: tauri::AppHandle,
    plan_id: String,
) -> Result<Value> {
    let app = host.app.clone();
    blocking(move || {
        let operation = app.prepare(&plan_id, |operation| {
            if let Ok(view) = operation_view(&app, operation) {
                let _ = handle.emit("operation-progress", view);
            }
        })?;
        operation_view(&app, &operation)
    })
    .await
}

#[tauri::command]
async fn get_operation(host: State<'_, Host>, operation_id: String) -> Result<Value> {
    operation_view(&host.app, &host.app.operation(&operation_id)?)
}

#[tauri::command]
async fn verify_operation(host: State<'_, Host>, operation_id: String) -> Result<Value> {
    let app = host.app.clone();
    blocking(move || operation_view(&app, &app.verify(&operation_id)?)).await
}

#[tauri::command]
async fn cancel_operation(host: State<'_, Host>, operation_id: String) -> Result<()> {
    host.app.cancel(&operation_id)
}

#[tauri::command]
async fn preview_report(host: State<'_, Host>, operation_id: String) -> Result<Value> {
    host.app.report_preview(&operation_id)
}

#[tauri::command]
async fn export_report(host: State<'_, Host>, operation_id: String) -> Result<Value> {
    Ok(json!({"path":host.app.export_report(&operation_id)?}))
}

#[tauri::command]
async fn auth_status(host: State<'_, Host>) -> Result<Value> {
    Ok(auth_view(&host).await)
}

#[tauri::command]
async fn auth_start(host: State<'_, Host>, client_id: String) -> Result<Value> {
    let _authorization = host.authorization.change()?;
    let client = Arc::new(GithubClient::new(host.app.store.root(), &client_id)?);
    let flow = client.start_login().await?;
    host.app
        .store
        .put("setting", "github_client_id", &client_id)?;
    *host.github.lock().await = Some(client);
    Ok(
        json!({"flow_id":flow.session_id,"user_code":flow.user_code,"verification_uri":flow.verification_uri,"interval":flow.interval_seconds,"expires_in":(flow.expires_at-chrono::Utc::now().timestamp()).max(0)}),
    )
}

#[tauri::command]
async fn auth_poll(host: State<'_, Host>, flow_id: String) -> Result<Value> {
    let _authorization = host.authorization.change()?;
    let poll = github(&host).await?.poll_login(&flow_id).await?;
    Ok(
        json!({"connected":poll.account.is_some(),"login":poll.account.map(|a|a.login),"message":poll.status,"client_id_configured":true,"retry_after_seconds":poll.retry_after_seconds}),
    )
}

#[tauri::command]
async fn auth_cancel(host: State<'_, Host>, flow_id: String) -> Result<()> {
    github(&host).await?.cancel_login(&flow_id).await
}

#[tauri::command]
async fn auth_logout(host: State<'_, Host>) -> Result<Value> {
    let _authorization = host.authorization.change()?;
    github(&host).await?.logout().await?;
    Ok(auth_view(&host).await)
}

#[tauri::command]
async fn fetch_calendar(host: State<'_, Host>, year: i32) -> Result<Value> {
    let observed = github(&host).await?.calendar_with_cache(Some(year)).await?;
    let snapshot = observed.snapshot;
    let days=snapshot.days.iter().map(|d|json!({"date":d.date,"count":d.count,"level":match d.level.as_deref() { Some("NONE")=>Some(0),Some("FIRST_QUARTILE")=>Some(1),Some("SECOND_QUARTILE")=>Some(2),Some("THIRD_QUARTILE")=>Some(3),Some("FOURTH_QUARTILE")=>Some(4),_=>None }})).collect::<Vec<_>>();
    Ok(
        json!({"year":year,"fetched_at":chrono::DateTime::from_timestamp(snapshot.fetched_at,0).map(|d|d.to_rfc3339()).unwrap_or_default(),"days":days,"partial":snapshot.partial,"coverage_note":snapshot.coverage,"years":snapshot.years,"restricted":snapshot.totals.restricted,"stale":observed.stale,"fetch_error":observed.fetch_error.map(|e|e.message)}),
    )
}

#[tauri::command]
async fn github_repositories(host: State<'_, Host>) -> Result<Value> {
    Ok(serde_json::to_value(
        github(&host).await?.repositories().await?,
    )?)
}

#[tauri::command]
async fn import_github_repository(host: State<'_, Host>, repository_id: u64) -> Result<Value> {
    let _authorization = host.authorization.import()?;
    let imported = github(&host)
        .await?
        .import_repository(repository_id)
        .await?;
    let app = host.app.clone();
    blocking(move || {
        let scan = app.scan(&PathBuf::from(imported.repository_path))?;
        Ok(scan_view(&app, &scan))
    })
    .await
}

#[tauri::command]
async fn fetch_day(host: State<'_, Host>, date: String) -> Result<Value> {
    let observed = github(&host).await?.day_with_cache(&date).await?;
    let mut value = serde_json::to_value(observed.snapshot)?;
    value["stale"] = json!(observed.stale);
    value["fetch_error"] = json!(observed.fetch_error.map(|e| e.message));
    Ok(value)
}

#[tauri::command]
async fn github_identity_evidence(host: State<'_, Host>) -> Result<Value> {
    let emails = github(&host).await?.emails().await?;
    Ok(
        json!({"emails":emails,"fetched_at":chrono::Utc::now().to_rfc3339(),"coverage_note":"Exact addresses returned by GitHub for the connected account. Only verified addresses provide ownership evidence; matching names or inferred aliases do not. This does not prove that a commit contributes to the calendar."}),
    )
}

#[tauri::command]
async fn open_github_device() -> Result<()> {
    // Fixed destination only: never accept a shell command or arbitrary URL from the renderer.
    let status = std::process::Command::new("/usr/bin/open")
        .arg("https://github.com/login/device")
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(CoreError::local(
            "Could not open GitHub authorization in your browser.",
        ))
    }
}

#[tauri::command]
async fn open_github_repository_access() -> Result<()> {
    // Open configuration for user review; this command cannot grant repository access.
    let status = std::process::Command::new("/usr/bin/open")
        .arg("https://github.com/settings/installations")
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(CoreError::local(
            "Could not open GitHub repository access in your browser.",
        ))
    }
}

fn main() {
    if let Some(code) = un_git_core::github::run_credential_helper_if_requested() {
        std::process::exit(code);
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|handle| {
            let app = Application::open(&default_data_dir())?;
            app.recover()?;
            handle.manage(Host {
                app,
                github: Mutex::new(None),
                authorization: authorization::AuthorizationGate::default(),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_dashboard,
            create_demo,
            scan_repository,
            plan_selection,
            prepare_plan,
            get_operation,
            verify_operation,
            cancel_operation,
            preview_report,
            export_report,
            auth_status,
            auth_start,
            auth_poll,
            auth_cancel,
            auth_logout,
            fetch_calendar,
            github_repositories,
            import_github_repository,
            fetch_day,
            github_identity_evidence,
            open_github_device,
            open_github_repository_access
        ])
        .run(tauri::generate_context!())
        .expect("Unable to start un-git");
}
