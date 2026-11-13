import Foundation

/// Калибровка радара одной карты (этап I3, `ipad-doc.md` разд. 6). Числа — `pos_x`,
/// `pos_y`, `scale` из игрового `resource/overviews/<карта>.txt` (формат HLTV overview),
/// см. `Resources/Maps/README.md` — откуда они взяты и как добавить картинку радара.
struct RadarCalibration: Codable, Equatable {
    var map: String
    /// Имя файла картинки радара (её самой в репозитории нет, см. README рядом).
    var image: String
    var posX: Double
    var posY: Double
    var scale: Double
    /// Сторона квадратного радара в пикселях (обычно 1024 у карт CS2/CS:GO).
    var imageSize: Double = 1024
    /// Карты с несколькими уровнями (Nuke, Vertigo) — не в первой версии; пустой список
    /// у обычных карт. Поле заведено сразу, чтобы не ломать формат калибровки позже.
    var levels: [RadarLevel] = []
}

struct RadarLevel: Codable, Equatable {
    var name: String
    var image: String
    /// Порог высоты (`z`, десятые игровой единицы): выше этого — верхний уровень.
    var zThreshold: Int
}

/// Загрузка калибровки из бандла приложения (`Resources/Maps/<карта>.json`).
enum RadarCalibrationStore {
    private static var cache: [String: RadarCalibration?] = [:]

    /// `nil`, если калибровки для карты нет — приложение должно показать предупреждение,
    /// а не считать это ошибкой (не у всех карт калибровка будет в первой версии).
    static func calibration(for map: String, bundle: Bundle = .main) -> RadarCalibration? {
        if let cached = cache[map] {
            return cached
        }
        let result = load(map: map, bundle: bundle)
        cache[map] = result
        return result
    }

    private static func load(map: String, bundle: Bundle) -> RadarCalibration? {
        guard let url = bundle.url(forResource: map, withExtension: "json"),
              let data = try? Data(contentsOf: url) else {
            return nil
        }
        return try? JSONDecoder().decode(RadarCalibration.self, from: data)
    }
}
