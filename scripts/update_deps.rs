#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
edition = "2024"

[dependencies]
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
---

use std::{path::Path, process::Command};
use tracing::*;
use tracing_subscriber::EnvFilter;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new("debug"))
        .init();

    let current_file_path = std::env::args().next().unwrap();
    let current_file_dir = Path::new(&current_file_path)
        .parent()
        .unwrap()
        .canonicalize()?;
    let workspace_dir = current_file_dir.parent().unwrap().canonicalize()?;

    info!("current_file_dir: {}", current_file_dir.display());
    info!("workspace_dir: {}", workspace_dir.display());

    update_cargo_deps(workspace_dir)?;

    Ok(())
}

fn update_cargo_deps(current_dir: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
    assert!(
        run_command(
            Command::new("cargo")
                .args(["upgrade", "--verbose", "--verbose"])
                .current_dir(&current_dir)
        )?
        .status
        .success()
    );

    assert!(
        run_command(
            Command::new("cargo")
                .args(["update"])
                .current_dir(&current_dir)
        )?
        .status
        .success()
    );

    Ok(())
}

fn run_command(command: &mut Command) -> Result<std::process::Output, Box<dyn std::error::Error>> {
    debug!(
        "Starting command: ({} {})",
        command.get_program().to_string_lossy(),
        command
            .get_args()
            .map(|a| a.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ")
    );

    let output = command.output()?;

    debug!(
        "Command finished with status code: ({})\n=== stdout ===\n{}=== stderr ===\n{}",
        output.status.code().unwrap_or_default(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    Ok(output)
}
