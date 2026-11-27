use crate::config::Config;
use std::path::Path;

const PROJECT_MARKERS: &[&str] = &[
    ".git",
    ".gitignore",
    ".hg",
    ".svn",
    ".venv",
    ".xcodeproj",
    "CMakeLists.txt",
    "Cargo.lock",
    "Cargo.toml",
    "Gemfile",
    "Makefile",
    "__pycache__",
    "build.gradle",
    "go.mod",
    "node_modules",
    "package.json",
    "package-lock.json",
    "pom.xml",
    "pyproject.toml",
    "requirements.txt",
    "setup.py",
    "venv",
    "yarn.lock",
];

pub fn file_extension(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

pub fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .map(|name| name.to_string_lossy().starts_with('.'))
        .unwrap_or(false)
}

pub fn is_under(path: &Path, dir: &Path) -> bool {
    path.starts_with(dir)
}

pub fn pdf_sample_text(path: &Path, max_pages: usize) -> Option<String> {
    if file_extension(path) != "pdf" {
        return None;
    }
    let doc = lopdf::Document::load(path).ok()?;
    let pages: Vec<u32> = doc.get_pages().keys().copied().take(max_pages).collect();
    doc.extract_text(&pages).ok()
}

pub fn classify(config: &Config, path: &Path, pdf_text: Option<&str>) -> String {
    classify_reason(config, path, pdf_text).0
}

pub fn classify_reason(
    config: &Config,
    path: &Path,
    pdf_text: Option<&str>,
) -> (String, Option<String>) {
    if is_sensitive_certificate_or_key(path) {
        return (config.fallback_subdir.clone(), None);
    }

    let ext = file_extension(path);
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    for rule in &config.rules {
        if rule_matches(rule, &ext, &file_name, pdf_text) {
            log::debug!(
                "{} → Regel '{}' ({})",
                path.display(),
                rule.name,
                rule.subdir
            );
            return (rule.subdir.clone(), Some(rule.name.clone()));
        }
    }
    (config.fallback_subdir.clone(), None)
}

fn is_sensitive_certificate_or_key(path: &Path) -> bool {
    match file_extension(path).as_str() {
        "pem" | "p12" | "pfx" | "p8" | "der" | "crt" | "cer" | "jks" | "keystore" => true,
        "key" => {
            let mut file = match std::fs::File::open(path) {
                Ok(file) => file,
                Err(_) => return false,
            };
            use std::io::Read;
            let mut prefix = [0u8; 65536];
            let len = match file.read(&mut prefix) {
                Ok(len) => len,
                Err(_) => return false,
            };
            let text = String::from_utf8_lossy(&prefix[..len]);
            [
                "PRIVATE KEY",
                "PuTTY-User-Key-File",
                "BEGIN SSH2 ENCRYPTED PRIVATE KEY",
            ]
            .iter()
            .any(|marker| text.contains(marker))
        }
        _ => false,
    }
}

pub fn classify_folder_reason(config: &Config, dir: &Path) -> (String, Option<String>) {
    let folder_name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    for rule in &config.rules {
        if rule
            .folder_keywords
            .iter()
            .any(|k| folder_name.contains(&k.to_lowercase()))
        {
            log::debug!(
                "{} → Folder-Regel '{}' ({})",
                dir.display(),
                rule.name,
                rule.subdir
            );
            return (rule.subdir.clone(), Some(rule.name.clone()));
        }
    }
    if is_bundle(dir) {
        return (config.folders.subdir.clone(), Some("bundle".to_string()));
    }
    if has_project_markers(dir, &config.folders.markers) {
        return (
            config.folders.subdir.clone(),
            Some("project-markers".to_string()),
        );
    }
    match homogeneous_category(config, dir) {
        Some(category) => (category, Some("homogeneous".to_string())),
        None => (
            config.folders.subdir.clone(),
            Some("mixed-content".to_string()),
        ),
    }
}

fn is_bundle(dir: &Path) -> bool {
    dir.extension()
        .map(|e| {
            let e = e.to_string_lossy().to_lowercase();
            e == "app" || e == "bundle" || e == "framework"
        })
        .unwrap_or(false)
}

