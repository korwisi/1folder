import AppKit
import Foundation

struct OnefolderResult {
    var json: UnsafeMutablePointer<CChar>?
    var error: UnsafeMutablePointer<CChar>?
}

@_silgen_name("onefolder_status")
func onefolder_status(_ path: UnsafePointer<CChar>) -> OnefolderResult

@_silgen_name("onefolder_preview")
func onefolder_preview(_ path: UnsafePointer<CChar>) -> OnefolderResult

@_silgen_name("onefolder_run_once")
func onefolder_run_once(_ path: UnsafePointer<CChar>, _ dryRun: Int32) -> OnefolderResult

@_silgen_name("onefolder_history")
func onefolder_history(_ path: UnsafePointer<CChar>) -> OnefolderResult

@_silgen_name("onefolder_undo_last")
func onefolder_undo_last(_ path: UnsafePointer<CChar>, _ dryRun: Int32) -> OnefolderResult

@_silgen_name("onefolder_set_target_icon")
func onefolder_set_target_icon(_ path: UnsafePointer<CChar>) -> OnefolderResult

@_silgen_name("onefolder_save_config")
func onefolder_save_config(_ path: UnsafePointer<CChar>, _ text: UnsafePointer<CChar>) -> OnefolderResult


@_silgen_name("onefolder_free_string")
func onefolder_free_string(_ value: UnsafeMutablePointer<CChar>)

struct CoreResult<T: Decodable> {
    let value: T?
    let error: String?
}

func callCore<T: Decodable>(_ configPath: String, function: (UnsafePointer<CChar>) -> OnefolderResult) -> CoreResult<T> {
    configPath.withCString { pointer in
        let result = function(pointer)
        defer {
            if let json = result.json { onefolder_free_string(json) }
            if let error = result.error { onefolder_free_string(error) }
        }
        if let errorPointer = result.error,
           let error = String(validatingCString: errorPointer),
           !error.isEmpty {
            return CoreResult(value: nil, error: error)
        }
        guard let jsonPointer = result.json,
              let json = String(validatingCString: jsonPointer),
              let data = json.data(using: .utf8) else {
            return CoreResult(value: nil, error: "The Rust core returned no data")
        }
        do {
            return CoreResult(value: try JSONDecoder().decode(T.self, from: data), error: nil)
        } catch {
            return CoreResult(value: nil, error: "Could not decode Rust response: \(error)")
        }
    }
}

struct CoreStatus: Decodable {
    let target_dir: String
    let target_exists: Bool
    let watched_dirs: [String]
    let missing_watched_dirs: [String]
    let pending_operation: Bool
}

struct PreviewOperation: Decodable {
    let action: String
    let src: String
    let dst: String
    let category: String
}

struct PreviewIssue: Decodable {
    let path: String?
    let message: String
}

struct PreviewSummary: Decodable {
    let operations: [PreviewOperation]
    let issues: [PreviewIssue]
    let items_checked: Int
}

struct ActivityRecord: Decodable {
    let timestamp: String
    let action: String
    let src: String
    let dst: String
    let category: String?
}

struct ActivitySummary: Decodable {
    let records: [ActivityRecord]
}

struct RunSummary: Decodable {
    let dry_run: Bool
}

struct UndoSummary: Decodable {
    let restored: Int
}

struct TargetIconSummary: Decodable {
    let target_dir: String
}

struct SaveConfigSummary: Decodable {
    let saved: Bool
}


@MainActor
final class DesktopController: NSObject {
    let configField = NSTextField()
    let configView = NSTextView()
    let targetDirField = NSTextField()
    let watchDirsField = NSTextField()
    let languagePopUp = NSPopUpButton()
    let preservePopUp = NSPopUpButton()
    let dedupePopUp = NSPopUpButton()
    let notifySwitch = NSSwitch()
    let tagsSwitch = NSSwitch()
    let stabilityField = NSTextField()
    let debounceField = NSTextField()
    let statusStack = NSStackView()
    let messageField = NSTextField(labelWithString: "")
    let previewView = NSTextView()
    let historyView = NSTextView()

