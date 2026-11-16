use crate::config::Config;
use std::path::Path;

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
    let ext = file_extension(path);
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    for rule in &config.rules {
        if rule_matches(rule, &ext, &file_name, pdf_text) {
            log::debug!("{} → Regel '{}' ({})", path.display(), rule.name, rule.subdir);
            return rule.subdir.clone();
        }
    }
    config.fallback_subdir.clone()
}

fn rule_matches(rule: &crate::config::Rule, ext: &str, file_name: &str, pdf_text: Option<&str>) -> bool {
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
        assert_eq!(classify(&config, &path("ubuntu-24.04.iso"), None), "Archive");
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
    fn pdf_keyword_rule_requires_text() {
        let config = test_config();
        assert_eq!(classify(&config, &path("unbekannt.pdf"), None), "Sonstiges");
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
}
