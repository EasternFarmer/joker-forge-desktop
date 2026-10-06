use std::{
    fs,
    io::{Cursor, Read},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use reqwest::Client;
use serde::{Deserialize, Serialize};
use zip::ZipArchive;

use super::commands::{
    resolve_appdata_root_from_any_path, resolve_balatro_paths_internal,
    resolve_game_dir_from_any_path, resolve_mods_dir_from_appdata,
};

const LOVELY_REPO: &str = "ethangreen-dev/lovely-injector";
const SMODS_REPO: &str = "Steamodded/smods";
const WINDOWS_ASSET: &str = "lovely-x86_64-pc-windows-msvc.zip";
const MAX_DOWNLOAD: usize = 64 * 1024 * 1024;
const MAX_EXTRACTED: u64 = 256 * 1024 * 1024;
static INSTALLING: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModSetupStatus {
    pub platform: String,
    pub steamodded_installed: bool,
    pub lovely_installed: bool,
    pub can_install: bool,
    pub appdata_path: Option<String>,
    pub game_path: Option<String>,
    pub mods_path: Option<String>,
    pub issues: Vec<String>,
    pub steamodded_version: Option<String>,
    pub lovely_version: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModSetupInstallResult {
    pub status: ModSetupStatus,
    pub steamodded_version: String,
    pub lovely_version: String,
    pub backups: Vec<String>,
}

struct SetupPaths {
    appdata: Option<PathBuf>,
    game: Option<PathBuf>,
}

fn selected_path(
    raw: Option<String>,
    resolve: fn(&str) -> Option<PathBuf>,
    default: Option<PathBuf>,
) -> Option<PathBuf> {
    match raw.filter(|value| !value.trim().is_empty()) {
        Some(value) => resolve(&value),
        None => default,
    }
}

fn setup_paths(appdata_path: Option<String>, game_path: Option<String>) -> SetupPaths {
    let (default_appdata, default_game) = resolve_balatro_paths_internal(None, None, None);
    SetupPaths {
        appdata: selected_path(
            appdata_path,
            resolve_appdata_root_from_any_path,
            default_appdata,
        ),
        game: selected_path(game_path, resolve_game_dir_from_any_path, default_game),
    }
}

fn steamodded_manifest(path: &Path) -> Option<serde_json::Value> {
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(path.join("manifest.json")).ok()?).ok()?;
    let identity = manifest
        .get("name")
        .or_else(|| manifest.get("id"))?
        .as_str()?;
    identity
        .eq_ignore_ascii_case("Steamodded")
        .then_some(manifest)
}

fn steamodded_dirs(mods: &Path) -> Result<Vec<PathBuf>, String> {
    if !mods.exists() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(mods).map_err(|error| format!("Cannot read Mods folder: {error}"))? {
        let path = entry
            .map_err(|error| format!("Cannot read Mods folder: {error}"))?
            .path();
        if steamodded_manifest(&path).is_some() {
            if fs::symlink_metadata(&path)
                .map_err(|error| error.to_string())?
                .file_type()
                .is_symlink()
            {
                return Err("Steamodded is linked to another folder. Update it using the manual installation guide.".into());
            }
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn complete_steamodded(path: &Path) -> bool {
    steamodded_manifest(path).is_some()
        && !path.join(".lovelyignore").exists()
        && ["src", "lovely", "libs"]
            .iter()
            .all(|name| path.join(name).is_dir())
        && path.join("src/core.lua").is_file()
        && path.join("lovely/core.toml").is_file()
}

fn steamodded_version(path: &Path) -> Option<String> {
    for file in ["version.lua", "release.lua"] {
        if let Ok(content) = fs::read_to_string(path.join(file)) {
            let Some(value) = content.trim().strip_prefix("return").map(str::trim) else {
                continue;
            };
            if let Some(quote) = value.chars().next().filter(|ch| matches!(ch, '\'' | '"')) {
                if let Some(value) = value[1..]
                    .split(quote)
                    .next()
                    .filter(|value| !value.is_empty())
                {
                    return Some(value.to_string());
                }
            }
        }
    }
    steamodded_manifest(path)?
        .get("version_number")?
        .as_str()
        .map(str::to_owned)
}

fn lovely_binary(bytes: &[u8]) -> bool {
    if bytes.len() < 64 || &bytes[..2] != b"MZ" {
        return false;
    }
    let offset = u32::from_le_bytes(bytes[60..64].try_into().unwrap()) as usize;
    bytes.get(offset..offset.saturating_add(6)) == Some(b"PE\0\0\x64\x86".as_slice())
        && bytes
            .windows(b"lovely-injector has crashed: ".len())
            .any(|window| window == b"lovely-injector has crashed: ")
}

fn installed_lovely(path: &Path) -> bool {
    fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.len() <= MAX_DOWNLOAD as u64)
        .unwrap_or(false)
        && fs::read(path)
            .map(|bytes| lovely_binary(&bytes))
            .unwrap_or(false)
}

fn inspect_paths(paths: &SetupPaths, platform: &str) -> ModSetupStatus {
    let mut status = ModSetupStatus {
        platform: platform.into(),
        steamodded_installed: false,
        lovely_installed: false,
        can_install: platform == "windows" && paths.appdata.is_some() && paths.game.is_some(),
        appdata_path: paths
            .appdata
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
        game_path: paths
            .game
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
        mods_path: paths.appdata.as_ref().map(|path| {
            resolve_mods_dir_from_appdata(path)
                .to_string_lossy()
                .into_owned()
        }),
        issues: Vec::new(),
        steamodded_version: None,
        lovely_version: None,
    };
    if platform != "windows" {
        status.issues.push("Automatic setup is available on Windows. Follow the Steamodded installation guide for your platform.".into());
    }
    if paths.appdata.is_none() {
        status.issues.push("Choose a valid Balatro AppData folder. Launch Balatro once if its data folder does not exist yet.".into());
    }
    if paths.game.is_none() {
        status
            .issues
            .push("Choose the Balatro game folder containing Balatro.exe.".into());
    }
    if let Some(appdata) = &paths.appdata {
        let mods = resolve_mods_dir_from_appdata(appdata);
        match steamodded_dirs(&mods) {
            Ok(dirs) => {
                if let Some(path) = dirs.iter().find(|path| complete_steamodded(path)) {
                    status.steamodded_installed = true;
                    status.steamodded_version = steamodded_version(path);
                }
                if dirs.len() > 1 {
                    status.steamodded_installed = false;
                    status.issues.push("Multiple Steamodded folders were found. Setup will back them up and install one current copy.".into());
                }
                let target = mods.join("smods");
                if target.exists() && !dirs.contains(&target) {
                    status.can_install = false;
                    status.issues.push("Mods/smods already exists and is not a recognized Steamodded installation. Rename it before setup.".into());
                }
            }
            Err(error) => {
                status.can_install = false;
                status.issues.push(error);
            }
        }
    }
    if let Some(game) = &paths.game {
        if !game.join("Balatro.exe").is_file() {
            status.can_install = false;
            status
                .issues
                .push("The selected game folder must contain Balatro.exe.".into());
        }
        status.lovely_installed = installed_lovely(&game.join("winmm.dll"))
            || installed_lovely(&game.join("version.dll"));
        if game.join("winmm.dll").exists() && game.join("version.dll").exists() {
            status.lovely_installed = false;
            status.issues.push("Both Lovely loader filenames are present. Setup will back them up and remove the legacy version.dll.".into());
        }
        for name in ["winmm.dll", "version.dll"] {
            if let Ok(metadata) = fs::symlink_metadata(game.join(name)) {
                if !metadata.is_file() || metadata.file_type().is_symlink() {
                    status.can_install = false;
                    status.issues.push(format!(
                        "{name} is not a regular file. Resolve it before setup."
                    ));
                } else if !installed_lovely(&game.join(name)) {
                    status.can_install = false;
                    status.issues.push(format!("{name} belongs to an unrecognized loader. Move it out of the game folder before installing Lovely."));
                }
            }
        }
    }
    if !status.steamodded_installed {
        status
            .issues
            .push("Steamodded is missing or incomplete.".into());
    }
    if !status.lovely_installed {
        status.issues.push("Lovely is missing.".into());
    }
    status
}

#[tauri::command]
pub fn inspect_balatro_mod_setup(
    appdata_path: Option<String>,
    game_path: Option<String>,
) -> ModSetupStatus {
    inspect_paths(&setup_paths(appdata_path, game_path), std::env::consts::OS)
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    zipball_url: String,
    #[serde(default)]
    assets: Vec<ReleaseAsset>,
}

#[derive(Deserialize)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
}

async fn download(client: &Client, url: &str) -> Result<Vec<u8>, String> {
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("Download failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Download failed (HTTP {}). Check your connection or try again later.",
            response.status()
        ));
    }
    if response.content_length().unwrap_or(0) > MAX_DOWNLOAD as u64 {
        return Err("Download is larger than the supported limit.".into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("Download failed: {error}"))?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX_DOWNLOAD {
            return Err("Download is larger than the supported limit.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

async fn latest_release(client: &Client, repo: &str) -> Result<Release, String> {
    let bytes = download(
        client,
        &format!("https://api.github.com/repos/{repo}/releases/latest"),
    )
    .await?;
    let release: Release = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Cannot read {repo} release: {error}"))?;
    if release.draft || release.prerelease || release.tag_name.trim().is_empty() {
        return Err(format!("No current official release was found for {repo}."));
    }
    Ok(release)
}

fn archive(bytes: &[u8]) -> Result<ZipArchive<Cursor<&[u8]>>, String> {
    let archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| format!("Invalid ZIP download: {error}"))?;
    if archive.len() > 10_000 {
        return Err("ZIP contains too many files.".into());
    }
    Ok(archive)
}

fn safe_archive_path(name: &str) -> Result<PathBuf, String> {
    let normalized = name.replace('\\', "/");
    let path = PathBuf::from(&normalized);
    if normalized.is_empty()
        || normalized.contains(':')
        || normalized.split('/').any(|part| matches!(part, "." | ".."))
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err("ZIP contains an unsafe file path.".into());
    }
    for part in normalized.split('/').filter(|part| !part.is_empty()) {
        let base = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        if part.ends_with(['.', ' '])
            || matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || (base.len() == 4
                && (base.starts_with("COM") || base.starts_with("LPT"))
                && matches!(base.as_bytes()[3], b'1'..=b'9'))
        {
            return Err("ZIP contains an unsupported Windows file path.".into());
        }
    }
    Ok(path)
}

fn extract_steamodded(bytes: &[u8], destination: &Path) -> Result<PathBuf, String> {
    let mut archive = archive(bytes)?;
    let mut extracted = 0u64;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|error| error.to_string())?;
        let relative = safe_archive_path(file.name())?;
        if file
            .unix_mode()
            .map(|mode| mode & 0o170000 == 0o120000)
            .unwrap_or(false)
        {
            return Err("ZIP contains a symbolic link.".into());
        }
        extracted = extracted.saturating_add(file.size());
        if extracted > MAX_EXTRACTED {
            return Err("Unpacked Steamodded exceeds the supported size limit.".into());
        }
        let target = destination.join(relative);
        if file.is_dir() {
            fs::create_dir_all(target).map_err(|error| error.to_string())?;
        } else {
            fs::create_dir_all(target.parent().ok_or("Invalid ZIP path")?)
                .map_err(|error| error.to_string())?;
            let mut output = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(target)
                .map_err(|error| error.to_string())?;
            let expected_size = file.size();
            let copied = std::io::copy(
                &mut (&mut file).take(expected_size.saturating_add(1)),
                &mut output,
            )
            .map_err(|error| error.to_string())?;
            if copied != expected_size {
                return Err("ZIP file size did not match its contents.".into());
            }
        }
    }
    let mut roots = fs::read_dir(destination)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| complete_steamodded(path));
    let root = roots
        .next()
        .ok_or("Downloaded archive does not contain a complete Steamodded installation.")?;
    if roots.next().is_some() {
        return Err("Downloaded archive contains multiple Steamodded installations.".into());
    }
    Ok(root)
}

