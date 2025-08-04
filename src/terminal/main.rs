use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{
    io,
    time::{Duration, Instant},
};
use tracing_appender;
use tracing_subscriber;
use tycode::{
    ai::{
        bedrock::BedrockProvider,
        types::{Model, ModelTunings},
    },
    terminal::App,
    timing::Timer,
};

#[derive(Parser)]
#[command(name = "tycode-chat")]
#[command(about = "TyCode AI chat terminal with TUI")]
struct Args {
    #[arg(short, long, default_value = "claude-sonnet-4")]
    model: String,

    #[arg(short, long, default_value = "cline")]
    profile: String,

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
}

async fn setup_provider(args: &Args) -> Result<BedrockProvider> {
    let profile_name = args.profile.clone();

    let aws_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .profile_name(&profile_name)
        .region(aws_config::Region::new(args.region.clone()))
        .load()
        .await;

    let bedrock_client = aws_sdk_bedrockruntime::Client::new(&aws_config);
    Ok(BedrockProvider::new(bedrock_client))
}

async fn create_app(args: Args) -> Result<App> {
    let model = Model::from_name(&args.model)
        .ok_or_else(|| anyhow::anyhow!("Invalid model: {}", args.model))?;

    let provider = setup_provider(&args).await?;

    let system_prompt =
        "You are a helpful AI assistant. Provide clear, concise, and accurate responses."
            .to_string();

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

    App::new(provider, model, system_prompt, tunings).await
}

async fn run_main_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    let mut last_tick = Instant::now();
    let mut last_log = Instant::now();
    let tick_rate = Duration::from_millis(50);
    let mut timer = Timer::new();

    loop {
        timer.start("total_tick");

        timer.start("render");
        terminal.draw(|f| app.render(f))?;
        timer.end("render");

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());

        timer.start("event_poll");
        let has_event = event::poll(timeout)?;
        timer.end("event_poll");

        if has_event {
            timer.start("event_read");
            let event = event::read()?;
            timer.end("event_read");

            match event {
                Event::Key(key) => {
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        break;
                    }
                    timer.start("handle_key");
                    app.handle_key(key);
                    timer.end("handle_key");
                }
                _ => {}
            }
        }

        // Actor handles messages asynchronously now

        timer.start("check_splash");
        app.check_splash_timeout();
        timer.end("check_splash");

        timer.end("total_tick");

        if last_log.elapsed() >= Duration::from_secs(1) {
            timer.log_all("total_tick", 50);
            timer = Timer::new();
            last_log = Instant::now();
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if let Err(e) = tycode::chat::setup_trace_logging() {
        eprintln!("Warning: Failed to setup trace logging: {:?}", e);
    }

    // Initialize tracing subscriber only if RUST_LOG is set
    if std::env::var("RUST_LOG").is_ok() {
        let log_dir = std::env::var("TYCODE_LOG_DIR").unwrap_or_else(|_| {
            dirs::cache_dir()
                .map(|d| d.join("tycode").to_string_lossy().to_string())
                .unwrap_or_else(|| "./logs".to_string())
        });

        // Create log directory if it doesn't exist
        std::fs::create_dir_all(&log_dir)?;

        let log_file = std::path::Path::new(&log_dir).join("tycode-chat.log");
        let file_appender = tracing_appender::rolling::never(&log_dir, "tycode-chat.log");

        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .with_writer(file_appender)
            .with_file(true)
            .with_line_number(true)
            .with_target(true)
            .with_ansi(false) // No colors in file
            .init();

        eprintln!("📝 Tracing enabled, logging to: {}", log_file.display());
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = create_app(args).await?;
    let result = run_main_loop(&mut terminal, &mut app).await;

    // Shut down the actor before exiting
    app.shutdown().await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = result {
        println!("Error: {:?}", err);
    }

    Ok(())
}
