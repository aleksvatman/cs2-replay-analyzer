import Foundation

struct ErrorLogEntry: Codable, Identifiable {
    var id = UUID()
    let date: Date
    let message: String
}

/// Журнал ошибок импорта для диагностики (этап I6, `ipad-doc.md` разд. 11): последние записи
/// на диске (`Application Support/error-log.json`), кнопка «скопировать» в `SettingsView`.
/// Пути к файлам пользователя (демка, выбранная в «Файлах») вычищаются перед записью —
/// в скопированном журнале их быть не должно.
final class ErrorLog {
    static let shared = ErrorLog()

    private static let maxEntries = 50
    private let fileURL: URL
    private let encoder: JSONEncoder = {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        return encoder
    }()
    private let decoder: JSONDecoder = {
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return decoder
    }()

    init(fileURL: URL? = nil) {
        if let fileURL {
            self.fileURL = fileURL
        } else {
            let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            self.fileURL = support.appendingPathComponent("error-log.json")
        }
    }

    func append(_ message: String) {
        var entries = load()
        entries.append(ErrorLogEntry(date: Date(), message: Self.redactingPaths(message)))
        if entries.count > Self.maxEntries {
            entries.removeFirst(entries.count - Self.maxEntries)
        }
        try? encoder.encode(entries).write(to: fileURL, options: .atomic)
    }

    func load() -> [ErrorLogEntry] {
        guard let data = try? Data(contentsOf: fileURL) else { return [] }
        return (try? decoder.decode([ErrorLogEntry].self, from: data)) ?? []
    }

    func clear() {
        try? FileManager.default.removeItem(at: fileURL)
    }

    /// Текст для копирования: новые записи сверху, формат даты фиксированный (не зависит от
    /// локали устройства — так его проще читать в тикете).
    func formattedForCopying() -> String {
        let formatter = ISO8601DateFormatter()
        return load()
            .reversed()
            .map { "[\(formatter.string(from: $0.date))] \($0.message)" }
            .joined(separator: "\n")
    }

    /// Заменяет похожее на абсолютный путь файловой системы (`/Users/...`, `/var/...`,
    /// `/private/...` и любую последовательность без пробелов с двумя и более `/`) на
    /// плейсхолдер — ошибки файлового доступа (`NSFileCoordinator`, `NSError`) иногда несут
    /// путь к демке пользователя в `localizedDescription`.
    static func redactingPaths(_ message: String) -> String {
        guard let regex = try? NSRegularExpression(pattern: #"\S*/\S+/\S+"#) else { return message }
        let range = NSRange(message.startIndex..., in: message)
        return regex.stringByReplacingMatches(in: message, range: range, withTemplate: "<путь скрыт>")
    }
}
