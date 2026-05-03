use std::fs::{self, File};
use std::io::{self, Cursor};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;
use zip::ZipArchive;

use crate::CREATE_NO_WINDOW;

const PLATFORM_TOOLS_URL: &str = "https://dl.google.com/android/repository/platform-tools-latest-windows.zip";
const PLATFORM_TOOLS_ZIP_NAME: &str = "platform-tools-latest-windows.zip";

#[derive(Debug, Serialize)]
pub struct AdbCheckResult {
    ok: bool,
    adb_path: String,
    version: String,
    stdout: String,
    stderr: String,
}

#[derive(Debug, Serialize)]
pub struct ToolDownloadResult {
    path: String,
    bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct AdbInstallResult {
    install_dir: String,
    adb_path: String,
    downloaded_zip: String,
}

#[derive(Debug, Serialize)]
pub struct AdbPathResult {
    adb_path: String,
}

#[derive(Debug, Serialize)]
pub struct AdbDeviceInfo {
    serial: String,
    state: String,
}

#[derive(Debug, Serialize)]
pub struct AdbDevicesResult {
    ok: bool,
    adb_path: String,
    devices: Vec<AdbDeviceInfo>,
    stdout: String,
    stderr: String,
}

fn run_adb_version(adb_path: &str) -> Result<AdbCheckResult, String> {
    let output = Command::new(adb_path)
        .arg("version")
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("failed to run adb version: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let version = stdout
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();

    Ok(AdbCheckResult {
        ok: output.status.success(),
        adb_path: adb_path.to_string(),
        version,
        stdout,
        stderr,
    })
}

fn run_adb_devices(adb_path: &str) -> Result<AdbDevicesResult, String> {
    let output = Command::new(adb_path)
        .arg("devices")
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("failed to run adb devices: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let devices = stdout
        .lines()
        .skip(1)
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let serial = parts.next()?.trim();
            let state = parts.next()?.trim();
            if serial.is_empty() || state.is_empty() {
                return None;
            }
            Some(AdbDeviceInfo {
                serial: serial.to_string(),
                state: state.to_string(),
            })
        })
        .collect();

    Ok(AdbDevicesResult {
        ok: output.status.success(),
        adb_path: adb_path.to_string(),
        devices,
        stdout,
        stderr,
    })
}

fn download_platform_tools_zip(download_dir: &Path) -> Result<ToolDownloadResult, String> {
    fs::create_dir_all(download_dir).map_err(|e| format!("failed to create download dir: {}", e))?;
    let zip_path = download_dir.join(PLATFORM_TOOLS_ZIP_NAME);

    let mut resp = crate::shared_http_client()
        .get(PLATFORM_TOOLS_URL)
        .send()
        .map_err(|e| format!("failed to download platform-tools: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("platform-tools download failed: HTTP {}", resp.status()));
    }

    let mut file = File::create(&zip_path).map_err(|e| format!("failed to create zip file: {}", e))?;
    let bytes = resp
        .copy_to(&mut file)
        .map_err(|e| format!("failed to save platform-tools zip: {}", e))?;

    Ok(ToolDownloadResult {
        path: zip_path.to_string_lossy().into_owned(),
        bytes,
    })
}

fn extract_zip(zip_path: &Path, install_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(install_dir).map_err(|e| format!("failed to create install dir: {}", e))?;
    let file = File::open(zip_path).map_err(|e| format!("failed to open zip: {}", e))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("failed to read zip: {}", e))?;
    extract_archive(&mut archive, install_dir)
}

fn extract_archive<R: io::Read + io::Seek>(archive: &mut ZipArchive<R>, install_dir: &Path) -> Result<(), String> {
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| format!("failed to read zip entry: {}", e))?;
        let enclosed_name = entry
            .enclosed_name()
            .map(PathBuf::from)
            .ok_or_else(|| format!("unsafe zip entry path: {}", entry.name()))?;
        let out_path = install_dir.join(enclosed_name);

        if entry.is_dir() {
            fs::create_dir_all(&out_path).map_err(|e| format!("failed to create dir {}: {}", out_path.display(), e))?;
            continue;
        }

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("failed to create dir {}: {}", parent.display(), e))?;
        }

        let mut outfile = File::create(&out_path).map_err(|e| format!("failed to create file {}: {}", out_path.display(), e))?;
        io::copy(&mut entry, &mut outfile).map_err(|e| format!("failed to extract file {}: {}", out_path.display(), e))?;
    }
    Ok(())
}

fn adb_path_in_install_dir(install_dir: &Path) -> PathBuf {
    install_dir.join("platform-tools").join("adb.exe")
}

fn find_adb_in_path() -> Result<PathBuf, String> {
    let output = Command::new("where")
        .arg("adb")
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("failed to run where adb: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        return Err(format!("adb not found in PATH: {}", stderr.trim()));
    }

    stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .find(|path| path.exists())
        .ok_or_else(|| "adb not found in PATH".to_string())
}

#[tauri::command]
pub fn tools_check_adb(adb_path: Option<String>) -> Result<AdbCheckResult, String> {
    let adb_path = adb_path.unwrap_or_else(|| "adb".to_string());
    run_adb_version(&adb_path)
}

#[tauri::command]
pub fn tools_check_adb_devices(adb_path: Option<String>) -> Result<AdbDevicesResult, String> {
    let adb_path = adb_path.unwrap_or_else(|| "adb".to_string());
    run_adb_devices(&adb_path)
}

#[tauri::command]
pub fn tools_find_adb_in_path() -> Result<AdbPathResult, String> {
    let adb_path = find_adb_in_path()?;
    Ok(AdbPathResult {
        adb_path: adb_path.to_string_lossy().into_owned(),
    })
}

#[tauri::command]
pub fn tools_download_adb(download_dir: String) -> Result<ToolDownloadResult, String> {
    download_platform_tools_zip(Path::new(&download_dir))
}

#[tauri::command]
pub fn tools_extract_adb(zip_path: String, install_dir: String) -> Result<String, String> {
    let install_dir = Path::new(&install_dir);
    extract_zip(Path::new(&zip_path), install_dir)?;
    Ok(adb_path_in_install_dir(install_dir).to_string_lossy().into_owned())
}

#[tauri::command]
pub fn tools_download_and_install_adb(install_dir: String) -> Result<AdbInstallResult, String> {
    let install_dir_path = Path::new(&install_dir);
    fs::create_dir_all(install_dir_path).map_err(|e| format!("failed to create install dir: {}", e))?;

    let resp = crate::shared_http_client()
        .get(PLATFORM_TOOLS_URL)
        .send()
        .map_err(|e| format!("failed to download platform-tools: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("platform-tools download failed: HTTP {}", resp.status()));
    }

    let bytes = resp
        .bytes()
        .map_err(|e| format!("failed to read platform-tools zip: {}", e))?;
    let downloaded_zip = install_dir_path.join(PLATFORM_TOOLS_ZIP_NAME);
    fs::write(&downloaded_zip, &bytes).map_err(|e| format!("failed to save platform-tools zip: {}", e))?;

    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).map_err(|e| format!("failed to read platform-tools zip: {}", e))?;
    extract_archive(&mut archive, install_dir_path)?;

    let adb_path = adb_path_in_install_dir(install_dir_path);
    if !adb_path.exists() {
        return Err(format!("adb.exe not found after install: {}", adb_path.display()));
    }

    Ok(AdbInstallResult {
        install_dir,
        adb_path: adb_path.to_string_lossy().into_owned(),
        downloaded_zip: downloaded_zip.to_string_lossy().into_owned(),
    })
}
