#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
edition = "2024"

[dependencies]
fs_extra = "1"
git2 = "0.20"
hex = "0.4"
sha2 = "0.10"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tempfile = "3"
walkdir = "2"
---

use fs_extra::dir::{copy as copy_dir, CopyOptions};
use git2::*;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use tempfile::tempdir;
use tracing::*;
use tracing_subscriber::EnvFilter;
use walkdir::WalkDir;

const SOURCE_REPO: &str = "https://github.com/dotnet/Open-XML-SDK";
const SOURCE_DIR: &str = "data";
const DESTINATION_DIR: &str = "../crates/ooxmlsdk-build/data";

fn main() -> Result<(), Box<dyn std::error::Error>> {
  tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::new("debug"))
    .init();

  let temp_dir = tempdir()?;

  let current_file_path = std::env::args().next().unwrap();
  let current_file_dir = Path::new(&current_file_path)
    .parent()
    .unwrap()
    .canonicalize()?;
  let source_data_dir = temp_dir.path().join(SOURCE_DIR);
  let destination_data_dir = current_file_dir.join(DESTINATION_DIR).canonicalize()?;

  info!("current_file_dir: {}", current_file_dir.display());
  info!("source_data_dir: {}", source_data_dir.display());
  info!("destination_data_dir: {}", destination_data_dir.display());

  download_github_dir(SOURCE_REPO, temp_dir.path(), SOURCE_DIR)?;

  let source_data_dir_hash = hash_directory(&source_data_dir)?;
  info!("Target hash: {source_data_dir_hash}");

  let destination_data_dir_hash = hash_directory(&destination_data_dir)?;
  info!("Source hash: {destination_data_dir_hash}");

  if destination_data_dir_hash == source_data_dir_hash {
    info!("No change detected. Exiting.");
    return Ok(());
  }

  info!(
    "Copying from ({}) to ({})",
    source_data_dir.display(),
    destination_data_dir.display()
  );
  let _ = fs::remove_dir_all(&destination_data_dir);
  copy_dir(
    source_data_dir,
    destination_data_dir,
    &CopyOptions {
      overwrite: true,
      copy_inside: true,
      ..Default::default()
    },
  )?;

  Ok(())
}

fn download_github_dir(
  url: &str,
  destination_dir: impl AsRef<Path>,
  folder: &str,
) -> Result<(), Box<dyn std::error::Error>> {
  let destination_dir = destination_dir.as_ref();

  info!(
    "Cloning repository ({url}) to ({})",
    destination_dir.display(),
  );

  let repo = Repository::init(destination_dir)?;
  let mut remote = repo.remote("origin", url)?;

  let mut fetch_options = FetchOptions::new();
  fetch_options.depth(1);
  fetch_options.download_tags(AutotagOption::None);

  remote.fetch(&["HEAD"], Some(&mut fetch_options), None)?;

  let mut cfg = repo.config()?;
  cfg.set_bool("core.sparseCheckout", true)?;
  cfg.set_bool("core.sparseCheckoutCone", true).ok();

  fs::create_dir_all(destination_dir.join(".git/info"))?;
  fs::write(
    destination_dir.join(".git/info/sparse-checkout"),
    folder.as_bytes(),
  )?;

  let head = repo.find_reference("FETCH_HEAD")?;
  let id = head.peel_to_commit()?.id();
  repo.checkout_tree(&repo.find_object(id, None)?, None)?;
  repo.set_head_detached(id)?;

  Ok(())
}

fn hash_directory(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
  let mut hasher = Sha256::new();

  if !path.exists() {
    return Ok(String::new());
  }

  for entry in WalkDir::new(path).sort_by_file_name() {
    let entry = entry?;
    if entry.file_type().is_file() {
      let rel = entry.path().strip_prefix(path)?;
      hasher.update(rel.to_string_lossy().as_bytes());
      hasher.update(fs::read(entry.path())?);
    }
  }

  Ok(hex::encode(hasher.finalize()))
}
