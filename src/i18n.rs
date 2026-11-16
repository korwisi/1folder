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
    pub items_to_check: &'static str,
    pub done_sorted: &'static str,
    pub event_failed: &'static str,
    pub error_processing: &'static str,
    pub file_sorted_title: &'static str,
    pub folder_sorted_title: &'static str,
    pub undo_restored: &'static str,
    pub undo_dst_missing: &'static str,
    pub undo_nothing: &'static str,
    pub undo_done: &'static str,
    pub undo_invalid_target: &'static str,
    pub undo_invalid_within: &'static str,
    pub report_write_failed: &'static str,
    pub duplicate_skipped_title: &'static str,
    pub duplicate_skipped_log: &'static str,
    pub duplicate_already_in: &'static str,
    pub move_failed: &'static str,
    pub dir_not_creatable: &'static str,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub tags_failed: &'static str,
}

pub const EN: Messages = Messages {
    watching: "Watching: {}",
    target_folder: "Target folder: {}",
    running_hint: "1folder is running – press Ctrl+C to quit",
    watch_dir_missing: "Watched folder does not exist, skipping: {}",
    items_to_check: "{} item(s) to check",
    done_sorted: "Done: {} item(s) sorted",
    event_failed: "Filesystem event failed: {}",
    error_processing: "Error processing {}: {}",
    file_sorted_title: "File sorted",
    folder_sorted_title: "Folder sorted",
    undo_restored: "Restored: {} → {}",
    undo_dst_missing: "Nothing to restore, destination missing: {}",
    undo_nothing: "Nothing to undo",
    undo_done: "Undone: {} item(s)",
    undo_invalid_target: "Invalid --undo value: {} (use a number or 'all')",
    undo_invalid_within: "Invalid --within duration: {} (e.g. 30m, 2h, 1d)",
    report_write_failed: "Report write failed: {}",
    duplicate_skipped_title: "Duplicate skipped",
    duplicate_skipped_log: "Duplicate skipped: {} (already in {})",
    duplicate_already_in: "{} already exists in {}",
    move_failed: "Moving failed: {} → {}",
    dir_not_creatable: "Directory not creatable: {}",
    tags_failed: "Could not set Finder tags on {}: {}",
};

pub const DE: Messages = Messages {
    watching: "Überwache: {}",
    target_folder: "Zielordner: {}",
    running_hint: "1folder läuft – beenden mit Strg+C",
    watch_dir_missing: "Überwachter Ordner existiert nicht, wird übersprungen: {}",
    items_to_check: "{} Element(e) zu prüfen",
    done_sorted: "Fertig: {} Element(e) einsortiert",
    event_failed: "Dateisystem-Ereignis fehlgeschlagen: {}",
    error_processing: "Fehler bei {}: {}",
    file_sorted_title: "Datei einsortiert",
    folder_sorted_title: "Ordner einsortiert",
    undo_restored: "Wiederhergestellt: {} → {}",
    undo_dst_missing: "Nichts wiederherzustellen, Ziel fehlt: {}",
    undo_nothing: "Nichts zum Rückgängigmachen",
    undo_done: "Rückgängig: {} Element(e)",
    undo_invalid_target: "Ungültiger Wert für --undo: {} (Zahl oder 'all')",
    undo_invalid_within: "Ungültige Dauer für --within: {} (z. B. 30m, 2h, 1d)",
    report_write_failed: "Bericht konnte nicht geschrieben werden: {}",
    duplicate_skipped_title: "Duplikat übersprungen",
    duplicate_skipped_log: "Duplikat übersprungen: {} (bereits in {})",
    duplicate_already_in: "{} liegt bereits in {}",
    move_failed: "Verschieben fehlgeschlagen: {} → {}",
    dir_not_creatable: "Ordner nicht anlegbar: {}",
    tags_failed: "Finder-Tags konnten nicht gesetzt werden für {}: {}",
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
