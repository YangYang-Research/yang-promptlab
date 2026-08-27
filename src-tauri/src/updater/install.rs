use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::UpdateError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallerKind {
    MacOsDmg,
    WindowsNsis,
    WindowsMsi,
    LinuxAppImage,
}

/// Shell/batch command executed after the current process exits.
#[derive(Debug, Clone)]
pub struct LaunchPlan {
    pub command: String,
}

pub fn installer_kind(path: &Path) -> Result<InstallerKind, UpdateError> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if name.ends_with(".dmg") {
        return Ok(InstallerKind::MacOsDmg);
    }
    if name.ends_with(".msi") {
        return Ok(InstallerKind::WindowsMsi);
    }
    if name.ends_with(".exe") {
        return Ok(InstallerKind::WindowsNsis);
    }
    if name.ends_with(".appimage") {
        return Ok(InstallerKind::LinuxAppImage);
    }
    Err(UpdateError::Install(format!(
        "unsupported installer '{}'",
        path.display()
    )))
}

/// Install the downloaded artifact and return the command that should run after this PID exits.
pub fn install_and_prepare_launch(installer: &Path) -> Result<LaunchPlan, UpdateError> {
    match installer_kind(installer)? {
        InstallerKind::MacOsDmg => install_macos_dmg(installer),
        InstallerKind::WindowsNsis => install_windows_nsis(installer),
        InstallerKind::WindowsMsi => install_windows_msi(installer),
        InstallerKind::LinuxAppImage => install_linux_appimage(installer),
    }
}

pub fn spawn_after_exit(pid: u32, launch: &LaunchPlan) -> Result<(), UpdateError> {
    #[cfg(windows)]
    {
        spawn_after_exit_windows(pid, launch)
    }
    #[cfg(not(windows))]
    {
        spawn_after_exit_unix(pid, launch)
    }
}

#[cfg(target_os = "macos")]
fn install_macos_dmg(dmg: &Path) -> Result<LaunchPlan, UpdateError> {
    let mount = dmg.with_extension("mount");
    let _ = std::fs::remove_dir_all(&mount);
    std::fs::create_dir_all(&mount)
        .map_err(|err| UpdateError::Install(format!("cannot create dmg mount: {err}")))?;

    let attach = Command::new("hdiutil")
        .args(["attach", "-nobrowse", "-readonly", "-mountpoint"])
        .arg(&mount)
        .arg(dmg)
        .status()
        .map_err(|err| UpdateError::Install(format!("hdiutil attach failed: {err}")))?;
    if !attach.success() {
        let _ = std::fs::remove_dir_all(&mount);
        return Err(UpdateError::Install("hdiutil attach failed".into()));
    }

    let result = (|| {
        let app = find_app_bundle(&mount).ok_or_else(|| {
            UpdateError::Install("dmg does not contain a .app bundle".into())
        })?;
        let dest = macos_install_destination();
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| UpdateError::Install(format!("cannot create install dir: {err}")))?;
        }
        let status = Command::new("ditto")
            .arg(&app)
            .arg(&dest)
            .status()
            .map_err(|err| UpdateError::Install(format!("ditto failed: {err}")))?;
        if !status.success() {
            return Err(UpdateError::Install("ditto copy failed".into()));
        }
        Ok(LaunchPlan {
            command: format!("open {}", sh_single_quote(&dest.to_string_lossy())),
        })
    })();

    let _ = Command::new("hdiutil")
        .args(["detach", "-quiet"])
        .arg(&mount)
        .status();
    let _ = std::fs::remove_dir_all(&mount);
    result
}

#[cfg(not(target_os = "macos"))]
fn install_macos_dmg(_dmg: &Path) -> Result<LaunchPlan, UpdateError> {
    Err(UpdateError::Install(
        "macOS disk images can only be installed on macOS".into(),
    ))
}

#[cfg(target_os = "macos")]
fn find_app_bundle(mount: &Path) -> Option<PathBuf> {
    let preferred = mount.join("PromptLab.app");
    if preferred.is_dir() {
        return Some(preferred);
    }
    std::fs::read_dir(mount)
        .ok()?
        .filter_map(|entry| entry.ok())
        .find_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("app") {
                Some(path)
            } else {
                None
            }
        })
}

#[cfg(target_os = "macos")]
fn macos_install_destination() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(app) = enclosing_app_bundle(&exe) {
            return app;
        }
    }
    PathBuf::from("/Applications/PromptLab.app")
}

#[cfg(target_os = "macos")]
fn enclosing_app_bundle(exe: &Path) -> Option<PathBuf> {
    let mut current = exe.parent()?;
    loop {
        if current.extension().and_then(|ext| ext.to_str()) == Some("app") {
            return Some(current.to_path_buf());
        }
        current = current.parent()?;
    }
}

#[cfg(windows)]
fn install_windows_nsis(installer: &Path) -> Result<LaunchPlan, UpdateError> {
    Ok(LaunchPlan {
        command: format!(
            "{} /S && start \"\" {}",
            quote_windows(&installer.to_string_lossy()),
            quote_windows(&windows_installed_exe().to_string_lossy())
        ),
    })
}

#[cfg(windows)]
fn install_windows_msi(installer: &Path) -> Result<LaunchPlan, UpdateError> {
    Ok(LaunchPlan {
        command: format!(
            "msiexec /i {} /qn /norestart && start \"\" {}",
            quote_windows(&installer.to_string_lossy()),
            quote_windows(&windows_installed_exe().to_string_lossy())
        ),
    })
}