    var configPath: String {
        configField.stringValue
    }

    override init() {
        super.init()
        configField.stringValue = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Application Support/1folder/config.toml")
            .path
        languagePopUp.addItems(withTitles: ["English", "Deutsch"])
        preservePopUp.addItems(withTitles: ["off", "structure", "prefix"])
        dedupePopUp.addItems(withTitles: ["skip", "keep_both", "move_to_duplicates"])
        for control in [targetDirField, watchDirsField, stabilityField, debounceField] {
            control.target = self
            control.action = #selector(markConfigDirty)
        }
        languagePopUp.target = self
        languagePopUp.action = #selector(markConfigDirty)
        preservePopUp.target = self
        preservePopUp.action = #selector(markConfigDirty)
        dedupePopUp.target = self
        dedupePopUp.action = #selector(markConfigDirty)
        notifySwitch.target = self
        notifySwitch.action = #selector(markConfigDirty)
        tagsSwitch.target = self
        tagsSwitch.action = #selector(markConfigDirty)
        statusStack.orientation = .vertical
        statusStack.spacing = 6
        statusStack.alignment = .leading
        loadConfigText()
    }

    func loadConfigText() {
        if let text = try? String(contentsOfFile: configPath, encoding: .utf8) {
            configView.string = text
            loadControls(from: text)
            messageField.stringValue = ""
        } else {
            messageField.stringValue = "Configuration file could not be read"
        }
    }

    @objc func markConfigDirty() {
        messageField.stringValue = "Unsaved changes"
    }

    @objc func loadSelectedConfig() {
        loadConfigText()
        refresh()
    }

    func tomlValue(_ key: String, section: String, in text: String) -> String? {
        var currentSection = ""
        for line in text.components(separatedBy: .newlines) {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            if trimmed.hasPrefix("[") && trimmed.hasSuffix("]") {
                currentSection = String(trimmed.dropFirst().dropLast())
                continue
            }
            if currentSection == section,
               let equals = trimmed.firstIndex(of: "=") {
                let foundKey = trimmed[..<equals].trimmingCharacters(in: .whitespaces)
                if foundKey == key {
                    return String(trimmed[trimmed.index(after: equals)...])
                        .trimmingCharacters(in: .whitespaces)
                        .trimmingCharacters(in: CharacterSet(charactersIn: "\""))
                }
            }
        }
        return nil
    }

    func loadControls(from text: String) {
        if let target = tomlValue("target_dir", section: "", in: text) {
            targetDirField.stringValue = target
        }
        if let watched = tomlValue("dirs", section: "watch", in: text) {
            watchDirsField.stringValue = parseWatchedFolders(watched).joined(separator: ", ")
        }
        stabilityField.stringValue = tomlValue("stability_wait_ms", section: "watch", in: text) ?? "5000"
        debounceField.stringValue = tomlValue("debounce_ms", section: "watch", in: text) ?? "1000"

        let language = tomlValue("language", section: "general", in: text) ?? "en"
        languagePopUp.selectItem(at: language == "de" ? 1 : 0)
        if let preserve = tomlValue("preserve_source_paths", section: "general", in: text) {
            preservePopUp.selectItem(withTitle: preserve)
        }
        if let dedupe = tomlValue("on_duplicate", section: "dedupe", in: text) {
            dedupePopUp.selectItem(withTitle: dedupe)
        }
        notifySwitch.state = tomlValue("enabled", section: "notify", in: text) == "true" ? .on : .off
        tagsSwitch.state = tomlValue("enabled", section: "tags", in: text) == "true" ? .on : .off

    }

    func parseWatchedFolders(_ value: String) -> [String] {
        let trimmed = value.trimmingCharacters(in: .whitespaces)
        guard trimmed.first == "[", trimmed.last == "]" else {
            return [trimmed.trimmingCharacters(in: CharacterSet(charactersIn: "\""))]
        }
        let inner = String(trimmed.dropFirst().dropLast())
        return inner.components(separatedBy: ",").map { item in
            item.trimmingCharacters(in: .whitespaces)
                .trimmingCharacters(in: CharacterSet(charactersIn: "\""))
        }.filter { !$0.isEmpty }
    }

