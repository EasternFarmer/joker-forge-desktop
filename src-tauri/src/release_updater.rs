//! The automatic updater only handles Windows NSIS installers from our releases.
//! Other platforms use the release page and their normal installation tools.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::Child,
    sync::{Mutex, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager};

const MAX_INSTALLER_SIZE: u64 = 512 * 1024 * 1024;
const MANIFEST: &str = "download.json";

struct HelperHandoff {
    child: Option<Child>,
    cancelled: bool,
}

fn helper_handoffs() -> &'static Mutex<HashMap<PathBuf, HelperHandoff>> {
    static HELPERS: OnceLock<Mutex<HashMap<PathBuf, HelperHandoff>>> = OnceLock::new();
    HELPERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn register_helper(directory: &Path, child: Child) {
    let mut handoffs = helper_handoffs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    handoffs.retain(|_, handoff| {
        handoff
            .child
            .as_mut()
            .is_some_and(|child| !matches!(child.try_wait(), Ok(Some(_))))
    });
    handoffs.insert(
        directory.to_path_buf(),
        HelperHandoff {
            child: Some(child),
            cancelled: false,
        },
    );
}

fn stop_registered_helper(directory: &Path) -> Result<bool, String> {
    let mut handoffs = helper_handoffs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(handoff) = handoffs.get_mut(directory) else {
        return Ok(false);
    };
    if let Some(child) = handoff.child.as_mut() {
        if child
            .try_wait()
            .map_err(|error| format!("Failed to check the update helper: {error}"))?
            .is_none()
        {
            child
                .kill()
                .map_err(|error| format!("Failed to stop the update helper: {error}"))?;
        }
        child
            .wait()
            .map_err(|error| format!("Failed to wait for the update helper to stop: {error}"))?;
    }
    handoff.child = None;
    handoff.cancelled = true;
    Ok(true)
}

fn helper_was_cancelled(directory: &Path) -> bool {
    helper_handoffs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(directory)
        .is_some_and(|handoff| handoff.cancelled)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePlatform {
    os: &'static str,
    arch: &'static str,
    automatic_install: bool,
}

#[tauri::command]
pub fn get_update_platform() -> UpdatePlatform {
    UpdatePlatform {
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        automatic_install: supports_automatic_install(std::env::consts::OS),
    }
}

fn supports_automatic_install(os: &str) -> bool {
    os == "windows"
}

fn require_windows() -> Result<(), String> {
    if supports_automatic_install(std::env::consts::OS) {
        Ok(())
    } else {
        Err("Automatic installation is only available on Windows. Open the release page to update this platform.".into())
    }
}

fn validate_file_name(name: &str) -> Result<(), String> {
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end()
        .to_ascii_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str())
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && matches!(stem.as_bytes()[3], b'1'..=b'9'));
    if name.len() > 200
        || name.is_empty()
        || name.trim() != name
        || !name.to_ascii_lowercase().ends_with(".exe")
        || name.starts_with('.')
        || reserved
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._- ()".contains(&byte))
    {
        return Err("The update must be a plain Windows installer file name.".into());
    }
    Ok(())
}

fn validate_asset_url(raw: &str, file_name: &str) -> Result<reqwest::Url, String> {
    validate_file_name(file_name)?;
    let url = reqwest::Url::parse(raw).map_err(|_| "Invalid release asset URL.")?;
    let segments: Vec<_> = url.path_segments().into_iter().flatten().collect();
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || segments.len() != 6
        || !segments[0].eq_ignore_ascii_case("Jaydchw")
        || !segments[1].eq_ignore_ascii_case("joker-forge-desktop")
        || segments[2..4] != ["releases", "download"]
        || segments[4].is_empty()
    {
        return Err(
            "Only assets from official Joker Forge Desktop releases can be downloaded.".into(),
        );
    }
    // Match decoded bytes so equivalent space/parenthesis encodings are accepted.
    if decoded_asset_name(segments[5]).as_deref() != Some(file_name) {
        return Err("The release asset URL does not match the installer file name.".into());
    }
    Ok(url)
}

