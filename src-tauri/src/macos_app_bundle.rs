//! Wrap unpackaged macOS launches in a `.app` bundle so Launch Services,
//! Force Quit, and Activity Monitor can resolve `CFBundleIconFile`.
//!
//! `tauri dev` / `cargo run` execute `target/debug/PromptLab` as a naked
//! Mach-O. `NSRunningApplication.icon` is nil in that case even if the Dock
//! icon was set at runtime. Packaged `PromptLab.app` builds already have a
//! bundle and skip this path.

use std::fs;
use std::io::{self, ErrorKind};
use std::os::unix::fs::symlink;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const BUNDLE_ID: &str = "com.promptlab.desktop";
const BUNDLE_NAME: &str = "PromptLab";
const ICON_ICNS: &[u8] = include_bytes!("../icons/icon.icns");

pub fn reexec_from_app_bundle() {
    if std::env::var_os("PROMPTLAB_SKIP_APP_BUNDLE").is_some() {
        return;
    }

    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    if is_inside_app_bundle(&exe) {
        return;
    }

    if let Err(err) = install_and_exec(&exe) {
        eprintln!("warning: macOS app bundle wrapper failed: {err}");
    }
}

fn app_bundle_root(exe: &Path) -> io::Result<PathBuf> {
    Ok(exe
        .parent()
        .ok_or_else(|| io::Error::new(ErrorKind::NotFound, "binary has no parent directory"))?
        .join(format!("{BUNDLE_NAME}.app")))
}

fn is_inside_app_bundle(exe: &Path) -> bool {
    let mut components = exe.components().rev();
    let _file = components.next();
    matches!(
        (
            components.next().map(|c| c.as_os_str()),
            components.next().map(|c| c.as_os_str()),
            components.next().and_then(|c| c.as_os_str().to_str()),
        ),
        (Some(macos), Some(contents), Some(app))
            if macos == "MacOS" && contents == "Contents" && app.ends_with(".app")
    )
}

fn install_app_bundle(exe: &Path) -> io::Result<PathBuf> {
    let app_root = app_bundle_root(exe)?;
    let contents = app_root.join("Contents");
    let macos_dir = contents.join("MacOS");
    let resources = contents.join("Resources");
    fs::create_dir_all(&macos_dir)?;
    fs::create_dir_all(&resources)?;

    write_if_changed(&contents.join("Info.plist"), info_plist().as_bytes())?;
    write_if_changed(&contents.join("PkgInfo"), b"APPL????")?;
    write_if_changed(&resources.join("icon.icns"), ICON_ICNS)?;
    refresh_executable_link(&macos_dir.join(BUNDLE_NAME), exe)?;
    Ok(app_root)
}

fn install_and_exec(exe: &Path) -> io::Result<()> {
    let app_root = install_app_bundle(exe)?;
    refresh_launch_services(&app_root);
    let bundled_exe = app_root.join("Contents/MacOS").join(BUNDLE_NAME);
    let err = Command::new(bundled_exe)
        .args(std::env::args_os().skip(1))
        .exec();
    Err(err)
}

fn write_if_changed(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if path.is_file() {
        if let Ok(existing) = fs::read(path) {
            if existing == bytes {
                return Ok(());
            }
        }
    }
    fs::write(path, bytes)
}

fn refresh_executable_link(link: &Path, exe: &Path) -> io::Result<()> {
    match fs::symlink_metadata(link) {
        Ok(meta) if meta.file_type().is_symlink() => {
            if fs::read_link(link).ok().as_deref() == Some(exe) {
                return Ok(());
            }
            fs::remove_file(link)?;
        }
        Ok(_) => fs::remove_file(link)?,
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => return Err(err),
    }
    symlink(exe, link)
}

fn refresh_launch_services(app_root: &Path) {
    let lsregister = PathBuf::from(
        "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister",
    );
    if !lsregister.is_file() {
        return;
    }
    let _ = Command::new(lsregister)
        .args(["-f"])
        .arg(app_root)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

fn info_plist() -> String {
    let version = env!("CARGO_PKG_VERSION");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key>
	<string>en</string>
	<key>CFBundleDisplayName</key>
	<string>{BUNDLE_NAME}</string>
	<key>CFBundleExecutable</key>
	<string>{BUNDLE_NAME}</string>
	<key>CFBundleIconFile</key>
	<string>icon.icns</string>
	<key>CFBundleIdentifier</key>
	<string>{BUNDLE_ID}</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleName</key>
	<string>{BUNDLE_NAME}</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>{version}</string>
	<key>CFBundleVersion</key>
	<string>{version}</string>
	<key>LSMinimumSystemVersion</key>
	<string>10.13</string>
	<key>NSHighResolutionCapable</key>
	<true/>
</dict>
</plist>
"#
    )
}

#[cfg(test)]
mod tests {
    use super::{install_app_bundle, is_inside_app_bundle, BUNDLE_ID, ICON_ICNS};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    #[test]
    fn detects_macos_bundle_layout() {
        assert!(is_inside_app_bundle(Path::new(
            "/tmp/PromptLab.app/Contents/MacOS/PromptLab"
        )));
        assert!(!is_inside_app_bundle(Path::new("/tmp/debug/PromptLab")));
        assert!(!is_inside_app_bundle(Path::new(
            "/tmp/PromptLab.app/Contents/Resources/icon.icns"
        )));
    }

    #[test]
    fn writes_icon_and_plist_for_unpackaged_binary() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("PromptLab");
        fs::write(&exe, b"fake").unwrap();
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o755)).unwrap();

        let app_root = install_app_bundle(&exe).unwrap();
        let plist = fs::read_to_string(app_root.join("Contents/Info.plist")).unwrap();
        assert!(plist.contains(BUNDLE_ID));
        assert!(plist.contains("icon.icns"));
        assert_eq!(
            fs::read(app_root.join("Contents/Resources/icon.icns")).unwrap(),
            ICON_ICNS
        );
        assert_eq!(
            fs::read_link(app_root.join("Contents/MacOS/PromptLab")).unwrap(),
            exe
        );
        assert!(is_inside_app_bundle(
            &app_root.join("Contents/MacOS/PromptLab")
        ));
    }
}
