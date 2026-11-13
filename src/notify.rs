use crate::config::NotifyConfig;

pub fn maybe_notify(config: &NotifyConfig, title: &str, message: &str) {
    if !config.enabled {
        return;
    }
    send(title, message);
}

#[cfg(target_os = "macos")]
fn send(title: &str, message: &str) {
    let escape = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!(
        "display notification \"{}\" with title \"1folder\" subtitle \"{}\"",
        escape(message),
        escape(title)
    );
    let _ = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output();
}

#[cfg(not(target_os = "macos"))]
fn send(title: &str, message: &str) {
    log::info!("[Notification] {title}: {message}");
}
