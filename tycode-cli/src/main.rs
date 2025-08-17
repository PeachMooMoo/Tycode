use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tycode_core::chat;

mod base_app;
mod event_handler;
mod formatter;
mod interactive_app;
mod subprocess;
mod subprocess_app;

use crate::interactive_app::InteractiveApp;
use crate::subprocess_app::SubprocessApp;

#[derive(Parser, Debug)]
#[command(name = "tycode-cli")]
#[command(about = "TyCode CLI - Native terminal chat interface")]
struct Args {
    /// Run in subprocess mode for VSCode extension
    #[arg(long)]
    subprocess: bool,

    /// Workspace roots (for multi-root workspaces)
    #[arg(long, value_delimiter = ',')]
    workspace_roots: Option<Vec<String>>,
    
    /// Path to the settings file (for testing or custom configurations)
    #[arg(long)]
    settings_path: Option<PathBuf>,
}

fn main() -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async {
        let local = tokio::task::LocalSet::new();
        local.run_until(async_main()).await
    })
}

async fn async_main() -> Result<()> {
    let args = Args::parse();

    if !args.subprocess {
        if let Err(e) = chat::setup_trace_logging() {
            eprintln!("Warning: Failed to setup trace logging: {:?}", e);
        }

        if std::env::var("RUST_LOG").is_ok() {
            tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
                .with_writer(std::io::stderr)
                .init();
        }
    }

    let workspace_roots = args
        .workspace_roots
        .map(|roots| -> Result<Vec<PathBuf>> {
            roots
                .into_iter()
                .map(|root| {
                    let path = PathBuf::from(root);
                    path.canonicalize().map_err(|e| {
                        anyhow::anyhow!("Failed to canonicalize workspace root {:?}: {}", path, e)
                    })
                })
                .collect()
        })
        .transpose()?;

    if args.subprocess {
        let mut app = SubprocessApp::new(workspace_roots, args.settings_path).await?;
        app.run().await?;
    } else {
        let mut app = InteractiveApp::new(workspace_roots, args.settings_path).await?;
        app.run().await?;
    }

    Ok(())
}