fn decoded_asset_name(segment: &str) -> Option<String> {
    if segment.len() > 600 {
        return None;
    }
    let mut bytes = segment.bytes();
    let mut decoded = Vec::new();
    while let Some(byte) = bytes.next() {
        decoded.push(if byte == b'%' {
            let high = char::from(bytes.next()?).to_digit(16)?;
            let low = char::from(bytes.next()?).to_digit(16)?;
            (high * 16 + low) as u8
        } else {
            byte
        });
    }
    String::from_utf8(decoded).ok()
}

fn allowed_download_host(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && matches!(
            url.host_str(),
            Some(
                "github.com"
                    | "release-assets.githubusercontent.com"
                    | "objects.githubusercontent.com"
            )
        )
}

fn parse_expected_digest(digest: Option<&str>) -> Result<Option<String>, String> {
    digest
        .map(|value| {
            let hash = value
                .strip_prefix("sha256:")
                .ok_or("Unsupported release asset digest.")?;
            if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err("Invalid release asset SHA256 digest.".into());
            }
            Ok(hash.to_ascii_lowercase())
        })
        .transpose()
}

fn validate_expected_size(size: Option<u64>) -> Result<(), String> {
    if size.is_some_and(|value| value == 0 || value > MAX_INSTALLER_SIZE) {
        return Err("The release installer size is empty or exceeds the download limit.".into());
    }
    Ok(())
}

fn update_root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_cache_dir()
        .map(|path| path.join("updates"))
        .map_err(|error| format!("Failed to resolve the update cache: {error}"))
}

fn create_download_directory(root: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(root)
        .map_err(|error| format!("Failed to create the update cache: {error}"))?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    for attempt in 0..10 {
        let directory = root.join(format!("download-{}-{nonce}-{attempt}", std::process::id()));
        match fs::create_dir(&directory) {
            Ok(()) => return Ok(directory),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("Failed to create the installer directory: {error}")),
        }
    }
    Err("Failed to create a unique installer directory.".into())
}

#[derive(Deserialize, Serialize)]
struct DownloadManifest {
    file_name: String,
    url: String,
    size: u64,
    sha256: String,
}

fn verify_executable(file: &mut File, size: u64) -> Result<(), String> {
    let mut header = [0; 64];
    file.seek(SeekFrom::Start(0))
        .and_then(|_| file.read_exact(&mut header))
        .map_err(|_| "The downloaded file is not a complete Windows installer.")?;
    let pe_offset = u32::from_le_bytes(header[60..64].try_into().unwrap()) as u64;
    if &header[..2] != b"MZ"
        || pe_offset < 64
        || pe_offset.checked_add(4).is_none_or(|end| end > size)
    {
        return Err("The downloaded file is not a Windows executable.".into());
    }
    let mut signature = [0; 4];
    file.seek(SeekFrom::Start(pe_offset))
        .and_then(|_| file.read_exact(&mut signature))
        .map_err(|_| "The downloaded executable is incomplete.")?;
    if &signature != b"PE\0\0" {
        return Err("The downloaded file has an invalid Windows executable header.".into());
    }
    Ok(())
}

