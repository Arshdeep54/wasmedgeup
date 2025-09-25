use std::path::PathBuf;

use clap::Parser;
use tokio::fs;

use crate::{
    cli::{CommandContext, CommandExecutor},
    commands::{default_path, use_cmd::UseArgs},
    prelude::*,
};

#[derive(Debug, Parser)]
pub struct RemoveArgs {
    /// WasmEdge version to remove, e.g. `0.13.0`, `0.15.0`, etc.
    #[arg(default_value = "")]
    pub version: String,

    /// Remove all installed versions
    #[arg(long)]
    pub all: bool,

    /// Set the install location for the WasmEdge runtime
    ///
    /// Defaults to `$HOME/.wasmedge` on Unix-like systems and `%HOME%\.wasmedge` on Windows.
    #[arg(short, long)]
    pub path: Option<PathBuf>,
}

impl CommandExecutor for RemoveArgs {
    async fn execute(self, ctx: CommandContext) -> Result<()> {
        let target_dir = self.path.unwrap_or_else(default_path);
        let versions_dir = target_dir.join("versions");

        if !versions_dir.exists() {
            tracing::debug!("No versions directory found");
            return Ok(());
        }

        if !self.all && self.version.is_empty() {
            return Err(Error::Unknown);
        }

        let current_version = if target_dir.join("bin").exists() {
            let bin_link = fs::read_link(target_dir.join("bin")).await?;
            tracing::debug!(link = ?bin_link, "Raw symlink path");

            let link_str = bin_link.to_str().ok_or_else(|| Error::Unknown)?;
            tracing::debug!(link_str = %link_str, "Symlink as string");

            if let Some(version_path) = link_str.strip_prefix("versions/") {
                let version = version_path
                    .split('/')
                    .next()
                    .ok_or_else(|| Error::Unknown)?;
                tracing::debug!(version = %version, "Extracted version");
                Some(version.to_string())
            } else {
                tracing::debug!("No version prefix found");
                None
            }
        } else {
            tracing::debug!("No bin symlink found");
            None
        };

        if self.all {
            tracing::debug!("Removing all installed versions");
            fs::remove_dir_all(&target_dir).await?;
            tracing::info!("All versions and configuration removed successfully");
            return Ok(());
        }

        let version = ctx.client.resolve_version(&self.version).inspect_err(
            |e| tracing::error!(error = %e.to_string(), "Failed to resolve version"),
        )?;
        tracing::debug!(%version, "Resolved version for use");

        let version_dir = versions_dir.join(version.to_string());
        if version_dir.exists() {
            fs::remove_dir_all(&version_dir).await?;
            tracing::info!(version = %version, "Version removed successfully");
        }

        let removed_current = Some(version.to_string()) == current_version;

        let mut remaining_versions = 0;
        let mut dir_stream = fs::read_dir(&versions_dir).await?;
        while let Some(entry) = dir_stream.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                remaining_versions += 1;
            }
        }

        if remaining_versions == 0 {
            tracing::debug!("No versions remaining, cleaning up configuration");
            fs::remove_dir_all(&target_dir).await?;
            tracing::info!("All versions and configuration removed successfully");
            return Ok(());
        }

        if removed_current && remaining_versions > 0 {
            tracing::debug!(removed_version = ?current_version, "Current version was removed");

            let mut installed_versions = Vec::new();
            let mut dir_entries = fs::read_dir(&versions_dir).await?;
            while let Some(entry) = dir_entries.next_entry().await? {
                if entry.file_type().await?.is_dir() {
                    if let Some(version_str) = entry.file_name().to_str() {
                        installed_versions.push(version_str.to_string());
                    }
                }
            }

            installed_versions.sort_by(|a, b| b.cmp(a));
            let latest_version = installed_versions.first().cloned();

            if let Some(version) = latest_version {
                tracing::info!(version = %version, "Switching to latest version");
                let use_args = UseArgs {
                    version,
                    path: Some(target_dir),
                };
                use_args.execute(ctx).await?;
            } else {
                tracing::warn!("No other versions found to switch to");
            }
        }

        Ok(())
    }
}
