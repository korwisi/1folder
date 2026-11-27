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

@MainActor
final class DesktopController: NSObject {
    let configField = NSTextField()
    let statusField = NSTextField(labelWithString: "")
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
    }

    @objc func refresh() {
        let statusResult: CoreResult<CoreStatus> = callCore(configPath) { path in
            onefolder_status(path)
        }
        if let error = statusResult.error {
            messageField.stringValue = error
            return
        }
        if let status = statusResult.value {
            statusField.stringValue = """
            Target: \(status.target_dir)
            Target exists: \(status.target_exists)
            Watched folders: \(status.watched_dirs.count)
            Missing folders: \(status.missing_watched_dirs.count)
            Pending operation: \(status.pending_operation)
            """
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
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    var window: NSWindow?
    let controller = DesktopController()

    func applicationDidFinishLaunching(_ notification: Notification) {
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 900, height: 620),
            styleMask: [.titled, .closable, .resizable, .miniaturizable],
            backing: .buffered,
            defer: false
        )
        window.title = "1folder"

        let root = NSStackView()
        root.orientation = .vertical
        root.spacing = 12
        root.edgeInsets = NSEdgeInsets(top: 16, left: 16, bottom: 16, right: 16)

        let configLabel = NSTextField(labelWithString: "Config")
        controller.configField.placeholderString = "Configuration TOML path"

        let buttonRow = NSStackView(views: [
            NSButton(title: "Refresh", target: controller, action: #selector(DesktopController.refresh)),
            NSButton(title: "Preview", target: controller, action: #selector(DesktopController.previewOnce)),
            NSButton(title: "Run once", target: controller, action: #selector(DesktopController.runOnce)),
            NSButton(title: "Undo last", target: controller, action: #selector(DesktopController.undoLast))
        ])
        buttonRow.orientation = .horizontal
        buttonRow.spacing = 8

        let contentRow = NSStackView()
        contentRow.orientation = .horizontal
        contentRow.distribution = .fillEqually
        contentRow.spacing = 12
        contentRow.addArrangedSubview(textSection("Preview", controller.previewView))
        contentRow.addArrangedSubview(textSection("Recent activity", controller.historyView))

        root.addArrangedSubview(configLabel)
        root.addArrangedSubview(controller.configField)
        root.addArrangedSubview(buttonRow)
        root.addArrangedSubview(controller.statusField)
        root.addArrangedSubview(controller.messageField)
        root.addArrangedSubview(contentRow)

        window.contentView = root
        window.center()
        window.makeKeyAndOrderFront(nil)
        self.window = window
        controller.refresh()
    }

    func textSection(_ title: String, _ textView: NSTextView) -> NSView {
        let stack = NSStackView()
        stack.orientation = .vertical
        stack.spacing = 6
        stack.addArrangedSubview(NSTextField(labelWithString: title))

        textView.isEditable = false
        textView.font = .monospacedSystemFont(ofSize: 12, weight: .regular)
        textView.textContainer?.widthTracksTextView = true
        let scroll = NSScrollView()
        scroll.documentView = textView
        scroll.hasVerticalScroller = true
        scroll.hasHorizontalScroller = true
        scroll.heightAnchor.constraint(greaterThanOrEqualToConstant: 360).isActive = true
        stack.addArrangedSubview(scroll)
        return stack
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
