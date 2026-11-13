import Foundation

/// Внутренние имена оружия в событиях (`ak47`) и имена в инвентаре/словаре `weapons`
/// (`AK-47`) — разные записи (`CLAUDE.md`, «известные особенности данных»). Эта таблица
/// даёт человекочитаемое имя и иконку по внутреннему имени события; для имени из
/// инвентаря (уже читаемое) `displayName` возвращает его почти как есть.
enum WeaponNames {
    /// SF Symbols — точных силуэтов оружия в системном наборе нет; все огнестрельные
    /// категории показываются одной и той же иконкой прицела, чтобы в ленте убийств не
    /// выделялся «просто кружок» у пистолетов на фоне остального оружия.
    enum Icon: String {
        case gun = "scope"
        case grenade = "aqi.medium"
        case knife = "wrench.and.screwdriver.fill"
        case world = "bolt.fill"
        case unknown = "questionmark.circle"
    }

    private struct Entry {
        let display: String
        let icon: Icon
    }

    private static let table: [String: Entry] = [
        // Пистолеты
        "glock": Entry(display: "Glock-18", icon: .gun),
        "usp_silencer": Entry(display: "USP-S", icon: .gun),
        "hkp2000": Entry(display: "P2000", icon: .gun),
        "p250": Entry(display: "P250", icon: .gun),
        "fiveseven": Entry(display: "Five-SeveN", icon: .gun),
        "cz75a": Entry(display: "CZ75-Auto", icon: .gun),
        "tec9": Entry(display: "Tec-9", icon: .gun),
        "elite": Entry(display: "Dual Berettas", icon: .gun),
        "deagle": Entry(display: "Desert Eagle", icon: .gun),
        "revolver": Entry(display: "R8 Revolver", icon: .gun),
        // Пистолет-пулемёты
        "mac10": Entry(display: "MAC-10", icon: .gun),
        "mp9": Entry(display: "MP9", icon: .gun),
        "mp7": Entry(display: "MP7", icon: .gun),
        "mp5sd": Entry(display: "MP5-SD", icon: .gun),
        "ump45": Entry(display: "UMP-45", icon: .gun),
        "p90": Entry(display: "P90", icon: .gun),
        "bizon": Entry(display: "PP-Bizon", icon: .gun),
        // Дробовики
        "nova": Entry(display: "Nova", icon: .gun),
        "xm1014": Entry(display: "XM1014", icon: .gun),
        "sawedoff": Entry(display: "Sawed-Off", icon: .gun),
        "mag7": Entry(display: "MAG-7", icon: .gun),
        // Автоматы и тяжёлое
        "m249": Entry(display: "M249", icon: .gun),
        "negev": Entry(display: "Negev", icon: .gun),
        // Штурмовые винтовки
        "ak47": Entry(display: "AK-47", icon: .gun),
        "m4a1": Entry(display: "M4A4", icon: .gun),
        "m4a1_silencer": Entry(display: "M4A1-S", icon: .gun),
        "famas": Entry(display: "FAMAS", icon: .gun),
        "galilar": Entry(display: "Galil AR", icon: .gun),
        "sg556": Entry(display: "SG 553", icon: .gun),
        "aug": Entry(display: "AUG", icon: .gun),
        // Снайперские
        "awp": Entry(display: "AWP", icon: .gun),
        "ssg08": Entry(display: "SSG 08", icon: .gun),
        "scar20": Entry(display: "SCAR-20", icon: .gun),
        "g3sg1": Entry(display: "G3SG1", icon: .gun),
        // Гранаты
        "hegrenade": Entry(display: "HE-граната", icon: .grenade),
        "flashbang": Entry(display: "Флешбанг", icon: .grenade),
        "smokegrenade": Entry(display: "Дымовая граната", icon: .grenade),
        "molotov": Entry(display: "Молотов", icon: .grenade),
        "incgrenade": Entry(display: "Зажигательная", icon: .grenade),
        "decoy": Entry(display: "Муляж", icon: .grenade),
        // Холодное оружие и прочее
        "knife": Entry(display: "Нож", icon: .knife),
        "taser": Entry(display: "Электрошокер (Zeus x27)", icon: .gun),
        "c4": Entry(display: "C4", icon: .grenade),
        // Не оружие игрока
        "world": Entry(display: "Мир (падение, взрыв)", icon: .world),
    ]

    /// Человекочитаемое имя по внутреннему имени события (`ak47`) или по имени из
    /// инвентаря/`MatchModel.weapons` (`AK-47`, уже читаемо — возвращается как есть,
    /// если не найдено в таблице событий).
    static func displayName(_ rawName: String) -> String {
        table[rawName]?.display ?? rawName
    }

    static func icon(_ rawName: String) -> Icon {
        table[rawName]?.icon ?? .unknown
    }
}
