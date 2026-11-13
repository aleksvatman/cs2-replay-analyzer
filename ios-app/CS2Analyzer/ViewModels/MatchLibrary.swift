import Foundation
import Observation

/// Список матчей: подложенные вручную в `Resources/SampleMatches` (этап I2) плюс разобранные
/// на устройстве и закэшированные в `MatchStore` (этап I9). Источник скрыт за этим классом —
/// остальной код про `MatchStore`/подложенные файлы ничего не знает, только про `items` и
/// `match(for:)`.
@Observable
final class MatchLibrary {
    private(set) var items: [MatchSummary] = []
    /// Ошибки загрузки отдельных подложенных матчей (не должны случаться при обычной
    /// сборке — список фиксирован, — но сбой одного файла не должен прятать остальные).
    private(set) var loadErrors: [String] = []

    private var matchesById: [String: MatchModel] = [:]
    private let store: MatchStore

    private struct Entry {
        let id: String
        let resource: String
    }

    private static let manifest: [Entry] = [
        Entry(id: "sample-mirage", resource: "sample_mirage"),
        Entry(id: "sample-ancient", resource: "sample_ancient"),
    ]

    init(store: MatchStore = MatchStore()) {
        self.store = store
        reload()
    }

    func match(for id: String) -> MatchModel? {
        if let match = matchesById[id] { return match }
        guard let match = try? store.loadMatch(for: id) else { return nil }
        matchesById[id] = match
        store.touch(id: id)
        return match
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

        for meta in store.listMeta() {
            items.append(meta.summary)
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

    /// Разбирает выбранную демку на устройстве (этап I9) и добавляет результат в список.
    /// Хэширует файл первым делом: если демка уже разобрана и версия формата не устарела,
    /// повторный разбор пропускается (`ipad-doc.md` разд. 9). `onProgress` зовётся с фонового
    /// потока — вызывающая сторона сама решает, как и когда переключиться на главный.
    @discardableResult
    func importDemo(at url: URL, onProgress: @escaping (ParseProgress) -> Void) async throws -> MatchSummary {
        let store = self.store
        let meta = try await Task.detached(priority: .userInitiated) {
            try withDemoFileAccess(url) { localURL -> CachedMatchMeta in
                let id = try DemoHasher.sha256Hex(ofFileAt: localURL)
                if let cached = store.meta(for: id), cached.schemaVersion == supportedSchemaVersion {
                    store.touch(id: id)
                    return cached
                }

                let tmpOut = FileManager.default.temporaryDirectory
                    .appendingPathComponent("import-\(UUID().uuidString).json.gz")
                defer { try? FileManager.default.removeItem(at: tmpOut) }
                try LocalParser.parse(demoPath: localURL.path, outPath: tmpOut.path, onProgress: onProgress)

                let gzipped = try Data(contentsOf: tmpOut)
                let json = try gzipped.gunzipped()
                let match = try MatchModel.decode(from: json)
                let meta = CachedMatchMeta(
                    id: id, map: match.map, scoreT: match.score.t, scoreCT: match.score.ct,
                    roundsCount: match.rounds.count, playersCount: match.players.count,
                    schemaVersion: match.schemaVersion, parsedAt: Date(), lastOpenedAt: Date(),
                    sizeBytes: gzipped.count
                )
                try store.store(id: id, gzippedJSON: gzipped, meta: meta)
                return meta
            }
        }.value

        await MainActor.run { reload() }
        return meta.summary
    }
}
