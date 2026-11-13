import Foundation

/// JSON-контракт, `docs/format.md` (schemaVersion 1). Структуры один в один как
/// `parser-core/src/models.rs`; ключи JSON уже `camelCase`, поэтому своих `CodingKeys`
/// почти нигде не нужно.
enum MatchModelError: Error, LocalizedError {
    case unsupportedSchemaVersion(Int)

    var errorDescription: String? {
        switch self {
        case .unsupportedSchemaVersion(let version):
            return "Неподдерживаемая версия формата: \(version) (ожидалась 1)"
        }
    }
}

/// Поддерживаемая версия формата (`docs/format.md`). Несовместимое изменение формата
/// повышает `schemaVersion` в `parser-core`.
let supportedSchemaVersion = 1

struct MatchModel: Codable, Equatable {
    var schemaVersion: Int
    var map: String
    var tickRate: Int
    var players: [Player]
    /// Индекс оружия/предмета -> название. В данных только индексы (порядок первого появления).
    var weapons: [String]
    var rounds: [Round]
    var stats: Stats
}

extension MatchModel {
    /// Декодирует JSON и проверяет `schemaVersion`; остальной код приложения
    /// от неподдерживаемой версии не защищён.
    static func decode(from data: Data, using decoder: JSONDecoder = JSONDecoder()) throws -> MatchModel {
        let match = try decoder.decode(MatchModel.self, from: data)
        guard match.schemaVersion == supportedSchemaVersion else {
            throw MatchModelError.unsupportedSchemaVersion(match.schemaVersion)
        }
        return match
    }
}

enum Team: String, Codable, Equatable {
    case t = "T"
    case ct = "CT"
}

struct Player: Codable, Equatable {
    /// Индекс, на который ссылаются остальные части файла (killer, thrower, ...).
    var index: Int
    var steamId: UInt64
    var name: String
    /// Сторона в конце матча; команды меняются на полувремени, сторона в конкретном
    /// раунде хранится в `Round.teams`.
    var finalTeam: Team?
}
