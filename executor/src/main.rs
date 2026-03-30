use clap::Parser;
use data_executor::{ExecutionResult, PythonExecutor};
use std::path::PathBuf;
use std::process::ExitCode;

/// Execute Python harness for dataset generation
#[derive(Parser)]
#[command(name = "data-executor")]
#[command(about = "Execute Python harness for dataset generation", long_about = None)]
struct Args {
    /// Path to Python harness file
    #[arg(long)]
    harness: PathBuf,

    /// Timeout in seconds (default: 3600, max: 7200)
    #[arg(short, long, default_value = "3600")]
    timeout: u64,

    /// Print verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_max_level(if args.verbose {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        })
        .with_target(false)
        .init();

    let code = match std::fs::read_to_string(&args.harness) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading harness file '{}': {}", args.harness.display(), e);
            return ExitCode::from(1);
        }
    };

    if args.verbose {
        println!("Loaded harness from: {}", args.harness.display());
        println!("Timeout: {} seconds", args.timeout.min(7200));
    }

    let executor = PythonExecutor::with_timeout(args.timeout);

    match executor.execute(&code).await {
        Ok(result) => {
            print_result(&result, args.verbose);
            if result.exit_code == Some(0) {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(e) => {
            eprintln!("Execution error: {}", e);
            ExitCode::from(1)
        }
    }
}

fn print_result(result: &ExecutionResult, verbose: bool) {
    println!("Exit code: {:?}", result.exit_code);
    println!("Duration: {}ms", result.duration_ms);

    if verbose {
        println!("Timed out: {}", result.timed_out);
    }

    if !result.stdout.is_empty() {
        println!("--- STDOUT ---");
        println!("{}", result.stdout);
    }

    if !result.stderr.is_empty() {
        println!("--- STDERR ---");
        eprintln!("{}", result.stderr);
    }
}
