use anyhow::Result;
use clap::Parser;
use tycode::{
    ai::{
        bedrock::BedrockProvider,
        types::{Model, ModelTunings},
    },
    cli::CliApp,
};

#[derive(Parser, Debug)]
#[command(name = "tycode-cli")]
#[command(about = "TyCode CLI - Native terminal chat interface")]
struct Args {
    /// Model to use (e.g., claude-sonnet-4, claude-opus-4, etc.)
    #[arg(short, long, default_value = "claude-sonnet-4")]
    model: String,

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
}

async fn setup_provider(args: &Args) -> Result<BedrockProvider> {
    let aws_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .profile_name(&args.profile)
        .region(aws_config::Region::new(args.region.clone()))
        .retry_config(
            aws_config::retry::RetryConfig::adaptive()
                .with_max_attempts(15)
                .with_initial_backoff(std::time::Duration::from_millis(100))
                .with_max_backoff(std::time::Duration::from_secs(1)),
        )
        .load()
        .await;

    let bedrock_client = aws_sdk_bedrockruntime::Client::new(&aws_config);
    Ok(BedrockProvider::new(bedrock_client))
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if let Err(e) = tycode::chat::setup_trace_logging() {
        eprintln!("Warning: Failed to setup trace logging: {:?}", e);
    }

    if std::env::var("RUST_LOG").is_ok() {
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .with_writer(std::io::stderr)
            .init();
    }

    // Parse model
    let model = Model::from_name(&args.model)
        .ok_or_else(|| anyhow::anyhow!("Invalid model: {}", args.model))?;

    // Set up provider
    let provider = setup_provider(&args).await?;

    // Set up system prompt
    let system_prompt =
        "You are a helpful AI assistant. Provide clear, concise, and accurate responses."
            .to_string();

    // Set up tunings
    let tunings = ModelTunings {
        max_tokens: args.max_tokens,
        temperature: args.temperature,
        top_p: args.top_p,
        reasoning_budget: args.reasoning_budget,
    };

    // Validate tunings
    if let Err(e) = tunings.validate() {
        return Err(anyhow::anyhow!("Invalid model tunings: {}", e));
    }

    // Create and run the CLI app
    let mut app = CliApp::new(provider, model, system_prompt, tunings).await?;
    app.run().await?;

    Ok(())
}