#[cfg(windows)]
fn windows_installed_exe() -> PathBuf {
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        return PathBuf::from(local).join("PromptLab").join("PromptLab.exe");
    }
    PathBuf::from(r"C:\Program Files\PromptLab\PromptLab.exe")
}

#[cfg(not(windows))]
fn install_windows_nsis(_installer: &Path) -> Result<LaunchPlan, UpdateError> {
    Err(UpdateError::Install(
        "Windows installers can only be installed on Windows".into(),
    ))
}

#[cfg(not(windows))]
fn install_windows_msi(_installer: &Path) -> Result<LaunchPlan, UpdateError> {
    Err(UpdateError::Install(
        "Windows installers can only be installed on Windows".into(),
    ))
}

#[cfg(target_os = "linux")]
fn install_linux_appimage(installer: &Path) -> Result<LaunchPlan, UpdateError> {
    let dest = linux_appimage_destination();
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| UpdateError::Install(format!("cannot create install dir: {err}")))?;
    }
    // Never overwrite a running AppImage in-place (ETXTBSY). Stage beside it,
    // then mv after this process exits.
    let staging = dest.with_file_name("PromptLab.AppImage.new");
    std::fs::copy(installer, &staging)
        .map_err(|err| UpdateError::Install(format!("cannot copy AppImage: {err}")))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&staging)
            .map_err(|err| UpdateError::Install(err.to_string()))?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&staging, perms)
            .map_err(|err| UpdateError::Install(err.to_string()))?;
    }
    Ok(LaunchPlan {
        command: format!(
            "mv -f {} {} && exec {}",
            sh_single_quote(&staging.to_string_lossy()),
            sh_single_quote(&dest.to_string_lossy()),
            sh_single_quote(&dest.to_string_lossy())
        ),
    })
}

#[cfg(target_os = "linux")]
fn linux_appimage_destination() -> PathBuf {
    if let Ok(current) = std::env::var("APPIMAGE") {
        let path = PathBuf::from(current);
        if path.parent().is_some() {
            return path;
        }
    }
    promptlab_core::environment::user_home()
        .join(".local")
        .join("bin")
        .join("PromptLab.AppImage")
}

#[cfg(not(target_os = "linux"))]
fn install_linux_appimage(_installer: &Path) -> Result<LaunchPlan, UpdateError> {
    Err(UpdateError::Install(
        "AppImage updates can only be installed on Linux".into(),
    ))
}

#[cfg(unix)]
fn spawn_after_exit_unix(pid: u32, launch: &LaunchPlan) -> Result<(), UpdateError> {
    let script = format!(
        "while kill -0 {pid} 2>/dev/null; do sleep 0.2; done; {}",
        launch.command
    );
    Command::new("sh")
        .arg("-c")
        .arg(format!(
            "nohup sh -c {} >/dev/null 2>&1 &",
            sh_single_quote(&script)
        ))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| UpdateError::Install(format!("failed to schedule relaunch: {err}")))?;
    Ok(())
}

#[cfg(unix)]
fn sh_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(windows)]
fn spawn_after_exit_windows(pid: u32, launch: &LaunchPlan) -> Result<(), UpdateError> {
    let dir = std::env::temp_dir();
    let bat = dir.join(format!("promptlab-relaunch-{pid}.bat"));
    let body = format!(
        "@echo off\r\n:wait\r\ntasklist /FI \"PID eq {pid}\" | find \"{pid}\" >nul\r\nif not errorlevel 1 (\r\n  timeout /t 1 /nobreak >nul\r\n  goto wait\r\n)\r\n{}\r\ndel \"%~f0\"\r\n",
        launch.command
    );
    std::fs::write(&bat, body)
        .map_err(|err| UpdateError::Install(format!("cannot write relaunch script: {err}")))?;
    Command::new("cmd")
        .args(["/C", "start", "", "/MIN"])
        .arg(&bat)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| UpdateError::Install(format!("failed to schedule relaunch: {err}")))?;
    Ok(())
}

#[cfg(windows)]
fn quote_windows(value: &str) -> String {
    if value.is_empty() {
        "\"\"".into()
    } else if value.chars().any(|ch| ch.is_whitespace() || ch == '"') {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_installer_kinds() {
        assert_eq!(
            installer_kind(Path::new("PromptLab-0.2.0-darwin-aarch64.dmg")).unwrap(),
            InstallerKind::MacOsDmg
        );
        assert_eq!(
            installer_kind(Path::new("PromptLab-0.2.0-windows-x64-setup.exe")).unwrap(),
            InstallerKind::WindowsNsis
        );
        assert_eq!(
            installer_kind(Path::new("PromptLab-0.2.0.msi")).unwrap(),
            InstallerKind::WindowsMsi
        );
        assert_eq!(
            installer_kind(Path::new("PromptLab-0.2.0-linux-x86_64.AppImage")).unwrap(),
            InstallerKind::LinuxAppImage
        );
        assert!(installer_kind(Path::new("notes.txt")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn quotes_paths_for_shell() {
        assert_eq!(sh_single_quote("/Applications/PromptLab.app"), "'/Applications/PromptLab.app'");
        assert_eq!(sh_single_quote("it's"), "'it'\\''s'");
    }
}
