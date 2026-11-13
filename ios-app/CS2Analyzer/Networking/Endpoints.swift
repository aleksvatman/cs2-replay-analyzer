import Foundation

/// URL сервера (`server-doc.md`, разд. 5) по базовому адресу из настроек.
enum Endpoints {
    static func health(base: URL) -> URL { base.appendingPathComponent("health") }
    static func matches(base: URL) -> URL { base.appendingPathComponent("matches") }
    static func match(base: URL, id: String) -> URL { base.appendingPathComponent("matches/\(id)") }
    static func matchData(base: URL, id: String) -> URL { base.appendingPathComponent("matches/\(id)/data") }
    static func reparse(base: URL, id: String) -> URL { base.appendingPathComponent("matches/\(id)/reparse") }
}
