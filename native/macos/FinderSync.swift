import Cocoa
import FinderSync

@objc(DalZipFinderSync)
final class DalZipFinderSync: FIFinderSync {
    override init() {
        super.init()
        FIFinderSyncController.default().directoryURLs = [URL(fileURLWithPath: "/")]
    }

    override func menu(for menuKind: FIMenuKind) -> NSMenu? {
        guard menuKind == .contextualMenuForItems,
              let selected = FIFinderSyncController.default().selectedItemURLs(),
              let first = selected.first else { return nil }
        let directory = (try? first.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true
        let representative = directory ? first.lastPathComponent : first.deletingPathExtension().lastPathComponent
        let menu = NSMenu()
        let item = NSMenuItem(title: "\(representative).zip으로 압축하기", action: #selector(compress(_:)), keyEquivalent: "")
        item.target = self
        item.image = NSImage(systemSymbolName: "archivebox", accessibilityDescription: "DalZip 압축")
        menu.addItem(item)
        let formats: Set<String> = ["zip", "zipx", "7z", "rar", "tar", "gz", "tgz", "bz2", "tbz2", "xz", "txz", "zst"]
        if selected.allSatisfy({ formats.contains($0.pathExtension.lowercased()) && (try? $0.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) != true }) {
            let extractItem = NSMenuItem(title: "별도 폴더에 압축 풀기", action: #selector(extract(_:)), keyEquivalent: "")
            extractItem.image = NSImage(systemSymbolName: "arrow.down.doc", accessibilityDescription: "DalZip 압축 풀기")
            menu.addItem(extractItem)
        }
        return menu
    }

    @objc func compress(_ sender: Any?) { sendSelection(action: "compress") }
    @objc func extract(_ sender: Any?) { sendSelection(action: "extract") }

    private func sendSelection(action: String) {
        // Finder는 메뉴 항목을 복제하므로 동작 시점에 공식 API로 선택 경로를 읽습니다.
        guard let selected = FIFinderSyncController.default().selectedItemURLs(), !selected.isEmpty else { return }
        var components = URLComponents()
        components.scheme = "dalzip"
        components.host = action
        components.queryItems = selected.map { URLQueryItem(name: "path", value: $0.path) }
        if let url = components.url { NSWorkspace.shared.open(url) }
    }
}