fn has_project_markers(dir: &Path, extra: &[String]) -> bool {
    for entry in walkdir::WalkDir::new(dir).follow_links(false).max_depth(3) {
        let Ok(entry) = entry else { continue };
        let name = entry.file_name().to_string_lossy();
        if PROJECT_MARKERS.contains(&name.as_ref()) {
            return true;
        }
        let bare = name.trim_start_matches('.');
        if extra
            .iter()
            .any(|m| m.trim_start_matches('.') == bare && !bare.is_empty())
        {
            return true;
        }
    }
    false
}

fn homogeneous_category(config: &Config, dir: &Path) -> Option<String> {
    let mut category: Option<String> = None;
    for entry in walkdir::WalkDir::new(dir).follow_links(false) {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if !entry.file_type().is_file() || is_hidden(path) {
            continue;
        }
        let current = classify(config, path, None);
        match &category {
            None => category = Some(current),
            Some(existing) if *existing == current => {}
            _ => return None,
        }
    }
    category
}

fn rule_matches(
    rule: &crate::config::Rule,
    ext: &str,
    file_name: &str,
    pdf_text: Option<&str>,
) -> bool {
    if !rule.extensions.is_empty()
        && !rule
            .extensions
            .iter()
            .any(|e| e.trim_start_matches('.').eq_ignore_ascii_case(ext))
    {
        return false;
    }
    if !rule.filename_keywords.is_empty()
        && !rule
            .filename_keywords
            .iter()
            .any(|k| file_name.contains(&k.to_lowercase()))
    {
        return false;
    }
    if !rule.pdf_keywords.is_empty() {
        let text = match pdf_text {
            Some(t) => t.to_lowercase(),
            None => return false,
        };
        if !rule
            .pdf_keywords
            .iter()
            .any(|k| text.contains(&k.to_lowercase()))
        {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, Rule};
    use std::path::PathBuf;

    fn test_config() -> Config {
        toml::from_str(crate::config::DEFAULT_CONFIG).unwrap()
    }

    fn path(name: &str) -> PathBuf {
        PathBuf::from(name)
    }

    fn rule(name: &str, subdir: &str, ext: &[&str], file_kw: &[&str], pdf_kw: &[&str]) -> Rule {
        Rule {
            name: name.to_string(),
            subdir: subdir.to_string(),
            extensions: ext.iter().map(|s| s.to_string()).collect(),
            filename_keywords: file_kw.iter().map(|s| s.to_string()).collect(),
            folder_keywords: vec![],
            pdf_keywords: pdf_kw.iter().map(|s| s.to_string()).collect(),
            color: None,
        }
    }

    #[test]
    fn extension_rule_matches_case_insensitively() {
        let config = test_config();
        assert_eq!(classify(&config, &path("Foto.JPG"), None), "Bilder");
        assert_eq!(classify(&config, &path("backup.zip"), None), "Archive");
    }

    #[test]
    fn raw_images_and_disc_images_match_default_rules() {
        let config = test_config();
        assert_eq!(classify(&config, &path("IMG_0001.CR2"), None), "Bilder");
        assert_eq!(classify(&config, &path("IMG_0002.cr3"), None), "Bilder");
        assert_eq!(classify(&config, &path("DSC_0001.NEF"), None), "Bilder");
        assert_eq!(
            classify(&config, &path("ubuntu-24.04.iso"), None),
            "Archive"
        );
    }

    #[test]
    fn additional_office_formats_match_documents() {
        let config = test_config();
        assert_eq!(
            classify(&config, &path("macro-document.docm"), None),
            "Dokumente"
        );
        assert_eq!(classify(&config, &path("template.potx"), None), "Dokumente");
        assert_eq!(classify(&config, &path("publisher.pub"), None), "Dokumente");
    }

    #[test]
    fn private_keys_stay_in_fallback_and_keynote_stays_in_documents() {
        let config = test_config();
        let tmp = tempfile::tempdir().unwrap();
        let private_key = tmp.path().join("server.key");
        std::fs::write(
            &private_key,
            b"-----BEGIN RSA PRIVATE KEY-----\nprivate-data\n",
        )
        .unwrap();
        assert_eq!(
            classify(&config, &private_key, None),
            config.fallback_subdir
        );

        let keynote = tmp.path().join("presentation.key");
        std::fs::write(&keynote, b"PK\x03\x04zip-based-keynote").unwrap();
        assert_eq!(classify(&config, &keynote, None), "Dokumente");
        assert_eq!(
            classify(&config, &path("certificate.pem"), None),
            "Sonstiges"
        );
    }

    #[test]
    fn modeling_formats_match_3d_category() {
        let config = test_config();
        assert_eq!(classify(&config, &path("scene.blend"), None), "3D-Druck");
        assert_eq!(classify(&config, &path("character.fbx"), None), "3D-Druck");
    }

    #[test]
    fn executable_formats_match_binaries() {
        let config = test_config();
        assert_eq!(classify(&config, &path("installer.exe"), None), "Binaries");
        assert_eq!(classify(&config, &path("library.dll"), None), "Binaries");
        assert_eq!(classify(&config, &path("app.AppImage"), None), "Binaries");
    }

    #[test]
    fn filename_keyword_rule_matches() {
        let config = test_config();
        assert_eq!(
            classify(&config, &path("Strom_Rechnung_Jan.pdf"), Some("inhalt")),
            "Rechnungen"
        );
    }

    #[test]
    fn pdf_keyword_rule_uses_extracted_text() {
        let config = test_config();
        let text = "Abstract. We propose a novel method. DOI: 10.1000/xyz";
        assert_eq!(
            classify(&config, &path("1701.00001.pdf"), Some(text)),
            "Papers"
        );
    }

    #[test]
    fn pdf_without_keyword_match_falls_back_to_catch_all() {
        let config = test_config();
        assert_eq!(classify(&config, &path("unbekannt.pdf"), None), "Dokumente");
    }

    #[test]
    fn fallback_for_unmatched_files() {
        let config = test_config();
        assert_eq!(classify(&config, &path("irgendwas.xyz"), None), "Sonstiges");
    }

    #[test]
    fn rule_without_conditions_matches_everything() {
        let mut config = test_config();
        config.rules = vec![rule("alle", "Alles", &[], &[], &[])];
        assert_eq!(classify(&config, &path("x.unknown"), None), "Alles");
    }

    #[test]
    fn folder_with_project_markers_goes_to_folders_subdir() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("some-repo");
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        std::fs::write(dir.join("main.rs"), b"fn main() {}").unwrap();
        assert_eq!(classify_folder_reason(&test_config(), &dir).0, "Sammlungen");
    }

    #[test]
    fn homogeneous_image_folder_goes_to_bilder() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("Abschlussfeier");
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.jpg"), b"j").unwrap();
        std::fs::write(dir.join("sub/b.jpg"), b"j").unwrap();
        assert_eq!(classify_folder_reason(&test_config(), &dir).0, "Bilder");
    }

    #[test]
    fn mixed_content_folder_goes_to_folders_subdir() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("extracted");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.jpg"), b"j").unwrap();
        std::fs::write(dir.join("notes.txt"), b"t").unwrap();
        assert_eq!(classify_folder_reason(&test_config(), &dir).0, "Sammlungen");
    }

    #[test]
    fn folder_keyword_rule_matches_folder_name() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("Urlaub_2024");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("strand.jpg"), b"j").unwrap();
        let mut config = test_config();
        config
            .rules
            .insert(0, rule("privates", "Privat", &[], &[], &[]));
        config.rules[0].folder_keywords = vec!["urlaub".to_string()];
        assert_eq!(classify_folder_reason(&config, &dir).0, "Privat");
    }

    #[test]
    fn custom_markers_from_config_are_detected() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("vorlesung");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("slides.md"), b"# slides").unwrap();
        std::fs::write(dir.join(".decker"), b"").unwrap();
        assert_eq!(classify_folder_reason(&test_config(), &dir).0, "Sammlungen");
    }

    #[test]
    fn custom_markers_match_without_leading_dot() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("projekt");
        std::fs::create_dir_all(dir.join(".decker")).unwrap();
        std::fs::write(dir.join("slides.md"), b"# slides").unwrap();
        assert_eq!(classify_folder_reason(&test_config(), &dir).0, "Sammlungen");
    }

    #[test]
    fn app_bundles_go_to_folders_subdir() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("Tool.app");
        std::fs::create_dir_all(dir.join("Contents/MacOS")).unwrap();
        std::fs::write(dir.join("Contents/MacOS/tool"), b"bin").unwrap();
        assert_eq!(classify_folder_reason(&test_config(), &dir).0, "Sammlungen");
    }

    #[test]
    fn hidden_files_do_not_break_homogeneity() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("fotos");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.png"), b"p").unwrap();
        std::fs::write(dir.join(".DS_Store"), b"h").unwrap();
        assert_eq!(classify_folder_reason(&test_config(), &dir).0, "Bilder");
    }
}
