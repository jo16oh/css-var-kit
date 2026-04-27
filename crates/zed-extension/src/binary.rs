use zed_extension_api::{self as zed, LanguageServerId, Result, Worktree};

use crate::CssVarKitExtension;

const REPO: &str = "jo16oh/css-var-kit";

pub fn resolve(
    ext: &mut CssVarKitExtension,
    id: &LanguageServerId,
    worktree: &Worktree,
) -> Result<String> {
    if let Some(path) = worktree.which(bin_name()) {
        return Ok(path);
    }

    if let Some(path) = ext.cached_binary_path.as_deref()
        && std::fs::metadata(path).is_ok_and(|m| m.is_file())
    {
        return Ok(path.to_string());
    }

    let path = download(id)?;
    ext.cached_binary_path = Some(path.clone());
    Ok(path)
}

fn download(id: &LanguageServerId) -> Result<String> {
    zed::set_language_server_installation_status(
        id,
        &zed::LanguageServerInstallationStatus::CheckingForUpdate,
    );

    let release = zed::latest_github_release(
        REPO,
        zed::GithubReleaseOptions {
            require_assets: true,
            pre_release: false,
        },
    )?;

    let asset_name = asset_name()?;
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == asset_name)
        .ok_or_else(|| format!("no release asset matching {asset_name}"))?;

    let version_dir = format!("css-var-kit-{}", release.version);
    let bin_path = format!("{version_dir}/{}", bin_name());

    if std::fs::metadata(&bin_path).is_ok_and(|m| m.is_file()) {
        return Ok(bin_path);
    }

    zed::set_language_server_installation_status(
        id,
        &zed::LanguageServerInstallationStatus::Downloading,
    );

    zed::download_file(
        &asset.download_url,
        &version_dir,
        zed::DownloadedFileType::GzipTar,
    )
    .map_err(|e| format!("failed to download {asset_name}: {e}"))?;

    zed::make_file_executable(&bin_path)?;
    cleanup_old_versions(&version_dir);

    Ok(bin_path)
}

fn bin_name() -> &'static str {
    match zed::current_platform().0 {
        zed::Os::Windows => "cvk.exe",
        _ => "cvk",
    }
}

fn asset_name() -> Result<String> {
    let (os, arch) = zed::current_platform();
    let os_part = match os {
        zed::Os::Mac => "darwin",
        zed::Os::Linux => "linux",
        zed::Os::Windows => "win32",
    };
    let arch_part = match arch {
        zed::Architecture::Aarch64 => "arm64",
        zed::Architecture::X8664 => "x64",
        zed::Architecture::X86 => return Err("unsupported architecture: x86".into()),
    };
    Ok(format!("css-var-kit-{os_part}-{arch_part}.tar.gz"))
}

fn cleanup_old_versions(current: &str) {
    let Ok(entries) = std::fs::read_dir(".") else {
        return;
    };
    entries
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| n.starts_with("css-var-kit-") && n != current)
        })
        .for_each(|e| {
            let _ = std::fs::remove_dir_all(e.path());
        });
}
