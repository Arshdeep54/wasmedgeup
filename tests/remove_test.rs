use std::path::Path;

use wasmedgeup::{
    api::WasmEdgeApiClient,
    cli::{CommandContext, CommandExecutor},
    commands::remove::RemoveArgs,
};

mod test_utils;

#[tokio::test]
async fn test_remove_single_version() {
    let (_tempdir, test_home) = test_utils::setup_test_environment();

    let version = "0.14.1";
    let version_dir = test_home.join("versions").join(version);
    setup_mock_version(&version_dir, version).await;

    let remove_args = RemoveArgs {
        version: version.to_string(),
        all: false,
        path: Some(test_home.clone()),
    };
    let ctx = CommandContext {
        client: WasmEdgeApiClient::default(),
        no_progress: true,
    };
    remove_args.execute(ctx).await.unwrap();

    assert!(!version_dir.exists(), "Version directory should be removed");
}

#[tokio::test]
async fn test_remove_multiple_versions() {
    let (_tempdir, test_home) = test_utils::setup_test_environment();

    let versions = ["0.14.1", "0.15.0"];
    for version in &versions {
        let version_dir = test_home.join("versions").join(version);
        setup_mock_version(&version_dir, version).await;
    }

    for version in &versions {
        let remove_args = RemoveArgs {
            version: version.to_string(),
            all: false,
            path: Some(test_home.clone()),
        };
        let ctx = CommandContext {
            client: WasmEdgeApiClient::default(),
            no_progress: true,
        };
        remove_args.execute(ctx).await.unwrap();
    }

    for version in &versions {
        let version_dir = test_home.join("versions").join(version);
        assert!(!version_dir.exists(), "Version directory should be removed");
    }
}

#[tokio::test]
async fn test_remove_all_versions() {
    let (_tempdir, test_home) = test_utils::setup_test_environment();

    let versions = ["0.14.1", "0.15.0"];
    for version in &versions {
        let version_dir = test_home.join("versions").join(version);
        setup_mock_version(&version_dir, version).await;
    }

    let remove_args = RemoveArgs {
        version: String::new(),
        all: true,
        path: Some(test_home.clone()),
    };
    let ctx = CommandContext {
        client: WasmEdgeApiClient::default(),
        no_progress: true,
    };
    remove_args.execute(ctx).await.unwrap();

    let versions_dir = test_home.join("versions");
    assert!(
        !versions_dir.exists(),
        "Versions directory should be removed"
    );
}

#[tokio::test]
async fn test_remove_nonexistent_version() {
    let (_tempdir, test_home) = test_utils::setup_test_environment();

    let remove_args = RemoveArgs {
        version: "0.99.99".to_string(),
        all: false,
        path: Some(test_home),
    };
    let ctx = CommandContext {
        client: WasmEdgeApiClient::default(),
        no_progress: true,
    };
    remove_args.execute(ctx).await.unwrap();
}

async fn setup_mock_version(version_dir: &Path, version: &str) {
    let bin_dir = version_dir.join("bin");
    let lib_dir = version_dir.join("lib");
    let include_dir = version_dir.join("include");

    tokio::fs::create_dir_all(&bin_dir).await.unwrap();
    tokio::fs::create_dir_all(&lib_dir).await.unwrap();
    tokio::fs::create_dir_all(&include_dir).await.unwrap();

    tokio::fs::write(bin_dir.join("wasmedge"), format!("mock wasmedge {version}"))
        .await
        .unwrap();
}
