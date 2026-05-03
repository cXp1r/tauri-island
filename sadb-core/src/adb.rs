//! ADB client wrapper for scrcpy operations
//!
//! Provides high-level abstractions over ADB commands needed for scrcpy:
//! - Push server jar to device
//! - Set up reverse/forward tunnels
//! - Start server process via app_process
//! - Clean up connections

use crate::error::{Error, Result};
use tokio::process::Command as TokioCommand;
use tracing::debug;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// ADB client wrapper
#[derive(Debug, Clone)]
pub struct AdbClient {
    /// Path to adb executable
    adb_path: String,
    /// Device serial (optional)
    serial: Option<String>,
}

impl AdbClient {
    /// Create new ADB client
    pub fn new(serial: Option<String>) -> Self {
        Self {
            adb_path: "adb".to_string(),
            serial,
        }
    }

    /// Set custom adb path
    pub fn with_adb_path<S: Into<String>>(mut self, path: S) -> Self {
        self.adb_path = path.into();
        self
    }

    /// Build async command with optional serial
    fn build_async_command(&self) -> TokioCommand {
        let mut cmd = TokioCommand::new(&self.adb_path);
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        if let Some(ref serial) = self.serial {
            cmd.arg("-s").arg(serial);
        }
        cmd
    }

    /// Check if device is connected
    pub async fn is_connected(&self) -> Result<bool> {
        let output = self
            .build_async_command()
            .args(["devices"])
            .output()
            .await?;

        let output = String::from_utf8_lossy(&output.stdout);
        Ok(output.lines().count() > 1) // Header + at least one device
    }

    /// Push file to device
    pub async fn push(&self, local: &str, remote: &str) -> Result<()> {
        debug!("Pushing {} to {}", local, remote);
        
        let output = self
            .build_async_command()
            .args(["push", local, remote])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Adb(format!("push failed: {}", stderr)));
        }

        Ok(())
    }

    /// Create reverse tunnel (device connects to PC)
    pub async fn reverse(&self, remote: &str, local: &str) -> Result<()> {
        debug!("Creating reverse tunnel: {} -> {}", remote, local);
        
        let output = self
            .build_async_command()
            .args(["reverse", remote, local])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Adb(format!("reverse failed: {}", stderr)));
        }

        Ok(())
    }

    /// Create forward tunnel (PC connects to device)
    pub async fn forward(&self, local: &str, remote: &str) -> Result<()> {
        debug!("Creating forward tunnel: {} -> {}", local, remote);
        
        let output = self
            .build_async_command()
            .args(["forward", local, remote])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Adb(format!("forward failed: {}", stderr)));
        }

        Ok(())
    }

    /// Remove reverse tunnel
    pub async fn reverse_remove(&self, remote: &str) -> Result<()> {
        debug!("Removing reverse tunnel: {}", remote);
        let _ = self
            .build_async_command()
            .args(["reverse", "--remove", remote])
            .output()
            .await?;
        // Don't error if tunnel doesn't exist
        Ok(())
    }

    /// Remove forward tunnel
    pub async fn forward_remove(&self, local: &str) -> Result<()> {
        debug!("Removing forward tunnel: {}", local);
        let _ = self
            .build_async_command()
            .args(["forward", "--remove", local])
            .output()
            .await?;
        // Don't error if tunnel doesn't exist
        Ok(())
    }

    /// Build a shell command (not yet spawned). Caller is responsible for
    /// configuring stdio and spawning.
    pub fn shell_command(&self, shell_cmd: &str) -> TokioCommand {
        debug!("Building shell command: {}", shell_cmd);
        let mut cmd = self.build_async_command();
        cmd.args(["shell", shell_cmd]);
        cmd
    }

    /// Get device properties
    pub async fn get_prop(&self, prop: &str) -> Result<String> {
        let output = self
            .build_async_command()
            .args(["shell", "getprop", prop])
            .output()
            .await?;

        if !output.status.success() {
            return Err(Error::Adb("getprop failed".to_string()));
        }

        let value = String::from_utf8_lossy(&output.stdout);
        Ok(value.trim().to_string())
    }

    /// Get device model/name
    pub async fn get_device_name(&self) -> Result<String> {
        self.get_prop("ro.product.model").await
    }

    /// Connect to a device over TCP/IP (`adb connect <host>`).
    /// The host format is `ip:port` (e.g. `192.168.1.100:5555`).
    pub async fn connect(host: &str) -> Result<()> {
        debug!("Connecting to {} via TCP/IP", host);
        let mut cmd = TokioCommand::new("adb");
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        let output = cmd.args(["connect", host]).output().await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Adb(format!("adb connect failed: {}", stderr)));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("failed") || stdout.contains("cannot connect") {
            return Err(Error::Adb(format!("adb connect failed: {}", stdout.trim())));
        }

        Ok(())
    }

    /// Disconnect from a TCP/IP device (`adb disconnect <host>`).
    /// If host is empty, disconnects all TCP/IP devices.
    pub async fn disconnect(host: &str) -> Result<()> {
        debug!("Disconnecting from {}", if host.is_empty() { "all" } else { host });
        let mut cmd = TokioCommand::new("adb");
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        let output = cmd.args(["disconnect", host]).output().await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Adb(format!("adb disconnect failed: {}", stderr)));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adb_client_creation() {
        let client = AdbClient::new(None);
        assert_eq!(client.adb_path, "adb");
        assert!(client.serial.is_none());

        let client = AdbClient::new(Some("test123".to_string()));
        assert_eq!(client.adb_path, "adb");
        assert_eq!(client.serial.as_deref(), Some("test123"));
    }

    #[test]
    fn test_with_adb_path() {
        let client = AdbClient::new(None).with_adb_path("/custom/adb");
        assert_eq!(client.adb_path, "/custom/adb");
    }
}
