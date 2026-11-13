use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    En,
    De,
}

#[derive(Debug)]
pub struct Messages {
    pub watching: &'static str,
    pub target_folder: &'static str,
    pub running_hint: &'static str,
    pub watch_dir_missing: &'static str,
    pub scanning_failed: &'static str,
    pub files_to_check: &'static str,
    pub done_sorted: &'static str,
    pub event_failed: &'static str,
    pub error_processing: &'static str,
    pub file_sorted_title: &'static str,
    pub duplicate_skipped_title: &'static str,
    pub duplicate_skipped_log: &'static str,
    pub duplicate_already_in: &'static str,
    pub path_without_name: &'static str,
    pub move_failed: &'static str,
    pub dir_not_creatable: &'static str,
}

pub const EN: Messages = Messages {
    watching: "Watching: {}",
    target_folder: "Target folder: {}",
    running_hint: "1folder is running – press Ctrl+C to quit",
    watch_dir_missing: "Watched folder does not exist, skipping: {}",
    scanning_failed: "Scanning failed: {}",
    files_to_check: "{} file(s) to check",
    done_sorted: "Done: {} file(s) sorted",
    event_failed: "Filesystem event failed: {}",
    error_processing: "Error processing {}: {}",
    file_sorted_title: "File sorted",
    duplicate_skipped_title: "Duplicate skipped",
    duplicate_skipped_log: "Duplicate skipped: {} (already in {})",
    duplicate_already_in: "{} already exists in {}",
    path_without_name: "Path without file name: {}",
    move_failed: "Moving failed: {} → {}",
    dir_not_creatable: "Directory not creatable: {}",
};

pub const DE: Messages = Messages {
    watching: "Überwache: {}",
    target_folder: "Zielordner: {}",
    running_hint: "1folder läuft – beenden mit Strg+C",
    watch_dir_missing: "Überwachter Ordner existiert nicht, wird übersprungen: {}",
    scanning_failed: "Durchsuchen fehlgeschlagen: {}",
    files_to_check: "{} Datei(en) zu prüfen",
    done_sorted: "Fertig: {} Datei(en) einsortiert",
    event_failed: "Dateisystem-Ereignis fehlgeschlagen: {}",
    error_processing: "Fehler bei {}: {}",
    file_sorted_title: "Datei einsortiert",
    duplicate_skipped_title: "Duplikat übersprungen",
    duplicate_skipped_log: "Duplikat übersprungen: {} (bereits in {})",
    duplicate_already_in: "{} liegt bereits in {}",
    path_without_name: "Pfad ohne Dateinamen: {}",
    move_failed: "Verschieben fehlgeschlagen: {} → {}",
    dir_not_creatable: "Ordner nicht anlegbar: {}",
};

impl Messages {
    pub fn get(language: Language) -> &'static Messages {
        match language {
            Language::En => &EN,
            Language::De => &DE,
        }
    }
}

/// Fills a template's `{}` placeholders in order with the given values.
pub fn fmt(template: &str, args: &[&dyn std::fmt::Display]) -> String {
    let mut out = template.to_string();
    for arg in args {
        if let Some(pos) = out.find("{}") {
            out.replace_range(pos..pos + 2, &arg.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn languages_differ_and_contain_all_fields() {
        let en = Messages::get(Language::En);
        let de = Messages::get(Language::De);
        assert_ne!(en.watching, de.watching);
        assert_ne!(en.file_sorted_title, de.file_sorted_title);
        assert_eq!(en.duplicate_skipped_log.contains("{}"), true);
        assert_eq!(de.duplicate_skipped_log.matches("{}").count(), 2);
    }

    #[test]
    fn fmt_replaces_placeholders_in_order() {
        let result = fmt(Messages::get(Language::En).duplicate_skipped_log, &[
            &Path::new("/a/b.txt").display(),
            &Path::new("/c/d.txt").display(),
        ]);
        assert_eq!(result, "Duplicate skipped: /a/b.txt (already in /c/d.txt)");
    }
}
