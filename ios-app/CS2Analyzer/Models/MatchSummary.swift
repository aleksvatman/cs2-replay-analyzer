import Foundation

/// Строка списка матчей (этап I2): то немногое, что нужно показать до открытия матча
/// целиком. Источник данных (сервер/локальный разбор, этапы I5-I9) пока не подключён —
/// список собирается из матчей, подложенных вручную в `Resources/SampleMatches`.
struct MatchSummary: Identifiable, Equatable {
    var id: String
    var map: String
    /// Итоговый счёт по картам раундов (не по "хавтаймам"): раунды, выигранные T и CT.
    var scoreT: Int
    var scoreCT: Int
    var roundsCount: Int
    var playersCount: Int
}

extension MatchModel {
    /// Итоговый счёт матча: сколько раундов выиграла каждая сторона (`winner == nil`
    /// у раунда без подсчитанного исхода в счёт не идёт — так в формате не бывает,
    /// но парсер это не гарантирует отдельным инвариантом).
    var score: (t: Int, ct: Int) {
        rounds.reduce(into: (t: 0, ct: 0)) { score, round in
            switch round.winner {
            case .t: score.t += 1
            case .ct: score.ct += 1
            case nil: break
            }
        }
    }

    /// Имя игрока по индексу (`Player.index`); `nil` для индекса "никто" (`-1`) и
    /// индексов, которых почему-то нет в `players` (не должно случаться по формату).
    func playerName(_ index: Int) -> String? {
        players.first { $0.index == index }?.name
    }
}
