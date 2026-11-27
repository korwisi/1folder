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

#[cfg(not(target_os = "macos"))]
pub fn set_target_folder_icon(_target: &Path) -> Result<()> {
    anyhow::bail!("Setting the target-folder Finder icon is supported only on macOS")
}
