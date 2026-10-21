use clap::{Parser, Subcommand};
use std::path::PathBuf;
use un_git_core::{
    engine::{self, Selection},
    operations::Application,
    store::default_data_dir,
    CoreError, Result,
};

#[derive(Parser)]
#[command(
    name = "un-git",
    version,
    about = "Inspect attribution. Prepare and verify local Git metadata changes.",
    long_about = "un-git Inspector. All rewrite preparation is local. Remote publication and restore transport are unavailable until the Writer safety gates pass."
)]
struct Args {
    #[arg(long, global = true)]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Doctor,
    /// Create a clearly synthetic Git repository in app-owned storage.
    Demo,
    /// Read an existing repository without changing its refs or worktree.
    Scan {
        repository: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Select exact commit IDs and explicitly confirm the author email.
    Plan {
        scan_id: String,
        #[arg(long)]
        selection: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Create, restore-test, rewrite, and validate independent local artifacts.
    Prepare {
        plan_id: String,
    },
    InspectOperation {
        id: String,
    },
    Operations,
    /// Revalidate the preserved candidate and backup.
    Verify {
        id: String,
    },
    /// Request safe cancellation at the next local checkpoint.
    Cancel {
        id: String,
    },
    /// Print the redacted support packet for review without exporting a file.
    PreviewReport {
        id: String,
    },
    /// Export the same redacted packet shown by preview-report.
    ExportReport {
        id: String,
    },
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    Calendar {
        #[arg(long)]
        year: Option<i32>,
    },
    Repositories,
    /// Download a fresh read-only snapshot of an installed GitHub repository by ID.
    Import {
        repository_id: u64,
    },
    /// Fetch exact account email evidence; no identity normalization or ownership guesses.
    Identities,
    /// Inspect observed daily aggregates, retaining unavailable/restricted categories.
    Day {
        date: String,
    },
}

#[derive(Subcommand)]
enum AuthCommand {
    Login {
        #[arg(long)]
        client_id: Option<String>,
    },
    Status,
    Logout,
}

fn main() {
    if let Some(code) = un_git_core::github::run_credential_helper_if_requested() {
        std::process::exit(code);
    }
    cli_main();
}

#[tokio::main]
async fn cli_main() {
    let args = Args::parse();
    match run(args).await {
        Ok(value) => println!(
            "{}",
            serde_json::to_string_pretty(&value).expect("JSON value serializes")
        ),
        Err(error) => {
            eprintln!(
                "{}",
                serde_json::to_string(&error).expect("Error serializes")
            );
            std::process::exit(match error.code.as_str() {
                "INVALID" => 2,
                "BLOCKED" => 3,
                "REMOTE_OUTCOME_UNKNOWN" => 4,
                _ => 1,
            });
        }
    }
}

async fn run(args: Args) -> Result<serde_json::Value> {
    if matches!(args.command, Command::Doctor) {
        return Ok(serde_json::to_value(engine::doctor())?);
    }
    let app = Application::open(&args.data_dir.unwrap_or_else(default_data_dir))?;
    app.recover()?;
    let value = match args.command {
        Command::Doctor => unreachable!(),
        Command::Demo => serde_json::json!({"repository_path":app.create_demo()?,"synthetic":true}),
        Command::Scan { repository, .. } => serde_json::to_value(app.scan(&repository)?)?,
        Command::Plan {
            scan_id, selection, ..
        } => {
            let metadata = std::fs::metadata(&selection)?;
            if metadata.len() > 8 * 1024 * 1024 {
                return Err(CoreError::invalid("Selection file exceeds 8 MiB"));
            }
            let selection: Selection = serde_json::from_slice(&std::fs::read(selection)?)?;
            serde_json::to_value(app.plan(&scan_id, &selection)?)?
        }
        Command::Prepare { plan_id } => serde_json::to_value(app.prepare(&plan_id, |op| {
            eprintln!(
                "{}",
                serde_json::json!({"operation_id":op.id,"state":op.state})
            )
        })?)?,
        Command::InspectOperation { id } => {
            serde_json::json!({"operation":app.operation(&id)?,"events":app.store.events(&id)?})
        }
        Command::Operations => serde_json::to_value(app.operations()?)?,
        Command::Verify { id } => serde_json::to_value(app.verify(&id)?)?,
        Command::Cancel { id } => {
            app.cancel(&id)?;
            serde_json::json!({"cancellation_requested":true})
        }
        Command::PreviewReport { id } => app.report_preview(&id)?,
        Command::ExportReport { id } => serde_json::json!({"path":app.export_report(&id)?}),
        Command::Auth { command } => auth(&app, command).await?,
        Command::Calendar { year } => {
            serde_json::to_value(github(&app)?.calendar_with_cache(year).await?)?
        }
        Command::Repositories => serde_json::to_value(github(&app)?.repositories().await?)?,
        Command::Import { repository_id } => {
            serde_json::to_value(github(&app)?.import_repository(repository_id).await?)?
        }
        Command::Identities => serde_json::to_value(github(&app)?.emails().await?)?,
        Command::Day { date } => serde_json::to_value(github(&app)?.day_with_cache(&date).await?)?,
    };
    Ok(value)
}

fn github(app: &Application) -> Result<un_git_core::github::GithubClient> {
    let client_id = std::env::var("UN_GIT_INSPECTOR_CLIENT_ID").ok().or_else(|| app.store.get::<String>("setting","github_client_id").ok()).ok_or_else(|| CoreError::blocked("Configure the public Inspector GitHub App client ID with auth login --client-id. No client secret is needed."))?;
    un_git_core::github::GithubClient::new(app.store.root(), &client_id)
}

async fn auth(app: &Application, command: AuthCommand) -> Result<serde_json::Value> {
    match command {
        AuthCommand::Status => Ok(serde_json::to_value(github(app)?.status().await?)?),
        AuthCommand::Logout => {
            github(app)?.logout().await?;
            Ok(serde_json::json!({"connected":false}))
        }
        AuthCommand::Login { client_id } => {
            if let Some(client_id) = client_id {
                app.store.put("setting", "github_client_id", &client_id)?;
            }
            let client = github(app)?;
            let flow = client.start_login().await?;
            eprintln!(
                "Open {} and enter {}.\nWaiting for account authorization…",
                flow.verification_uri, flow.user_code
            );
            let mut interval = flow.interval_seconds;
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
                let poll = client.poll_login(&flow.session_id).await?;
                if poll.account.is_some() {
                    return Ok(serde_json::to_value(client.status().await?)?);
                }
                interval = poll.retry_after_seconds.max(1);
            }
        }
    }
}
