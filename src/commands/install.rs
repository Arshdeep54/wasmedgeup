use std::path::PathBuf;

use clap::Parser;
use semver::Version;
use snafu::ResultExt;
use tokio::fs;

use crate::{
    api::{Asset, WasmEdgeApiClient},
    cli::{CommandContext, CommandExecutor},
    prelude::*,
    shell_utils,
    target::{TargetArch, TargetOS},
};

fn default_path() -> PathBuf {
    let home_dir = dirs::home_dir().expect("home_dir should be present");
    home_dir.join(".wasmedge")
}

fn default_tmpdir() -> PathBuf {
    std::env::temp_dir()
}

#[derive(Debug, Parser)]
pub struct InstallArgs {
    /// WasmEdge version to install, e.g. `latest`, `0.14.1`, `0.14.1-rc.1`, etc.
    pub version: String,

    /// Set the install location for the WasmEdge runtime
    ///
    /// Defaults to `$HOME/.wasmedge` on Unix-like systems and `%HOME%\.wasmedge` on Windows.
    #[arg(short, long)]
    pub path: Option<PathBuf>,

    /// Set the temporary directory for staging downloaded assets
    ///
    /// Defaults to the system temporary directory, this differs between operating systems.
    #[arg(short, long)]
    pub tmpdir: Option<PathBuf>,

    /// Set the target OS for the WasmEdge runtime
    ///
    /// `wasmedgeup` will detect the OS of your host system by default.
    #[arg(short, long)]
    pub os: Option<TargetOS>,

    /// Set the target architecture for the WasmEdge runtime
    ///
    /// `wasmedgeup` will detect the architecture of your host system by default.
    #[arg(short, long)]
    pub arch: Option<TargetArch>,
}

impl CommandExecutor for InstallArgs {
    /// Executes the installation process by resolving the version, downloading the asset,
    /// unpacking it, and copying the extracted files to the target directory.
    ///
    /// # Steps:
    /// 1. Resolves the version (either a specific version or the latest).
    /// 2. Downloads the asset for the appropriate OS and architecture.
    /// 3. Unpacks the asset to a temporary directory.
    /// 4. Copies the extracted files to the target directory.
    /// 5. Add the installed bin directory to PATH
    ///
    /// # Arguments
    ///
    /// * `ctx` - The command context containing the client and progress bar settings.
    ///
    /// # Errors
    ///
    /// Returns an error if any step fails, such as download failure, extraction issues,
    /// or copying issues.
    #[tracing::instrument(name = "install", skip_all, fields(version = self.version))]
    async fn execute(mut self, ctx: CommandContext) -> Result<()> {
        let version = self.resolve_version(&ctx.client).inspect_err(
            |e| tracing::error!(error = %e.to_string(), "Failed to resolve version"),
        )?;
        tracing::debug!(%version, "Resolved version for installation");

        let os = self.os.get_or_insert_default();
        let arch = self.arch.get_or_insert_default();
        tracing::debug!(?os, ?arch, "Host OS and architecture detected");

        let asset = Asset::new(&version, os, arch);
        let base_tmpdir = self.tmpdir.unwrap_or_else(default_tmpdir);

        let tmpdir = base_tmpdir.join(&asset.install_name);
        fs::create_dir_all(&tmpdir).await.inspect_err(
            |e| tracing::error!(error = %e.to_string(), "Failed to create temporary directory"),
        )?;
        tracing::debug!(tmpdir = %tmpdir.display(), "Created temporary directory");

        let expected_checksum = ctx
            .client
            .get_release_checksum(&version, &asset)
            .await
            .inspect_err(|e| tracing::error!(error = %e.to_string(), "Failed to get checksum"))?;
        tracing::debug!(%expected_checksum, "Got release checksum");

        let named_file = ctx
            .client
            .download_asset(&asset, &tmpdir, ctx.no_progress)
            .await
            .inspect_err(|e| tracing::error!(error = %e.to_string(), "Failed to download asset"))?;

        let mut file = named_file.into_file();
        WasmEdgeApiClient::verify_file_checksum(&mut file, &expected_checksum)
            .await
            .inspect_err(
                |e| tracing::error!(error = %e.to_string(), "Checksum verification failed"),
            )?;
        tracing::debug!("Checksum verified successfully");

        tracing::debug!(dest = %tmpdir.display(), "Starting extraction of asset");
        crate::fs::extract_archive(&mut file, &tmpdir)
            .await
            .inspect_err(|e| tracing::error!(error = %e.to_string(), "Failed to extract asset"))?;
        tracing::debug!(dest = %tmpdir.display(), "Extraction completed successfully");

        // Copy to final location
        let target_dir = self.path.unwrap_or_else(default_path);
        tracing::debug!(target_dir = %target_dir.display(), "Start copying files to target location");
        crate::fs::copy_tree(&tmpdir, &target_dir).await;
        tracing::debug!(target_dir = %target_dir.display(), "Copying files to target location completed");

        fs::remove_dir_all(&tmpdir).await.inspect_err(
            |e| tracing::error!(error = %e.to_string(), "Failed to clean up temporary directory"),
        )?;
        tracing::debug!(tmpdir = %tmpdir.display(), "Cleaned up temporary directory");

        let install_dir = target_dir.join("bin");
        shell_utils::setup_path(&install_dir)?;

        Ok(())
    }
}

impl InstallArgs {
    fn resolve_version(&self, client: &WasmEdgeApiClient) -> Result<Version> {
        if self.version == "latest" {
            client.latest_release()
        } else {
            Version::parse(&self.version).context(SemVerSnafu {})
        }
    }
}