fn extract_lovely(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut archive = archive(bytes)?;
    let mut result = None;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|error| error.to_string())?;
        let path = safe_archive_path(file.name())?;
        if path.file_name().and_then(|name| name.to_str()) != Some("winmm.dll") || file.is_dir() {
            continue;
        }
        if result.is_some()
            || file.size() > MAX_DOWNLOAD as u64
            || file
                .unix_mode()
                .map(|mode| mode & 0o170000 == 0o120000)
                .unwrap_or(false)
        {
            return Err("Unexpected Lovely archive contents.".into());
        }
        let mut dll = Vec::new();
        (&mut file)
            .take(MAX_DOWNLOAD as u64 + 1)
            .read_to_end(&mut dll)
            .map_err(|error| error.to_string())?;
        if dll.len() > MAX_DOWNLOAD || !lovely_binary(&dll) {
            return Err("Downloaded winmm.dll is not the expected Windows Lovely injector.".into());
        }
        result = Some(dll);
    }
    result.ok_or_else(|| "The current Lovely Windows release does not contain winmm.dll. Use the manual installation guide.".into())
}

struct TemporaryDirectory(PathBuf);

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn unique_directory(parent: &Path, prefix: &str) -> Result<TemporaryDirectory, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let path = parent.join(format!("{prefix}-{timestamp}-{}", std::process::id()));
    fs::create_dir(&path)
        .map_err(|error| format!("Cannot create setup folder {}: {error}", path.display()))?;
    Ok(TemporaryDirectory(path))
}

