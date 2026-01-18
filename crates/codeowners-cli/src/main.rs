//! CODEOWNERS Validator CLI
//!
//! A command-line tool for validating GitHub CODEOWNERS files.

use clap::Parser;
use std::io::{self, IsTerminal, Write};
use std::process::ExitCode as StdExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::signal;
use tracing::{debug, error, info, warn, Level};
use tracing_subscriber::EnvFilter;

mod cli;

use cli::config::{create_octocrab, ExitCode, ValidatedConfig};
use cli::github::OctocrabClient;
use cli::output::{HumanOutput, ValidationResults};
use cli::{Args, CheckKind, ExperimentalCheckKind};
use codeowners_validator_core::parse::parse_codeowners;
use codeowners_validator_core::validate::checks::{
    AvoidShadowingCheck, Check, CheckContext, DupPatternsCheck, FilesCheck, NotOwnedCheck,
    SyntaxCheck,
};

#[tokio::main]
async fn main() -> StdExitCode {
    // Parse command-line arguments
    let args = Args::parse();

    // Initialize tracing
    init_tracing(args.verbose, args.json);

    // Set up signal handling for graceful shutdown
    let terminated = Arc::new(AtomicBool::new(false));
    let terminated_clone = terminated.clone();

    tokio::spawn(async move {
        let ctrl_c = signal::ctrl_c();
        #[cfg(unix)]
        let terminate = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("failed to install signal handler")
                .recv()
                .await;
        };
        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {
                info!("Received SIGINT, shutting down...");
            }
            _ = terminate => {
                info!("Received SIGTERM, shutting down...");
            }
        }

        terminated_clone.store(true, Ordering::SeqCst);
    });

    // Run the validator
    let exit_code = run(args, &terminated).await;

    // Check if we were terminated by signal
    if terminated.load(Ordering::SeqCst) {
        return StdExitCode::from(ExitCode::Terminated as u8);
    }

    StdExitCode::from(i32::from(exit_code) as u8)
}

/// Initialize tracing based on verbosity level.
fn init_tracing(verbosity: u8, json_output: bool) {
    // Don't output logs when using JSON output mode
    if json_output {
        return;
    }

    let level = match verbosity {
        0 => Level::WARN,
        1 => Level::INFO,
        2 => Level::DEBUG,
        _ => Level::TRACE,
    };

    let filter = EnvFilter::from_default_env()
        .add_directive(level.into())
        .add_directive("octocrab=warn".parse().unwrap())
        .add_directive("hyper=warn".parse().unwrap())
        .add_directive("reqwest=warn".parse().unwrap());

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_ansi(io::stderr().is_terminal())
        .init();
}