async fn write_download(
    directory: &Path,
    url: reqwest::Url,
    file_name: &str,
    expected_size: Option<u64>,
    expected_digest: Option<String>,
) -> Result<PathBuf, String> {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(600))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 || !allowed_download_host(attempt.url()) {
                attempt.error("Unexpected release asset redirect")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|error| format!("Failed to prepare the update download: {error}"))?;
    let mut response = client
        .get(url.clone())
        .send()
        .await
        .map_err(|error| format!("Failed to fetch the installer: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Failed to fetch the installer: {error}"))?;
    if let Some(size) = response.content_length() {
        validate_expected_size(Some(size))?;
        if expected_size.is_some_and(|expected| expected != size) {
            return Err("The installer size does not match the release metadata.".into());
        }
    }
    let target = directory.join(file_name);
    let mut output = OpenOptions::new()
        .write(true)
        .read(true)
        .create_new(true)
        .open(&target)
        .map_err(|error| format!("Failed to create the installer file: {error}"))?;
    let mut hasher = Sha256::new();
    let mut size: u64 = 0;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("The installer download was interrupted: {error}"))?
    {
        size = size
            .checked_add(chunk.len() as u64)
            .ok_or("The installer is too large.")?;
        if size > MAX_INSTALLER_SIZE || expected_size.is_some_and(|expected| size > expected) {
            return Err("The installer exceeds the expected download size.".into());
        }
        output
            .write_all(&chunk)
            .map_err(|error| format!("Failed to save the installer: {error}"))?;
        hasher.update(&chunk);
    }
    validate_expected_size(Some(size))?;
    if expected_size.is_some_and(|expected| size != expected) {
        return Err("The installer download is incomplete.".into());
    }
    let sha256 = format!("{:x}", hasher.finalize());
    if expected_digest
        .as_ref()
        .is_some_and(|expected| expected != &sha256)
    {
        return Err("The installer checksum does not match the release metadata.".into());
    }
    output
        .sync_all()
        .map_err(|error| format!("Failed to finish saving the installer: {error}"))?;
    verify_executable(&mut output, size)?;
    let manifest = DownloadManifest {
        file_name: file_name.to_owned(),
        url: url.into(),
        size,
        sha256,
    };
    let bytes = serde_json::to_vec(&manifest).map_err(|error| error.to_string())?;
    fs::write(directory.join(MANIFEST), bytes)
        .map_err(|error| format!("Failed to record the update download: {error}"))?;
    Ok(target)
}

#[tauri::command]
pub async fn download_release_asset(
    app: AppHandle,
    url: String,
    file_name: String,
    expected_size: Option<u64>,
    expected_digest: Option<String>,
) -> Result<String, String> {
    require_windows()?;
    let url = validate_asset_url(&url, &file_name)?;
    validate_expected_size(expected_size)?;
    let expected_digest = parse_expected_digest(expected_digest.as_deref())?;
    let directory = create_download_directory(&update_root(&app)?)?;
    match write_download(&directory, url, &file_name, expected_size, expected_digest).await {
        Ok(path) => Ok(path.to_string_lossy().into_owned()),
        Err(error) => {
            // Remove only the files this download owns; never traverse a directory.
            let _ = fs::remove_file(directory.join(&file_name));
            let _ = fs::remove_file(directory.join(MANIFEST));
            let _ = fs::remove_dir(&directory);
            Err(error)
        }
    }
}

fn owned_download_directory(root: &Path, directory: &Path) -> Result<PathBuf, String> {
    let root = fs::canonicalize(root).map_err(|_| "The updater download cache does not exist.")?;
    let directory = fs::canonicalize(directory)
        .map_err(|_| "The updater download directory no longer exists.")?;
    if directory.parent() != Some(root.as_path())
        || !directory
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("download-"))
    {
        return Err("Only files downloaded into the updater cache can be installed.".into());
    }
    Ok(directory)
}

