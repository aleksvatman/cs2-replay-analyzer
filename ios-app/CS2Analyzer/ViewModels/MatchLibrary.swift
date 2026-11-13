import Foundation
import Observation

/// Этап I2: список матчей из JSON, подложенных вручную в `Resources/SampleMatches`.
/// Это временная замена `MatchSource` (сеть — этап I5, локальный разбор — I9):
/// то же место в архитектуре займёт протокол, который даёт список и данные матча,
/// не раскрывая, откуда они взялись.
@Observable
final class MatchLibrary {
    private(set) var items: [MatchSummary] = []
    /// Ошибки загрузки отдельных подложенных матчей (не должны случаться при обычной
    /// сборке — список фиксирован, — но сбой одного файла не должен прятать остальные).
    private(set) var loadErrors: [String] = []

    private var matchesById: [String: MatchModel] = [:]

    private struct Entry {
        let id: String
        let resource: String
    }

    private static let manifest: [Entry] = [
        Entry(id: "sample-mirage", resource: "sample_mirage"),
        Entry(id: "sample-ancient", resource: "sample_ancient"),
    ]

    init() {
        reload()
    }

    func match(for id: String) -> MatchModel? {
        matchesById[id]
    }

    func reload() {
        var items: [MatchSummary] = []
        var errors: [String] = []
        var matchesById: [String: MatchModel] = [:]

        for entry in Self.manifest {
            do {
                let match = try Self.loadMatch(resource: entry.resource)
                matchesById[entry.id] = match
                let score = match.score
                items.append(
                    MatchSummary(
                        id: entry.id,
                        map: match.map,
                        scoreT: score.t,
                        scoreCT: score.ct,
                        roundsCount: match.rounds.count,
                        playersCount: match.players.count
                    )
                )
            } catch {
                errors.append("\(entry.resource): \(error.localizedDescription)")
            }
        }

        self.items = items
        self.loadErrors = errors
        self.matchesById = matchesById
    }

    private static func loadMatch(resource: String) throws -> MatchModel {
        guard let url = Bundle.main.url(forResource: resource, withExtension: "json.gz") else {
            throw CocoaError(.fileNoSuchFile)
        }
        let gzipped = try Data(contentsOf: url)
        let json = try gzipped.gunzipped()
        return try MatchModel.decode(from: json)
    }
}