/// Run the validator with the given arguments.
async fn run(args: Args, terminated: &AtomicBool) -> ExitCode {
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();

    // Validate configuration
    let config = match ValidatedConfig::from_args(&args) {
        Ok(config) => config,
        Err(e) => {
            let use_colors = !args.json && io::stdout().is_terminal();
            write_error(&mut stderr, &e.to_string(), use_colors);
            return ExitCode::StartupFailure;
        }
    };

    let use_colors = !config.json_output && io::stdout().is_terminal();

    debug!("Validated configuration: {:?}", config);
    info!("Repository path: {}", config.repo_path.display());
    info!("CODEOWNERS file: {}", config.codeowners_path.display());

    // Read and parse CODEOWNERS file
    let codeowners_content = match std::fs::read_to_string(&config.codeowners_path) {
        Ok(content) => content,
        Err(e) => {
            write_error(
                &mut stderr,
                &format!(
                    "Failed to read CODEOWNERS file '{}': {}",
                    config.codeowners_path.display(),
                    e
                ),
                use_colors,
            );
            return ExitCode::StartupFailure;
        }
    };

    let parse_result = parse_codeowners(&codeowners_content);

    if !parse_result.is_ok() {
        // Report parse errors
        if config.json_output {
            let mut results = ValidationResults::new();
            let validation_result = codeowners_validator_core::ValidationResult::new();
            for error in &parse_result.errors {
                // Convert parse errors to a simple message for now
                warn!("Parse error: {}", error);
            }
            results.add("parse", validation_result);
            if let Err(e) = results.write_json(&mut stdout) {
                error!("Failed to write JSON output: {}", e);
            }
        } else {
            let mut output = HumanOutput::new(&mut stderr, use_colors);
            let _ = output.write_error("Failed to parse CODEOWNERS file");
            for error in &parse_result.errors {
                let _ = writeln!(stderr, "  {}", error);
            }
        }
        return ExitCode::ValidationFailed;
    }

    // Check for termination
    if terminated.load(Ordering::SeqCst) {
        return ExitCode::Terminated;
    }

    // Create GitHub client if needed
    let octocrab = if config.checks.contains(&CheckKind::Owners) {
        match create_octocrab(&args).await {
            Ok(client) => client.map(OctocrabClient::new),
            Err(e) => {
                write_error(&mut stderr, &e.to_string(), use_colors);
                return ExitCode::StartupFailure;
            }
        }
    } else {
        None
    };

    // Run validation checks
    let mut results = ValidationResults::new();
    let ctx = CheckContext::new(&parse_result.ast, &config.repo_path, &config.check_config);

    // Run standard checks
    for check_kind in &config.checks {
        if terminated.load(Ordering::SeqCst) {
            return ExitCode::Terminated;
        }

        let (name, result) = match check_kind {
            CheckKind::Syntax => {
                info!("Running syntax check...");
                ("syntax", SyntaxCheck::new().run(&ctx))
            }
            CheckKind::Duppatterns => {
                info!("Running duplicate patterns check...");
                ("duppatterns", DupPatternsCheck::new().run(&ctx))
            }
            CheckKind::Files => {
                info!("Running files check...");
                ("files", FilesCheck::new().run(&ctx))
            }
            CheckKind::Owners => {
                if let Some(ref octo) = octocrab {
                    info!("Running owners check...");
                    use codeowners_validator_core::validate::checks::{
                        AsyncCheck, AsyncCheckContext, OwnersCheck,
                    };
                    let async_ctx = AsyncCheckContext::new(
                        &parse_result.ast,
                        &config.repo_path,
                        &config.check_config,
                        octo,
                    );
                    ("owners", OwnersCheck::new().run(&async_ctx).await)
                } else {
                    warn!("Skipping owners check: no GitHub authentication configured");
                    continue;
                }
            }
        };

        debug!("Check '{}' found {} issue(s)", name, result.errors.len());
        results.add(name, result);
    }

    // Run experimental checks
    for check_kind in &config.experimental_checks {
        if terminated.load(Ordering::SeqCst) {
            return ExitCode::Terminated;
        }

        let (name, result) = match check_kind {
            ExperimentalCheckKind::Notowned => {
                info!("Running not-owned check (experimental)...");
                ("notowned", NotOwnedCheck::new().run(&ctx))
            }
            ExperimentalCheckKind::AvoidShadowing => {
                info!("Running avoid-shadowing check (experimental)...");
                ("avoid-shadowing", AvoidShadowingCheck::new().run(&ctx))
            }
        };

        debug!("Check '{}' found {} issue(s)", name, result.errors.len());
        results.add(name, result);
    }

    // Output results
    if config.json_output {
        if let Err(e) = results.write_json(&mut stdout) {
            error!("Failed to write JSON output: {}", e);
            return ExitCode::StartupFailure;
        }
    } else {
        if let Err(e) = results.write_human(&mut stdout, use_colors) {
            error!("Failed to write output: {}", e);
            return ExitCode::StartupFailure;
        }
    }

    // Determine exit code
    config.exit_code_for_results(results.has_errors(), results.has_warnings())
}

/// Write an error message to the writer.
fn write_error<W: Write>(writer: &mut W, message: &str, use_colors: bool) {
    if use_colors {
        let _ = writeln!(writer, "\x1b[1;31mError:\x1b[0m {}", message);
    } else {
        let _ = writeln!(writer, "Error: {}", message);
    }
}