fn validate_download(root: &Path, target: &Path) -> Result<PathBuf, String> {
    let target =
        fs::canonicalize(target).map_err(|_| "The downloaded installer no longer exists.")?;
    let directory =
        owned_download_directory(root, target.parent().ok_or("Invalid installer path.")?)?;
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Invalid installer file name.")?;
    validate_file_name(name)?;
    let manifest_path = directory.join(MANIFEST);
    let metadata = fs::symlink_metadata(&manifest_path)
        .map_err(|_| "The updater download record is missing.")?;
    if !metadata.is_file() || metadata.len() > 4096 || metadata.file_type().is_symlink() {
        return Err("Invalid updater download record.".into());
    }
    let manifest: DownloadManifest =
        serde_json::from_slice(&fs::read(manifest_path).map_err(|error| error.to_string())?)
            .map_err(|_| "The updater download record is damaged.")?;
    if manifest.file_name != name {
        return Err("The installer does not match its download record.".into());
    }
    validate_asset_url(&manifest.url, name)?;
    validate_expected_size(Some(manifest.size))?;
    let expected_digest =
        parse_expected_digest(Some(&format!("sha256:{}", manifest.sha256)))?.unwrap();
    let mut input = File::open(&target).map_err(|error| error.to_string())?;
    if input.metadata().map_err(|error| error.to_string())?.len() != manifest.size {
        return Err("The downloaded installer has changed or is incomplete.".into());
    }
    verify_executable(&mut input, manifest.size)?;
    input
        .seek(SeekFrom::Start(0))
        .map_err(|error| error.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = input.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    if format!("{:x}", hasher.finalize()) != expected_digest {
        return Err("The downloaded installer checksum has changed. Download it again.".into());
    }
    Ok(target)
}

fn cancel_download_handoff(root: &Path, target: &Path) -> Result<(), String> {
    // Cancellation must still work if the installer or manifest was damaged/deleted.
    validate_file_name(
        target
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("Invalid installer file name.")?,
    )?;
    let directory =
        owned_download_directory(root, target.parent().ok_or("Invalid installer path.")?)?;
    // Stop the helper without requiring any disk space or writable cache files.
    let stopped = stop_registered_helper(&directory)?;
    match fs::write(directory.join("install.cancelled"), b"cancelled") {
        Ok(()) => Ok(()),
        Err(_) if stopped => Ok(()),
        Err(error) => Err(format!("Failed to cancel the update handoff: {error}")),
    }
}

fn discard_download(root: &Path, target: &Path) -> Result<(), String> {
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Invalid installer file name.")?;
    validate_file_name(name)?;
    let directory =
        owned_download_directory(root, target.parent().ok_or("Invalid installer path.")?)?;
    if directory.join("install.ps1").exists()
        && !directory.join("install.cancelled").exists()
        && !helper_was_cancelled(&directory)
    {
        return Err("Cancel the active update handoff before discarding its download.".into());
    }
    for path in [directory.join(name), directory.join(MANIFEST)] {
        match fs::remove_file(path) {
            Ok(()) => (),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(format!("Failed to discard the update download: {error}")),
        }
    }
    // Retain helper/cancellation records and unrelated files. Never recurse.
    match fs::remove_dir(directory) {
        Ok(()) => Ok(()),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::DirectoryNotEmpty | std::io::ErrorKind::NotFound
            ) =>
        {
            Ok(())
        }
        Err(error) => Err(format!("Failed to clean the update directory: {error}")),
    }
}

fn windows_process_path(path: &Path) -> Result<PathBuf, String> {
    let value = path
        .to_str()
        .ok_or("The updater cannot use a path containing invalid Unicode.")?;
    if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
        return Ok(PathBuf::from(format!(r"\\{unc}")));
    }
    if let Some(dos) = value.strip_prefix(r"\\?\") {
        if dos.as_bytes().get(1) != Some(&b':')
            || !dos.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        {
            return Err(
                "The updater cannot launch an installer from this Windows device path.".into(),
            );
        }
        return Ok(PathBuf::from(dos));
    }
    Ok(path.to_path_buf())
}

// Shared by the native preflight and both helper checks. tasklist applies the
// same current-user/image-name scope as NSIS without terminating any process.
const WINDOWS_SIBLING_GUARD: &str = r#"function Assert-NoSiblingApp {
    $imageName = $env:JF_UPDATE_EXECUTABLE_NAME
    $updatingParentId = [uint32]0
    if ([string]::IsNullOrWhiteSpace($imageName) -or
        -not [uint32]::TryParse($env:JF_UPDATE_PARENT_PID, [ref]$updatingParentId)) {
        throw 'Could not check for other Joker Forge windows. Keep the app open and try again.'
    }
    $currentUser = [System.Security.Principal.WindowsIdentity]::GetCurrent().Name
    $tasklist = Join-Path $env:SystemRoot 'System32\tasklist.exe'
    $rows = & $tasklist /FI ('IMAGENAME eq ' + $imageName) /FI ('USERNAME eq ' + $currentUser) /FO CSV /NH 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw 'Could not check for other Joker Forge windows. Keep the app open and try again.'
    }
    foreach ($line in @($rows)) {
        $text = [string]$line
        # With no matches tasklist prints a localized informational line.
        if (-not $text.StartsWith('"')) { continue }
        $record = ConvertFrom-Csv -InputObject $text -Header ImageName,ProcessId,SessionName,SessionNumber,MemoryUsage
        if (-not [string]::Equals($record.ImageName, $imageName, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw 'Could not validate the list of other Joker Forge windows. Keep the app open and try again.'
        }
        $otherProcessId = [uint32]0
        if (-not [uint32]::TryParse($record.ProcessId, [ref]$otherProcessId)) {
            throw 'Could not validate the list of other Joker Forge windows. Keep the app open and try again.'
        }
        if ($otherProcessId -ne $updatingParentId) {
            throw 'Close any other Joker Forge windows, including the other release channel or development app, before updating.'
        }
    }
}
"#;

