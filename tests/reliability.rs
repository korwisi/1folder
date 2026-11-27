use std::process::Command;

#[test]
fn cli_once_dry_run_is_side_effect_free() {
    let tmp = tempfile::tempdir().unwrap();
    let watched = tmp.path().join("watched");
    let target = tmp.path().join("target");
    let report = tmp.path().join("report.jsonl");
    let config_path = tmp.path().join("config.toml");
    std::fs::create_dir_all(&watched).unwrap();
    std::fs::write(watched.join("foto.jpg"), b"jpeg").unwrap();

    std::fs::write(
        &config_path,
        format!(
            r#"target_dir = "{}"

[watch]
dirs = ["{}"]
stability_wait_ms = 100
debounce_ms = 100

[general]
preserve_source_paths = "off"

[[rules]]
name = "images"
subdir = "Images"
extensions = ["jpg"]
"#,
            target.display(),
            watched.display()
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_onefolder"))
        .arg("--config")
        .arg(&config_path)
        .arg("--once")
        .arg("--dry-run")
        .arg("--report")
        .arg(&report)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(std::fs::read(watched.join("foto.jpg")).unwrap(), b"jpeg");
    assert!(!target.exists());
    assert!(!target.join(".1folder-journal.jsonl").exists());
    assert!(!target.join(".1folder_index.json").exists());
    assert!(!target.join(".1folder-operation.json").exists());
    assert!(!report.exists());
}

#[test]
fn deleted_target_file_is_not_duplicate_when_reintroduced() {
    let tmp = tempfile::tempdir().unwrap();
    let watched = tmp.path().join("watched");
    let target = tmp.path().join("target");
    let config_path = tmp.path().join("config.toml");
    std::fs::create_dir_all(&watched).unwrap();
    let source = watched.join("foto.jpg");
    std::fs::write(&source, b"jpeg").unwrap();

    std::fs::write(
        &config_path,
        format!(
            r#"target_dir = "{}"

[watch]
dirs = ["{}"]
stability_wait_ms = 100
debounce_ms = 100

[general]
preserve_source_paths = "off"

[notify]
enabled = false

[[rules]]
name = "images"
subdir = "Images"
extensions = ["jpg"]
"#,
            target.display(),
            watched.display()
        ),
    )
    .unwrap();

    let first = Command::new(env!("CARGO_BIN_EXE_onefolder"))
        .arg("--config")
        .arg(&config_path)
        .arg("--once")
        .output()
        .unwrap();
    assert!(
        first.status.success(),
        "first run stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );

    let first_destination = target.join("Images/foto.jpg");
    assert_eq!(std::fs::read(&first_destination).unwrap(), b"jpeg");
    assert!(!source.exists());

    std::fs::remove_file(&first_destination).unwrap();
    std::fs::write(&source, b"jpeg").unwrap();

    let second = Command::new(env!("CARGO_BIN_EXE_onefolder"))
        .arg("--config")
        .arg(&config_path)
        .arg("--once")
        .output()
        .unwrap();
    assert!(
        second.status.success(),
        "second run stderr: {}",
        String::from_utf8_lossy(&second.stderr)
    );

    assert_eq!(std::fs::read(&first_destination).unwrap(), b"jpeg");
    assert!(!source.exists());

    let journal = std::fs::read_to_string(target.join(".1folder-journal.jsonl")).unwrap();
    assert_eq!(journal.lines().count(), 2);
    assert!(journal.contains("\"action\":\"moved\""));
}
