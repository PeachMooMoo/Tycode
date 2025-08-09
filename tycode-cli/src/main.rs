use anyhow::Result;
use clap::Parser;
use std::sync::Arc;
use tycode_core::{
    ai::{bedrock::BedrockProvider, types::ModelSettings},
    chat,
    settings::SettingsManager,
};

mod app;
mod formatter;

use crate::app::CliApp;

#[derive(Parser, Debug)]
#[command(name = "tycode-cli")]
#[command(about = "TyCode CLI - Native terminal chat interface")]
struct Args {
    /// AWS profile to use
    #[arg(short, long, default_value = "cline")]
    profile: String,

    /// AWS region
    #[arg(short, long, default_value = "us-west-2")]
    region: String,

    /// Temperature for model responses (0.0 to 1.0)
    #[arg(short, long)]
    temperature: Option<f32>,

    /// Maximum number of tokens to generate
    #[arg(long)]
    max_tokens: Option<u32>,

    /// Top-p sampling parameter (0.0 to 1.0)
    #[arg(long)]
    top_p: Option<f32>,

    /// Reasoning budget for models that support it
    #[arg(long)]
    reasoning_budget: Option<u32>,

    /// Disable colors in output
    #[arg(long)]
    no_color: bool,

    /// Don't load settings file
    #[arg(long)]
    no_settings: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if let Err(e) = chat::setup_trace_logging() {
        eprintln!("Warning: Failed to setup trace logging: {:?}", e);
    }

    if std::env::var("RUST_LOG").is_ok() {
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .with_writer(std::io::stderr)
            .init();
    }

    // Load settings if not disabled
    let settings = if !args.no_settings {
        match SettingsManager::new() {
            Ok(mgr) => Some(Arc::new(mgr)),
            Err(e) => {
                eprintln!("Warning: Failed to load settings: {:?}", e);
                None
            }
        }
    } else {
        None
    };

    // Determine AWS profile (CLI args override settings)
    let aws_profile = if args.profile != "cline" {
        args.profile.clone()
    } else if let Some(ref settings_mgr) = settings {
        settings_mgr
            .settings()
            .providers
            .bedrock
            .profile
            .clone()
            .unwrap_or_else(|| "cline".to_string())
    } else {
        "cline".to_string()
    };

    // Use region from CLI args (no settings equivalent for region in new structure)
    let aws_region = args.region.clone();

    // Set up provider with determined values
    let provider = {
        let aws_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .profile_name(&aws_profile)
            .region(aws_config::Region::new(aws_region))
            .retry_config(
                aws_config::retry::RetryConfig::adaptive()
                    .with_max_attempts(15)
                    .with_initial_backoff(std::time::Duration::from_millis(100))
                    .with_max_backoff(std::time::Duration::from_secs(1)),
            )
            .load()
            .await;

        let bedrock_client = aws_sdk_bedrockruntime::Client::new(&aws_config);
        BedrockProvider::new(bedrock_client)
    };

    // Set up tunings (CLI args take precedence)
    let tunings = ModelSettings {
        model: tycode_core::ai::types::Model::default(),
        max_tokens: args.max_tokens,
        temperature: args.temperature,
        top_p: args.top_p,
        reasoning_budget: args.reasoning_budget,
    };

    // Validate tunings
    if let Err(e) = tunings.validate() {
        return Err(anyhow::anyhow!("Invalid model tunings: {}", e));
    }

    // Create and run the CLI app with settings
    let mut app = CliApp::with_settings(provider, tunings, settings).await?;
    app.run().await?;

    Ok(())
}