    func watchedFoldersToml(_ value: String) -> String {
        let folders = value.components(separatedBy: ",").map { item in
            item.trimmingCharacters(in: .whitespaces)
        }.filter { !$0.isEmpty }
        let encoded = folders.map { folder in
            "\"\(folder.replacingOccurrences(of: "\"", with: "\\\""))\""
        }.joined(separator: ", ")
        return "[\(encoded)]"
    }

    @objc func refresh() {
        if let text = try? String(contentsOfFile: configPath, encoding: .utf8) {
            if configView.string.isEmpty {
                configView.string = text
            }
        }
        let statusResult: CoreResult<CoreStatus> = callCore(configPath) { path in
            onefolder_status(path)
        }
        if let error = statusResult.error {
            messageField.stringValue = error
            return
        }
        if let status = statusResult.value {
            updateStatus(status)
        }

        let historyResult: CoreResult<ActivitySummary> = callCore(configPath) { path in
            onefolder_history(path)
        }
        if let error = historyResult.error {
            messageField.stringValue = error
        } else if let history = historyResult.value {
            historyView.string = history.records.prefix(20).map { record in
                "\(record.timestamp) · \(record.action) · \(record.src)"
            }.joined(separator: "\n")
            if history.records.isEmpty {
                historyView.string = "No activity"
            }
        }
    }

    func updateStatus(_ status: CoreStatus) {
        statusStack.arrangedSubviews.forEach { $0.removeFromSuperview() }
        let rows: [(String, String)] = [
            ("Target folder", status.target_dir),
            ("Folder exists", status.target_exists ? "Yes" : "No"),
            ("Watched folders", String(status.watched_dirs.count)),
            ("Missing folders", status.missing_watched_dirs.isEmpty
                ? "None"
                : status.missing_watched_dirs.joined(separator: ", ")),
            ("Pending operation", status.pending_operation ? "Yes" : "No")
        ]
        for (key, value) in rows {
            let row = statusRow(key, value)
            statusStack.addArrangedSubview(row)
            row.widthAnchor.constraint(equalTo: statusStack.widthAnchor).isActive = true
        }
    }

    func statusRow(_ key: String, _ value: String) -> NSView {
        let keyLabel = NSTextField(labelWithString: key)
        keyLabel.font = .systemFont(ofSize: 12)
        keyLabel.textColor = .secondaryLabelColor
        keyLabel.widthAnchor.constraint(equalToConstant: 120).isActive = true
        let valueLabel = NSTextField(labelWithString: value)
        valueLabel.font = .systemFont(ofSize: 12)
        valueLabel.lineBreakMode = .byTruncatingMiddle
        valueLabel.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        let row = NSStackView(views: [keyLabel, valueLabel])
        row.orientation = .horizontal
        row.spacing = 8
        row.alignment = .firstBaseline
        return row
    }

    @objc func previewOnce() {
        let result: CoreResult<PreviewSummary> = callCore(configPath) { path in
            onefolder_preview(path)
        }
        if let error = result.error {
            messageField.stringValue = error
            return
        }
        guard let preview = result.value else { return }
        previewView.string = preview.operations.map { operation in
            "\(operation.action): \(operation.src) → \(operation.dst) [\(operation.category)]"
        }.joined(separator: "\n")
        if preview.operations.isEmpty {
            previewView.string = "No operations previewed"
        }
        messageField.stringValue =
            "\(preview.items_checked) item(s) checked, \(preview.operations.count) operation(s) previewed"
    }

    @objc func runOnce() {
        let result: CoreResult<RunSummary> = configPath.withCString { pointer in
            let cResult = onefolder_run_once(pointer, 0)
            defer {
                if let json = cResult.json { onefolder_free_string(json) }
                if let error = cResult.error { onefolder_free_string(error) }
            }
            if let errorPointer = cResult.error,
               let error = String(validatingCString: errorPointer),
               !error.isEmpty {
                return CoreResult(value: nil, error: error)
            }
            guard let jsonPointer = cResult.json,
                  let json = String(validatingCString: jsonPointer),
                  let data = json.data(using: .utf8),
                  let value = try? JSONDecoder().decode(RunSummary.self, from: data) else {
                return CoreResult(value: nil, error: "The Rust core returned no data")
            }
            return CoreResult(value: value, error: nil)
        }
        if let error = result.error {
            messageField.stringValue = error
            return
        }
        messageField.stringValue = "One-shot run completed"
        refresh()
    }

    @objc func undoLast() {
        let result: CoreResult<UndoSummary> = configPath.withCString { pointer in
            let cResult = onefolder_undo_last(pointer, 0)
            defer {
                if let json = cResult.json { onefolder_free_string(json) }
                if let error = cResult.error { onefolder_free_string(error) }
            }
            if let errorPointer = cResult.error,
               let error = String(validatingCString: errorPointer),
               !error.isEmpty {
                return CoreResult(value: nil, error: error)
            }
            guard let jsonPointer = cResult.json,
                  let json = String(validatingCString: jsonPointer),
                  let data = json.data(using: .utf8),
                  let value = try? JSONDecoder().decode(UndoSummary.self, from: data) else {
                return CoreResult(value: nil, error: "The Rust core returned no data")
            }
            return CoreResult(value: value, error: nil)
        }
        if let error = result.error {
            messageField.stringValue = error
            return
        }
        messageField.stringValue = "Restored \(result.value?.restored ?? 0) operation(s)"
        refresh()
    }

    @objc func setTargetIcon() {
        let result: CoreResult<TargetIconSummary> = configPath.withCString { pointer in
            let cResult = onefolder_set_target_icon(pointer)
            defer {
                if let json = cResult.json { onefolder_free_string(json) }
                if let error = cResult.error { onefolder_free_string(error) }
            }
            if let errorPointer = cResult.error,
               let error = String(validatingCString: errorPointer),
               !error.isEmpty {
                return CoreResult(value: nil, error: error)
            }
            guard let jsonPointer = cResult.json,
                  let json = String(validatingCString: jsonPointer),
                  let data = json.data(using: .utf8),
                  let value = try? JSONDecoder().decode(TargetIconSummary.self, from: data) else {
                return CoreResult(value: nil, error: "The Rust core returned no data")
            }
            return CoreResult(value: value, error: nil)
        }
        if let error = result.error {
            messageField.stringValue = error
            return
        }
        messageField.stringValue = "Set target folder icon: \(result.value?.target_dir ?? "")"
        refresh()
    }

    @objc func saveConfig() {
        applyControlsToConfig()
        let result: CoreResult<SaveConfigSummary> = configPath.withCString { pathPointer in
            configView.string.withCString { textPointer in
                let cResult = onefolder_save_config(pathPointer, textPointer)
                defer {
                    if let json = cResult.json { onefolder_free_string(json) }
                    if let error = cResult.error { onefolder_free_string(error) }
                }
                if let errorPointer = cResult.error,
                   let error = String(validatingCString: errorPointer),
                   !error.isEmpty {
                    return CoreResult(value: nil, error: error)
                }
                guard let jsonPointer = cResult.json,
                      let json = String(validatingCString: jsonPointer),
                      let data = json.data(using: .utf8),
                      let value = try? JSONDecoder().decode(SaveConfigSummary.self, from: data) else {
                    return CoreResult(value: nil, error: "The Rust core returned no data")
                }
                return CoreResult(value: value, error: nil)
            }
        }
        if let error = result.error {
            messageField.stringValue = error
            return
        }

        let uid = getuid()
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/launchctl")
        process.arguments = ["kickstart", "-k", "gui/\(uid)/de.korwisi.onefolder"]
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        do {
            try process.run()
            process.waitUntilExit()
            if process.terminationStatus == 0 {
                messageField.stringValue = "Configuration saved; LaunchAgent restarted"
            } else {
                messageField.stringValue = "Configuration saved; LaunchAgent was not running"
            }
        } catch {
            messageField.stringValue = "Configuration saved; could not restart LaunchAgent"
        }
        refresh()
    }

    func replaceSetting(_ text: String, section: String, key: String, value: String) -> String {
        var currentSection = ""
        var result: [String] = []
        for line in text.components(separatedBy: .newlines) {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            if trimmed.hasPrefix("[") && trimmed.hasSuffix("]") {
                currentSection = String(trimmed.dropFirst().dropLast())
                result.append(line)
                continue
            }
            if currentSection == section,
               let equals = trimmed.firstIndex(of: "=") {
                let foundKey = trimmed[..<equals].trimmingCharacters(in: .whitespaces)
                if foundKey == key {
                    result.append("\(key) = \(value)")
                    continue
                }
            }
            result.append(line)
        }
        return result.joined(separator: "\n")
    }

    func applyControlsToConfig() {
        var text = configView.string
        text = replaceSetting(text, section: "", key: "target_dir",
                              value: "\"\(targetDirField.stringValue.replacingOccurrences(of: "\"", with: "\\\""))\"")
        text = replaceSetting(text, section: "watch", key: "dirs",
                              value: watchedFoldersToml(watchDirsField.stringValue))
        text = replaceSetting(text, section: "watch", key: "stability_wait_ms",
                              value: stabilityField.stringValue)
        text = replaceSetting(text, section: "watch", key: "debounce_ms",
                              value: debounceField.stringValue)
        text = replaceSetting(text, section: "general", key: "language",
                              value: languagePopUp.indexOfSelectedItem == 1 ? "\"de\"" : "\"en\"")
        text = replaceSetting(text, section: "general", key: "preserve_source_paths",
                              value: "\"\(preservePopUp.titleOfSelectedItem ?? "off")\"")
        text = replaceSetting(text, section: "dedupe", key: "on_duplicate",
                              value: "\"\(dedupePopUp.titleOfSelectedItem ?? "skip")\"")
        text = replaceSetting(text, section: "notify", key: "enabled",
                              value: notifySwitch.state == .on ? "true" : "false")
        text = replaceSetting(text, section: "tags", key: "enabled",
                              value: tagsSwitch.state == .on ? "true" : "false")
        configView.string = text
    }

}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    var window: NSWindow?
    let controller = DesktopController()

    func applicationDidFinishLaunching(_ notification: Notification) {
        setApplicationIcon()
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 1120, height: 840),
            styleMask: [.titled, .closable, .resizable, .miniaturizable],
            backing: .buffered,
            defer: false
        )
        window.title = "1folder"
        window.minSize = NSSize(width: 940, height: 660)

        controller.configField.placeholderString = "Path to config.toml"
        controller.configField.bezelStyle = .roundedBezel
        controller.configField.isEditable = true
        controller.configField.font = .monospacedSystemFont(ofSize: 12, weight: .regular)
        controller.configField.target = controller
        controller.configField.action = #selector(DesktopController.loadSelectedConfig)

        let integerFormatter = NumberFormatter()
        integerFormatter.numberStyle = .none
        integerFormatter.allowsFloats = false
        integerFormatter.minimum = 0
        controller.stabilityField.formatter = integerFormatter
        controller.debounceField.formatter = integerFormatter

        controller.messageField.font = .systemFont(ofSize: 12)
        controller.messageField.textColor = .secondaryLabelColor
        controller.messageField.lineBreakMode = .byTruncatingTail
        controller.messageField.setContentHuggingPriority(.defaultLow, for: .horizontal)
        controller.messageField.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

        let saveButton = NSButton(title: "Save & restart agent", target: controller,
                                  action: #selector(DesktopController.saveConfig))
        saveButton.bezelStyle = .rounded
        saveButton.keyEquivalent = "\r"
        let toolbar = NSStackView(views: [
            controller.messageField,
            NSButton(title: "Refresh", target: controller, action: #selector(DesktopController.refresh)),
            NSButton(title: "Preview", target: controller, action: #selector(DesktopController.previewOnce)),
            NSButton(title: "Run once", target: controller, action: #selector(DesktopController.runOnce)),
            NSButton(title: "Set target icon", target: controller, action: #selector(DesktopController.setTargetIcon)),
            NSButton(title: "Undo last", target: controller, action: #selector(DesktopController.undoLast)),
            saveButton
        ])
        toolbar.orientation = .horizontal
        toolbar.spacing = 8
        toolbar.alignment = .centerY
        toolbar.setCustomSpacing(16, after: controller.messageField)

        let pathRow = settingsRow("Config file", controller.configField)

        let foldersCard = card("Folders", [
            settingsRow("Target folder", controller.targetDirField),
            settingsRow("Watched folders", controller.watchDirsField)
        ])
        let statusCard = card("Status", [controller.statusStack])
        let settingsLeftColumn = NSStackView(views: [foldersCard, statusCard])
        settingsLeftColumn.orientation = .vertical
        settingsLeftColumn.spacing = 16
        settingsLeftColumn.alignment = .leading
        for section in [foldersCard, statusCard] {
            section.widthAnchor.constraint(equalTo: settingsLeftColumn.widthAnchor).isActive = true
        }

        let behaviorCard = card("Behavior", [
            settingsRow("Language", controller.languagePopUp),
            settingsRow("Source paths", controller.preservePopUp),
            settingsRow("Duplicates", controller.dedupePopUp),
            settingsRow("Stability", controller.stabilityField),
            settingsRow("Debounce", controller.debounceField),
            switchRow("Notifications", controller.notifySwitch),
            switchRow("Finder tags", controller.tagsSwitch)
        ])
        let settingsRowStack = NSStackView(views: [settingsLeftColumn, behaviorCard])
        settingsRowStack.orientation = .horizontal
        settingsRowStack.distribution = .fillEqually
        settingsRowStack.spacing = 16
        settingsRowStack.alignment = .top

        let previewCard = textCard("Preview", controller.previewView, editable: false, height: 170)
        let activityCard = textCard("Recent activity", controller.historyView, editable: false, height: 170)
        let bottomRow = NSStackView(views: [previewCard, activityCard])
        bottomRow.orientation = .horizontal
        bottomRow.distribution = .fillEqually
        bottomRow.spacing = 16
        bottomRow.alignment = .top

        let tomlCard = textCard("Configuration (TOML)", controller.configView, editable: true, height: 240)

        let content = NSView()
        content.wantsLayer = true
        content.layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor

        let root = NSStackView()
        root.orientation = .vertical
        root.spacing = 16
        root.alignment = .leading
        root.edgeInsets = NSEdgeInsets(top: 20, left: 24, bottom: 20, right: 24)
        root.translatesAutoresizingMaskIntoConstraints = false
        content.addSubview(root)
        NSLayoutConstraint.activate([
            root.leadingAnchor.constraint(equalTo: content.leadingAnchor),
            root.trailingAnchor.constraint(equalTo: content.trailingAnchor),
            root.topAnchor.constraint(equalTo: content.topAnchor),
            root.bottomAnchor.constraint(equalTo: content.bottomAnchor)
        ])

        root.addArrangedSubview(pathRow)
        root.addArrangedSubview(toolbar)
        root.addArrangedSubview(settingsRowStack)
        root.addArrangedSubview(bottomRow)
        root.addArrangedSubview(tomlCard)

        for section in [pathRow, toolbar, settingsRowStack, bottomRow, tomlCard] {
            section.widthAnchor.constraint(equalTo: root.widthAnchor, constant: -48).isActive = true
        }

        window.contentView = content
        window.center()
        window.makeKeyAndOrderFront(nil)
        self.window = window
        controller.loadConfigText()
        controller.refresh()
    }

    func setApplicationIcon() {
        let iconCandidates = [
            Bundle.main.resourceURL?.appendingPathComponent("onefolder.icns").path,
            URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
                .deletingLastPathComponent()
                .appendingPathComponent("assets/icon/onefolder.icns")
                .path
        ].compactMap { $0 }

        for path in iconCandidates {
            if let icon = NSImage(contentsOfFile: path) {
                NSApp.applicationIconImage = icon
                break
            }
        }
    }

    func textCard(_ title: String, _ textView: NSTextView, editable: Bool, height: CGFloat) -> NSView {
        textView.isEditable = editable
        textView.isSelectable = true
        textView.font = .monospacedSystemFont(ofSize: 11, weight: .regular)
        textView.textColor = .labelColor
        textView.backgroundColor = .textBackgroundColor
        textView.drawsBackground = true
        textView.textContainerInset = NSSize(width: 8, height: 8)
        textView.textContainer?.widthTracksTextView = true
        textView.isVerticallyResizable = true
        textView.isHorizontallyResizable = false
        textView.autoresizingMask = [.width]
        textView.minSize = NSSize(width: 0, height: 0)
        textView.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude,
                                  height: CGFloat.greatestFiniteMagnitude)

        let scroll = NSScrollView()
        scroll.documentView = textView
        scroll.hasVerticalScroller = true
        scroll.drawsBackground = false
        scroll.wantsLayer = true
        scroll.layer?.cornerRadius = 6
        scroll.layer?.masksToBounds = true
        scroll.heightAnchor.constraint(equalToConstant: height).isActive = true

        return card(title, [scroll])
    }

    func settingsRow(_ title: String, _ control: NSControl) -> NSView {
        let titleLabel = NSTextField(labelWithString: title)
        titleLabel.font = .systemFont(ofSize: 13)
        titleLabel.textColor = .secondaryLabelColor
        titleLabel.setContentHuggingPriority(.defaultHigh, for: .horizontal)
        titleLabel.widthAnchor.constraint(equalToConstant: 130).isActive = true
        control.setContentHuggingPriority(.init(1), for: .horizontal)
        control.setContentCompressionResistancePriority(.init(1), for: .horizontal)

        let row = NSStackView(views: [titleLabel, control])
        row.orientation = .horizontal
        row.spacing = 12
        row.alignment = .centerY
        return row
    }

    func switchRow(_ title: String, _ toggle: NSSwitch) -> NSView {
        let titleLabel = NSTextField(labelWithString: title)
        titleLabel.font = .systemFont(ofSize: 13)
        titleLabel.textColor = .secondaryLabelColor
        let spacer = NSView()
        let row = NSStackView(views: [titleLabel, spacer, toggle])
        row.orientation = .horizontal
        row.spacing = 12
        row.alignment = .centerY
        return row
    }

    func card(_ title: String, _ rows: [NSView]) -> NSView {
        let titleLabel = NSTextField(labelWithString: title)
        titleLabel.font = .systemFont(ofSize: 13, weight: .semibold)

        let column = NSStackView(views: rows)
        column.orientation = .vertical
        column.spacing = 10
        column.alignment = .leading
        for row in rows {
            row.widthAnchor.constraint(equalTo: column.widthAnchor).isActive = true
        }

        let section = NSStackView(views: [titleLabel, column])
        section.orientation = .vertical
        section.spacing = 12
        section.alignment = .leading
        section.edgeInsets = NSEdgeInsets(top: 14, left: 16, bottom: 14, right: 16)
        section.wantsLayer = true
        section.layer?.cornerRadius = 10
        section.layer?.backgroundColor = NSColor.controlBackgroundColor.cgColor
        section.layer?.borderWidth = 1
        section.layer?.borderColor = NSColor.separatorColor.withAlphaComponent(0.5).cgColor
        column.widthAnchor.constraint(equalTo: section.widthAnchor, constant: -32).isActive = true
        return section
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }
}

@MainActor
func runApplication() {
    let app = NSApplication.shared
    app.setActivationPolicy(.regular)
    let delegate = AppDelegate()
    app.delegate = delegate
    app.activate(ignoringOtherApps: true)
    app.run()
}

runApplication()
