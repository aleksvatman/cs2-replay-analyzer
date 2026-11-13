import Foundation

/// JSON-контракт сервера (`server-doc.md`, разд. 5, 6, 10) — ключи уже `camelCase`
/// (`#[serde(rename_all = "camelCase")]` на стороне сервера), свои `CodingKeys` не нужны.
enum MatchStatus: String, Codable, Equatable {
    case queued
    case parsing
    case done
    case failed
}

/// Строка `GET /matches`.
struct MatchListItem: Codable, Equatable, Identifiable {
    var id: String
    var status: MatchStatus
    var map: String?
    var uploadedAt: UInt64
    var sizeBytes: UInt64
    var originalName: String?
    var rounds: UInt32?
    var score: [UInt32]?
    var schemaVersion: UInt32?
}

/// `GET /matches/{id}` — поля `MatchListItem` (на сервере через `#[serde(flatten)]`) плюс
/// прогресс разбора.
struct MatchDetail: Codable, Equatable, Identifiable {
    var id: String
    var status: MatchStatus
    var map: String?
    var uploadedAt: UInt64
    var sizeBytes: UInt64
    var originalName: String?
    var rounds: UInt32?
    var score: [UInt32]?
    var schemaVersion: UInt32?
    var parsedAt: UInt64?
    /// Текущая стадия разбора (`events`, `players`, `grenades`, `assemble`, `done`);
    /// `nil` пока в очереди и после ошибки.
    var stage: String?
    /// 0 в очереди, 100 когда готово.
    var percent: UInt8
    var error: String?
}

/// Ответ `POST /matches` (`202` новый матч, `200` дубликат по SHA-256 — в обоих случаях
/// тело одинаковое).
struct UploadResponse: Codable, Equatable {
    var id: String
    var status: MatchStatus
}

/// `{"error": {"code": "...", "message": "..."}}` — формат ошибок сервера (разд. 5).
struct ServerErrorBody: Codable, Equatable {
    struct Error: Codable, Equatable {
        var code: String
        var message: String
    }
    var error: Error
}