// Each replaced item is moved to a backup on its own volume. Restore all moved items on failure.
fn install_files(
    appdata: &Path,
    game: &Path,
    smods_source: &Path,
    dll: &[u8],
) -> Result<Vec<String>, String> {
    if !game.join("Balatro.exe").is_file() {
        return Err("The game folder must contain Balatro.exe.".into());
    }
    let paths = SetupPaths {
        appdata: Some(appdata.to_owned()),
        game: Some(game.to_owned()),
    };
    let status = inspect_paths(&paths, "windows");
    if !status.can_install {
        return Err(status.issues.join(" "));
    }
    let mods = resolve_mods_dir_from_appdata(appdata);
    let old_smods = steamodded_dirs(&mods)?;
    fs::create_dir_all(&mods).map_err(|error| format!("Cannot create Mods folder: {error}"))?;
    let app_backup = unique_directory(appdata, ".jokerforge-mod-backup")?;
    let game_backup = unique_directory(game, ".jokerforge-loader-backup")?;
    let new_loader = game_backup.0.join("new-winmm.dll");
    fs::write(&new_loader, dll)
        .map_err(|error| format!("Cannot write Lovely. Close Balatro and try again: {error}"))?;
    let target = mods.join("smods");
    let mut moved: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut installed_smods = false;
    let mut installed_loader = false;
    let action = (|| -> Result<(), String> {
        for path in old_smods {
            let backup = app_backup
                .0
                .join(path.file_name().ok_or("Invalid Steamodded folder")?);
            fs::rename(&path, &backup).map_err(|error| {
                format!("Cannot back up Steamodded. Close Balatro and try again: {error}")
            })?;
            moved.push((path, backup));
        }
        for name in ["winmm.dll", "version.dll"] {
            let path = game.join(name);
            if path.exists() {
                let backup = game_backup.0.join(name);
                fs::rename(&path, &backup).map_err(|error| {
                    format!("Cannot replace {name}. Close Balatro and try again: {error}")
                })?;
                moved.push((path, backup));
            }
        }
        fs::rename(smods_source, &target)
            .map_err(|error| format!("Cannot install Steamodded: {error}"))?;
        installed_smods = true;
        fs::rename(&new_loader, game.join("winmm.dll"))
            .map_err(|error| format!("Cannot install Lovely: {error}"))?;
        installed_loader = true;
        Ok(())
    })();
    if let Err(error) = action {
        let mut restoration_errors = Vec::new();
        if installed_loader {
            if let Err(error) = fs::remove_file(game.join("winmm.dll")) {
                restoration_errors.push(error.to_string());
            }
        }
        if installed_smods {
            if let Err(error) = fs::remove_dir_all(&target) {
                restoration_errors.push(error.to_string());
            }
        }
        for (path, backup) in moved.iter().rev() {
            if let Err(error) = fs::rename(backup, path) {
                restoration_errors.push(error.to_string());
            }
        }
        if !restoration_errors.is_empty() {
            let app_path = app_backup.0.display().to_string();
            let game_path = game_backup.0.display().to_string();
            std::mem::forget(app_backup);
            std::mem::forget(game_backup);
            return Err(format!("{error} Some originals could not be restored; backups remain at {app_path} and {game_path}: {}", restoration_errors.join("; ")));
        }
        return Err(error);
    }
    let backups: Vec<String> = moved
        .iter()
        .map(|(_, backup)| backup.to_string_lossy().into_owned())
        .collect();
    if moved
        .iter()
        .any(|(_, path)| path.starts_with(&app_backup.0))
    {
        std::mem::forget(app_backup);
    }
    if moved
        .iter()
        .any(|(_, path)| path.starts_with(&game_backup.0))
    {
        std::mem::forget(game_backup);
    }
    Ok(backups)
}