// Paths are environment data, never interpolated into executable script source.
const WINDOWS_HELPER: &str = r#"$ErrorActionPreference = 'Stop'
$directory = $env:JF_UPDATE_DIRECTORY
$installer = $env:JF_UPDATE_INSTALLER
$executable = $env:JF_UPDATE_EXECUTABLE
$parentId = [int]$env:JF_UPDATE_PARENT_PID
$ready = Join-Path $directory 'helper.ready'
$cancel = Join-Path $directory 'install.cancelled'
$log = Join-Path $directory 'update-error.txt'
try {
    $parentProcess = Get-Process -Id $parentId
    Assert-NoSiblingApp
    [System.IO.File]::WriteAllText($ready, 'ready')
    $deadline = [DateTime]::UtcNow.AddSeconds(180)
    while (-not $parentProcess.HasExited) {
        if (Test-Path -LiteralPath $cancel) { exit 0 }
        if ([DateTime]::UtcNow -ge $deadline) {
            throw 'Joker Forge is still running. Close the app and try the update again.'
        }
        $parentProcess.WaitForExit(250) | Out-Null
    }
    if (Test-Path -LiteralPath $cancel) { exit 0 }
    Assert-NoSiblingApp
    # Get-FileHash is a script-module function and may be unavailable when the
    # app inherits a PowerShell 7 PSModulePath. Use the framework directly.
    $installerStream = [System.IO.File]::OpenRead($installer)
    $sha256 = $null
    try {
        $sha256 = [System.Security.Cryptography.SHA256]::Create()
        $actualHash = [System.BitConverter]::ToString($sha256.ComputeHash($installerStream)).Replace('-', '').ToLowerInvariant()
        if ($installerStream.Length -ne [long]$env:JF_UPDATE_SIZE -or
            -not [string]::Equals($actualHash, $env:JF_UPDATE_SHA256, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw 'The installer changed while waiting for the app to close. Download the update again.'
        }
    } finally {
        if ($null -ne $sha256) { $sha256.Dispose() }
        $installerStream.Dispose()
    }
    $process = Start-Process -FilePath $installer -ArgumentList '/S /UPDATE' -WindowStyle Hidden -PassThru -Wait
    if ($process.ExitCode -ne 0) {
        throw ('The installer returned error code ' + $process.ExitCode + '. Install the update manually from the official release page.')
    }
    Start-Process -FilePath $executable -WindowStyle Normal
    Remove-Item -LiteralPath $installer -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath (Join-Path $directory 'download.json') -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $ready -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $PSCommandPath -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $directory -ErrorAction SilentlyContinue
} catch {
    $message = 'Joker Forge could not finish updating: ' + $_.Exception.Message
    try { [System.IO.File]::WriteAllText($log, $message) } catch { }
    Add-Type -AssemblyName System.Windows.Forms
    [System.Windows.Forms.MessageBox]::Show($message, 'Joker Forge update failed') | Out-Null
    exit 1
}
"#;

#[cfg(target_os = "windows")]
fn windows_powershell() -> Result<PathBuf, String> {
    Ok(PathBuf::from(
        std::env::var_os("SystemRoot").ok_or("Windows system directory is unavailable.")?,
    )
    .join("System32/WindowsPowerShell/v1.0/powershell.exe"))
}

#[cfg(target_os = "windows")]
fn reject_sibling_instances(executable: &Path, parent_id: u32) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    let name = executable
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Could not identify the running Joker Forge executable.")?;
    let script = format!(
        "{WINDOWS_SIBLING_GUARD}\n$ErrorActionPreference = 'Stop'\ntry {{ Assert-NoSiblingApp }} catch {{ [Console]::Error.WriteLine($_.Exception.Message); exit 1 }}"
    );
    let result = Command::new(windows_powershell()?)
        .args(["-NoProfile", "-NonInteractive", "-Command"])
        .arg(script)
        .env("JF_UPDATE_EXECUTABLE_NAME", name)
        .env("JF_UPDATE_PARENT_PID", parent_id.to_string())
        .creation_flags(0x08000000)
        .output()
        .map_err(|error| format!("Could not check for other Joker Forge windows: {error}"))?;
    if !result.status.success() {
        let message = String::from_utf8_lossy(&result.stderr).trim().to_owned();
        return Err(if message.is_empty() {
            "Could not check for other Joker Forge windows. Keep the app open and try again.".into()
        } else {
            message
        });
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn launch_installer(target: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    let directory = target.parent().ok_or("Invalid updater directory.")?;
    let manifest: DownloadManifest = serde_json::from_slice(
        &fs::read(directory.join(MANIFEST)).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let executable =
        windows_process_path(&std::env::current_exe().map_err(|error| error.to_string())?)?;
    reject_sibling_instances(&executable, std::process::id())?;
    let executable_name = executable
        .file_name()
        .ok_or("Could not identify the running Joker Forge executable.")?;
    let script = directory.join("install.ps1");
    let ready = directory.join("helper.ready");
    // A completed/failed handoff cannot be reused while its helper is running.
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&script)
        .map_err(|_| "An update handoff already exists. Download the update again.")?;
    output
        .write_all(WINDOWS_SIBLING_GUARD.as_bytes())
        .and_then(|_| output.write_all(WINDOWS_HELPER.as_bytes()))
        .map_err(|error| error.to_string())?;
    output.sync_all().map_err(|error| error.to_string())?;
    // PowerShell cannot read a script while Windows still holds its writer open.
    drop(output);
    let result = Command::new(windows_powershell()?)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(windows_process_path(&script)?)
        .env("JF_UPDATE_DIRECTORY", windows_process_path(directory)?)
        .env("JF_UPDATE_INSTALLER", windows_process_path(target)?)
        .env("JF_UPDATE_EXECUTABLE_NAME", executable_name)
        .env("JF_UPDATE_EXECUTABLE", &executable)
        .env("JF_UPDATE_SHA256", manifest.sha256)
        .env("JF_UPDATE_SIZE", manifest.size.to_string())
        .env("JF_UPDATE_PARENT_PID", std::process::id().to_string())
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .spawn();
    let mut child = match result {
        Ok(child) => child,
        Err(error) => {
            let _ = fs::remove_file(&script);
            return Err(format!("Failed to launch the update helper: {error}"));
        }
    };
    for _ in 0..100 {
        if ready.is_file() {
            register_helper(directory, child);
            return Ok(());
        }
        if child
            .try_wait()
            .map_err(|error| error.to_string())?
            .is_some()
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_file(&ready);
    let _ = fs::remove_file(&script);
    Err(
        "The update helper did not start. Joker Forge will remain open; try updating manually."
            .into(),
    )
}

#[cfg(not(target_os = "windows"))]
fn launch_installer(_target: &Path) -> Result<(), String> {
    require_windows()
}

#[tauri::command]
pub async fn install_update_and_restart(
    app: AppHandle,
    installer_path: String,
) -> Result<(), String> {
    require_windows()?;
    let root = update_root(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let target = validate_download(&root, Path::new(&installer_path))?;
        launch_installer(&target)
    })
    .await
    .map_err(|error| format!("The update handoff failed: {error}"))?
}

#[tauri::command]
pub async fn cancel_update_install(app: AppHandle, installer_path: String) -> Result<(), String> {
    require_windows()?;
    let root = update_root(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        cancel_download_handoff(&root, Path::new(&installer_path))
    })
    .await
    .map_err(|error| format!("Failed to cancel the update: {error}"))?
}

#[tauri::command]
pub async fn discard_update_download(app: AppHandle, installer_path: String) -> Result<(), String> {
    require_windows()?;
    let root = update_root(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        discard_download(&root, Path::new(&installer_path))
    })
    .await
    .map_err(|error| format!("Failed to discard the update download: {error}"))?
}
