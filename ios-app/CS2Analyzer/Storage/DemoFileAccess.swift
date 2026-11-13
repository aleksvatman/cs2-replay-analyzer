import CryptoKit
import Foundation

enum DemoAccessError: Error, LocalizedError {
    case accessDenied
    case coordinatorFailed(String)

    var errorDescription: String? {
        switch self {
        case .accessDenied: return "Нет доступа к выбранному файлу"
        case .coordinatorFailed(let message): return "Не удалось прочитать файл: \(message)"
        }
    }
}

/// Открывает `.dem`, выбранный через `fileImporter` (`ipad-doc.md` разд. 10): начинает
/// security-scoped доступ и координирует чтение через `NSFileCoordinator`, который сам
/// дождётся скачивания, если файл лежит в iCloud Drive. `body` вызывается синхронно на
/// текущем потоке — вызов должен уже идти в фоне, эта функция сама туда не переключается.
func withDemoFileAccess<T>(_ url: URL, _ body: (URL) throws -> T) throws -> T {
    guard url.startAccessingSecurityScopedResource() else {
        throw DemoAccessError.accessDenied
    }
    defer { url.stopAccessingSecurityScopedResource() }

    var coordinatorError: NSError?
    var bodyResult: Result<T, Error>!
    NSFileCoordinator().coordinate(readingItemAt: url, options: [], error: &coordinatorError) { coordinatedURL in
        bodyResult = Result { try body(coordinatedURL) }
    }
    if let coordinatorError {
        throw DemoAccessError.coordinatorFailed(coordinatorError.localizedDescription)
    }
    return try bodyResult.get()
}

enum DemoHasher {
    /// SHA-256 демки, потоково — демки бывают под 400 МБ, в память целиком не читаем
    /// (тот же повод, по которому `parser-core` читает их через mmap, а не `read_to_end`).
    static func sha256Hex(ofFileAt url: URL) throws -> String {
        let handle = try FileHandle(forReadingFrom: url)
        defer { try? handle.close() }

        var hasher = SHA256()
        while let chunk = try handle.read(upToCount: 4 * 1024 * 1024), !chunk.isEmpty {
            hasher.update(data: chunk)
        }
        return hasher.finalize().map { String(format: "%02x", $0) }.joined()
    }
}
