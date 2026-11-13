import Foundation
import Observation

/// Настройки приложения (этап I6, `ipad-doc.md` разд. 11) — пока только лимит кэша; без
/// онлайн-разбора (разд. 8a) выбора источника и серверных настроек тут нет.
@Observable
final class AppSettings {
    static let defaultMaxCacheBytes = 1024 * 1_048_576 // 1 ГБ
    /// Варианты для `Picker` в `SettingsView`; в мегабайтах.
    static let cacheSizeOptionsMB = [256, 512, 1024, 2048, 4096]

    private static let maxCacheBytesKey = "maxCacheBytes"
    private let defaults: UserDefaults

    var maxCacheBytes: Int {
        didSet { defaults.set(maxCacheBytes, forKey: Self.maxCacheBytesKey) }
    }

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        let stored = defaults.integer(forKey: Self.maxCacheBytesKey)
        maxCacheBytes = stored > 0 ? stored : Self.defaultMaxCacheBytes
    }
}
