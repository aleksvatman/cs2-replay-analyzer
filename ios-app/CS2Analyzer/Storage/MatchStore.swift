import Foundation

/// Метаданные закэшированного матча (`meta.json`) — то немногое, что нужно для списка
/// без распаковки и декодирования всего JSON.
struct CachedMatchMeta: Codable {
    let id: String
    let map: String
    let scoreT: Int
    let scoreCT: Int
    let roundsCount: Int
    let playersCount: Int
    let schemaVersion: Int
    let parsedAt: Date
    var lastOpenedAt: Date
    let sizeBytes: Int

    var summary: MatchSummary {
        MatchSummary(
            id: id, map: map, scoreT: scoreT, scoreCT: scoreCT,
            roundsCount: roundsCount, playersCount: playersCount
        )
    }
}

/// Локальный кэш разобранных матчей (`ipad-doc.md` разд. 9): `Application Support/matches/<id>/
/// {match.json.gz, meta.json}`, `id` — SHA-256 исходной демки, чтобы один и тот же файл не
/// разбирался дважды. JSON хранится сжатым — декодирование остаётся быстрым даже на iPad 7
/// (замер в I1, `summary.md`), экономия места важнее. Контроль размера кэша и ручное удаление
/// (настройки, этап I6) сюда пока не входят — здесь только механика хранения, нужная I9.
final class MatchStore {
    private let root: URL
    private let decoder: JSONDecoder = {
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return decoder
    }()
    private let encoder: JSONEncoder = {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        return encoder
    }()

    /// `rootDirectory` переопределяет каталог кэша — для тестов, чтобы не делить состояние
    /// с настоящим кэшем приложения и с другими тестами. По умолчанию `Application Support`.
    init(rootDirectory: URL? = nil) {
        if let rootDirectory {
            root = rootDirectory
        } else {
            let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            root = support.appendingPathComponent("matches", isDirectory: true)
        }
        try? FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        excludeFromBackup(root)
    }

    private func excludeFromBackup(_ url: URL) {
        var url = url
        var values = URLResourceValues()
        values.isExcludedFromBackup = true
        try? url.setResourceValues(values)
    }

    private func matchDir(_ id: String) -> URL { root.appendingPathComponent(id, isDirectory: true) }
    private func jsonURL(_ id: String) -> URL { matchDir(id).appendingPathComponent("match.json.gz") }
    private func metaURL(_ id: String) -> URL { matchDir(id).appendingPathComponent("meta.json") }

    func meta(for id: String) -> CachedMatchMeta? {
        guard let data = try? Data(contentsOf: metaURL(id)) else { return nil }
        return try? decoder.decode(CachedMatchMeta.self, from: data)
    }

    /// Все закэшированные матчи, последние открытые — первыми.
    func listMeta() -> [CachedMatchMeta] {
        guard let ids = try? FileManager.default.contentsOfDirectory(atPath: root.path) else { return [] }
        return ids.compactMap { meta(for: $0) }.sorted { $0.lastOpenedAt > $1.lastOpenedAt }
    }

    func loadMatch(for id: String) throws -> MatchModel {
        let gzipped = try Data(contentsOf: jsonURL(id))
        let json = try gzipped.gunzipped()
        return try MatchModel.decode(from: json)
    }

    /// Сохраняет уже сжатый результат разбора и его метаданные. Перезаписывает, если матч с
    /// таким `id` уже был (повторный разбор той же демки, например после обновления формата).
    func store(id: String, gzippedJSON: Data, meta: CachedMatchMeta) throws {
        let dir = matchDir(id)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        try gzippedJSON.write(to: jsonURL(id), options: .atomic)
        try encoder.encode(meta).write(to: metaURL(id), options: .atomic)
        excludeFromBackup(dir)
    }

    /// Обновляет `lastOpenedAt` на сейчас (матч открыли или решили не разбирать заново).
    func touch(id: String) {
        guard var meta = meta(for: id) else { return }
        meta.lastOpenedAt = Date()
        try? encoder.encode(meta).write(to: metaURL(id), options: .atomic)
    }

    func delete(id: String) {
        try? FileManager.default.removeItem(at: matchDir(id))
    }
}