struct InstallGuard;
impl Drop for InstallGuard {
    fn drop(&mut self) {
        INSTALLING.store(false, Ordering::Release);
    }
}

#[tauri::command]
pub async fn install_latest_balatro_mod_setup(
    appdata_path: Option<String>,
    game_path: Option<String>,
) -> Result<ModSetupInstallResult, String> {
    if INSTALLING
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        return Err("Balatro mod setup is already running.".into());
    }
    let _guard = InstallGuard;
    let paths = setup_paths(appdata_path, game_path);
    let status = inspect_paths(&paths, std::env::consts::OS);
    if !status.can_install {
        return Err(status.issues.join(" "));
    }
    let appdata = paths
        .appdata
        .as_ref()
        .ok_or("Missing Balatro AppData folder")?;
    let game = paths.game.as_ref().ok_or("Missing Balatro game folder")?;
    let client = Client::builder()
        .https_only(true)
        .user_agent(concat!("JokerForgeDesktop/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|error| error.to_string())?;
    let smods = latest_release(&client, SMODS_REPO).await?;
    let lovely = latest_release(&client, LOVELY_REPO).await?;
    let asset = lovely
        .assets
        .iter()
        .find(|asset| asset.name == WINDOWS_ASSET)
        .ok_or("The latest Lovely release has no supported Windows ZIP.")?;
    if !smods.zipball_url.starts_with(&format!(
        "https://api.github.com/repos/{SMODS_REPO}/zipball/"
    )) || !asset.browser_download_url.starts_with(&format!(
        "https://github.com/{LOVELY_REPO}/releases/download/"
    )) {
        return Err("The official release returned an unexpected download URL.".into());
    }
    let smods_bytes = download(&client, &smods.zipball_url).await?;
    let lovely_bytes = download(&client, &asset.browser_download_url).await?;
    let dll = extract_lovely(&lovely_bytes)?;
    let staging = unique_directory(appdata, ".jokerforge-setup")?;
    let smods_source = extract_steamodded(&smods_bytes, &staging.0)?;
    let backups = install_files(appdata, game, &smods_source, &dll)?;
    let mut status = inspect_paths(&paths, std::env::consts::OS);
    status.steamodded_version = Some(smods.tag_name.clone());
    status.lovely_version = Some(lovely.tag_name.clone());
    Ok(ModSetupInstallResult {
        status,
        steamodded_version: smods.tag_name,
        lovely_version: lovely.tag_name,
        backups,
    })
}
