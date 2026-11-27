use anyhow::{Context, Result};
use std::path::Path;

#[cfg(target_os = "macos")]
const TARGET_ICON_ICNS: &[u8] = include_bytes!("../assets/icon/onefolder-bw.icns");

#[cfg(target_os = "macos")]
pub fn set_target_folder_icon(target: &Path) -> Result<()> {
    if !target.is_dir() {
        anyhow::bail!("Target folder does not exist: {}", target.display());
    }

    let temp_dir = std::env::temp_dir();
    let icon_path = temp_dir.join(format!("onefolder-target-icon-{}.icns", std::process::id()));
    std::fs::write(&icon_path, TARGET_ICON_ICNS)
        .with_context(|| "Temporary target-folder icon could not be written")?;

    let script = r#"
import AppKit
import Foundation
let arguments = CommandLine.arguments
guard arguments.count >= 3 else { exit(2) }
let folder = arguments[1]
let iconPath = arguments[2]
guard let image = NSImage(contentsOfFile: iconPath) else { exit(3) }
if !NSWorkspace.shared.setIcon(image, forFile: folder, options: []) { exit(4) }
"#;
    let output = std::process::Command::new("swift")
        .arg("-e")
        .arg(script)
        .arg(target)
        .arg(&icon_path)
        .output()
        .context("The macOS Swift icon helper could not be started")?;
    let _ = std::fs::remove_file(&icon_path);
    if !output.status.success() {
        anyhow::bail!(
            "macOS Finder rejected the target-folder icon: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let icon_marker = target.join("Icon\r");
    let set_custom = std::process::Command::new("/usr/bin/SetFile")
        .args(["-a", "C"])
        .arg(target)
        .status()
        .context("SetFile could not set the custom-icon flag")?;
    if !set_custom.success() {
        anyhow::bail!("SetFile could not set the custom-icon flag");
    }
    if icon_marker.exists() {
        let hide_marker = std::process::Command::new("/usr/bin/SetFile")
            .args(["-a", "V"])
            .arg(&icon_marker)
            .status()
            .context("SetFile could not hide the Finder icon marker")?;
        if !hide_marker.success() {
            anyhow::bail!("SetFile could not hide the Finder icon marker");
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn create_finder_sidebar_alias(target: &Path) -> Result<std::path::PathBuf> {
    if !target.is_dir() {
        anyhow::bail!("Target folder does not exist: {}", target.display());
    }
    let parent = target
        .parent()
        .context("Target folder has no parent directory")?;
    let name = target.file_name().unwrap_or_default().to_string_lossy();
    let alias = parent.join(format!("{name} 1folder"));

    if alias.exists() {
        std::fs::remove_file(&alias)
            .with_context(|| "Existing Finder sidebar alias could not be removed")?;
    }

    let script = r#"
on run argv
  set targetFolder to POSIX file (item 1 of argv) as alias
  set parentFolder to POSIX file (item 2 of argv) as alias
  set aliasName to item 3 of argv
  tell application "Finder"
    set newAlias to make new alias file to targetFolder at parentFolder with properties {name:aliasName}
    return POSIX path of (newAlias as alias)
  end tell
end run
"#;
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .arg(target)
        .arg(parent)
        .arg(format!("{name} 1folder"))
        .output()
        .context("Finder could not create the sidebar alias")?;
    if !output.status.success() {
        anyhow::bail!(
            "Finder could not create the sidebar alias: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    set_target_folder_icon(target)?;
    let temp_dir = std::env::temp_dir();
    let icon_path = temp_dir.join(format!(
        "onefolder-sidebar-icon-{}.icns",
        std::process::id()
    ));
    std::fs::write(&icon_path, TARGET_ICON_ICNS)
        .with_context(|| "Temporary sidebar alias icon could not be written")?;

    let script = r#"
import AppKit
import Foundation
let arguments = CommandLine.arguments
guard arguments.count >= 3 else { exit(2) }
guard let image = NSImage(contentsOfFile: arguments[2]) else { exit(3) }
if !NSWorkspace.shared.setIcon(image, forFile: arguments[1], options: []) { exit(4) }
"#;
    let output = std::process::Command::new("swift")
        .arg("-e")
        .arg(script)
        .arg(&alias)
        .arg(&icon_path)
        .output()
        .context("The macOS Swift alias-icon helper could not be started")?;
    let _ = std::fs::remove_file(&icon_path);
    if !output.status.success() {
        anyhow::bail!("Finder could not set the sidebar alias icon");
    }
    Ok(alias)
}

#[cfg(not(target_os = "macos"))]
pub fn set_target_folder_icon(_target: &Path) -> Result<()> {
    anyhow::bail!("Setting the target-folder Finder icon is supported only on macOS")
}

#[cfg(not(target_os = "macos"))]
pub fn create_finder_sidebar_alias(_target: &Path) -> Result<std::path::PathBuf> {
    anyhow::bail!("Creating a Finder sidebar alias is supported only on macOS")
}
