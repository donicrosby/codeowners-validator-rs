//! Generate CODEOWNERS fixtures for benchmarking.
//!
//! Usage: cargo run --release --bin generate-fixtures --features generate -- [output_dir]
//!
//! Generates deterministic fixtures using the same presets as the Rust benchmarks.

use codeowners_validator_core::generate::{GeneratorConfig, generate};
use std::{fs, io, path::PathBuf, process::ExitCode};

/// Type alias for fixture preset entries.
type PresetEntry = (&'static str, fn() -> GeneratorConfig);

/// Fixture presets - keep in sync with benches/fixtures.rs
const PRESETS: &[PresetEntry] = &[
    ("small", GeneratorConfig::small),
    ("medium", GeneratorConfig::medium),
    ("large", GeneratorConfig::large),
    ("xlarge", GeneratorConfig::xlarge),
];

fn main() -> ExitCode {
    let output_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("benches/cli/fixtures"));

    if let Err(e) = run(&output_dir) {
        eprintln!("Error: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run(output_dir: &PathBuf) -> io::Result<()> {
    fs::create_dir_all(output_dir)?;

    for (name, config_fn) in PRESETS {
        let config = config_fn();
        let content = generate(&config);
        let path = output_dir.join(format!("{name}.codeowners"));
        fs::write(&path, &content)?;
        println!(
            "Generated {} ({} bytes, {} rules, {} comments)",
            path.display(),
            content.len(),
            config.num_rules,
            config.num_comments
        );
    }

    Ok(())
}
