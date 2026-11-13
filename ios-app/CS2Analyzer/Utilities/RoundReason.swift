import Foundation

/// Коды причины конца раунда (`docs/format.md`, раздел `rounds`). Неизвестные коды
/// показываем как есть — формат не гарантирует полноту списка.
enum RoundReason {
    static func description(_ code: Int) -> String {
        switch code {
        case 1: return "Взрыв бомбы"
        case 7: return "Бомба разминирована"
        case 8: return "Уничтожены все T"
        case 9: return "Уничтожены все CT"
        case 18: return "Сдача CT"
        default: return "Код \(code)"
        }
    }
}

extension Team {
    var label: String {
        switch self {
        case .t: return "T"
        case .ct: return "CT"
        }
    }
}
