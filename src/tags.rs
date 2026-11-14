use crate::config::TagsConfig;
use crate::i18n::Language;
use std::path::Path;

#[cfg(target_os = "macos")]
const TAGS_XATTR: &str = "com.apple.metadata:_kMDItemUserTags";

pub fn tag_string(name: &str, color: Option<u8>) -> String {
    match color {
        Some(c) => format!("{name}\n{c}"),
        None => name.to_string(),
    }
}

pub fn apply(
    config: &TagsConfig,
    language: Language,
    path: &Path,
    category_tag: &str,
    color: Option<u8>,
) {
    if !config.enabled || category_tag.is_empty() {
        return;
    }
    let mut tags = vec![tag_string(category_tag, color)];
    let root = &config.root_tag;
    if !root.is_empty() && root != category_tag {
        tags.push(root.clone());
    }
    write_tags(path, &tags, language);
}

#[cfg(target_os = "macos")]
fn write_tags(path: &Path, tags: &[String], language: Language) {
    use crate::i18n::{fmt, Messages};
    let value = plist::Value::Array(
        tags.iter()
            .map(|t| plist::Value::String(t.clone()))
            .collect(),
    );
    if let Err(e) = xattr::set(path, TAGS_XATTR, &value.to_binary()) {
        log::warn!("{}", fmt(Messages::get(language).tags_failed, &[&path.display(), &e]));
    }
}

#[cfg(not(target_os = "macos"))]
fn write_tags(_path: &Path, _tags: &[String], _language: Language) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_string_encodes_color_after_newline() {
        assert_eq!(tag_string("Papers", None), "Papers");
        assert_eq!(tag_string("Rechnungen", Some(6)), "Rechnungen\n6");
    }

    #[test]
    fn apply_is_safe_on_all_platforms() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("a.txt");
        std::fs::write(&file, b"x").unwrap();
        let config: TagsConfig =
            toml::from_str("enabled = true\nroot_tag = \"1folder\"").unwrap();
        apply(&config, Language::En, &file, "Dokumente", Some(4));
        apply(&config, Language::De, &file, "", None);
        let disabled: TagsConfig = toml::from_str("enabled = false").unwrap();
        apply(&disabled, Language::En, &file, "Dokumente", None);
    }
}
