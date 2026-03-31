use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use std::time::{Duration, Instant};

use data_cli::{parse_keypair, status_command, submit_command, App, ui};

/// Data Fabrication Challenge CLI
#[derive(Parser, Debug)]
#[command(name = "data-cli", about = "Data Fabrication Challenge CLI", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Submit agent code to the validator
    Submit {
        /// Mnemonic or hex seed for signing
        #[arg(long)]
        hotkey: String,

        /// Path to harness directory
        #[arg(long)]
        code: String,

        /// Validator RPC endpoint URL
        #[arg(long, default_value = "http://localhost:9944")]
        endpoint: String,
    },

    /// Check submission status by ID
    Status {
        /// Submission ID to query
        #[arg(long)]
        id: String,

        /// Validator RPC endpoint URL
        #[arg(long, default_value = "http://localhost:9944")]
        endpoint: String,
    },

    /// Launch TUI dashboard monitor
    Monitor,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        None => run_monitor()?,
        Some(Commands::Submit { hotkey, code, endpoint }) => {
            let keypair = parse_keypair(&hotkey)?;
            let harness_dir = PathBuf::from(&code);
            let submission_id = submit_command(&harness_dir, &endpoint, &keypair).await?;
            println!("\nSubmission ID: {}", submission_id);
        }
        Some(Commands::Status { id, endpoint }) => {
            status_command(&id, &endpoint).await?;
        }
        Some(Commands::Monitor) => {
            run_monitor()?;
        }
    }

    Ok(())
}

fn run_monitor() -> Result<()> {
    let mut terminal = ratatui::try_init()?;
    let result = run_tui(&mut terminal);
    ratatui::try_restore()?;
    result
}

fn run_tui(terminal: &mut ratatui::DefaultTerminal) -> Result<()> {
    let mut app = App::new();
    app.refresh();

    let tick_rate = Duration::from_secs(10);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_default();

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => app.should_quit = true,
                        KeyCode::Char('r') => {
                            app.refresh();
                            last_tick = Instant::now();
                        }
                        KeyCode::Up => app.scroll_up(),
                        KeyCode::Down => app.scroll_down(),
                        _ => {}
                    }
                }
            }
        }

        if app.should_quit {
            break;
        }

        if last_tick.elapsed() >= tick_rate {
            app.refresh();
            last_tick = Instant::now();
        }
    }

    Ok(())
}
