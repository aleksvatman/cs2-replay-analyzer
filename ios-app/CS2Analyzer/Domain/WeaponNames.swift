import Foundation

/// Внутренние имена оружия в событиях (`ak47`) и имена в инвентаре/словаре `weapons`
/// (`AK-47`) — разные записи (`CLAUDE.md`, «известные особенности данных»). Эта таблица
/// даёт человекочитаемое имя и иконку по внутреннему имени события; для имени из
/// инвентаря (уже читаемое) `displayName` возвращает его почти как есть.
enum WeaponNames {
    /// SF Symbols — не точные силуэты оружия (их нет в системном наборе), просто
    /// различимые по категории значки для ленты убийств.
    enum Icon: String {
        case pistol = "circle.fill"
        case smg = "burst.fill"
        case rifle = "scope"
        case sniper = "camera.aperture"
        case heavy = "shield.fill"
        case shotgun = "wind"
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
        "glock": Entry(display: "Glock-18", icon: .pistol),
        "usp_silencer": Entry(display: "USP-S", icon: .pistol),
        "hkp2000": Entry(display: "P2000", icon: .pistol),
        "p250": Entry(display: "P250", icon: .pistol),
        "fiveseven": Entry(display: "Five-SeveN", icon: .pistol),
        "cz75a": Entry(display: "CZ75-Auto", icon: .pistol),
        "tec9": Entry(display: "Tec-9", icon: .pistol),
        "elite": Entry(display: "Dual Berettas", icon: .pistol),
        "deagle": Entry(display: "Desert Eagle", icon: .pistol),
        "revolver": Entry(display: "R8 Revolver", icon: .pistol),
        // Пистолет-пулемёты
        "mac10": Entry(display: "MAC-10", icon: .smg),
        "mp9": Entry(display: "MP9", icon: .smg),
        "mp7": Entry(display: "MP7", icon: .smg),
        "mp5sd": Entry(display: "MP5-SD", icon: .smg),
        "ump45": Entry(display: "UMP-45", icon: .smg),
        "p90": Entry(display: "P90", icon: .smg),
        "bizon": Entry(display: "PP-Bizon", icon: .smg),
        // Дробовики
        "nova": Entry(display: "Nova", icon: .shotgun),
        "xm1014": Entry(display: "XM1014", icon: .shotgun),
        "sawedoff": Entry(display: "Sawed-Off", icon: .shotgun),
        "mag7": Entry(display: "MAG-7", icon: .shotgun),
        // Автоматы и тяжёлое
        "m249": Entry(display: "M249", icon: .heavy),
        "negev": Entry(display: "Negev", icon: .heavy),
        // Штурмовые винтовки
        "ak47": Entry(display: "AK-47", icon: .rifle),
        "m4a1": Entry(display: "M4A4", icon: .rifle),
        "m4a1_silencer": Entry(display: "M4A1-S", icon: .rifle),
        "famas": Entry(display: "FAMAS", icon: .rifle),
        "galilar": Entry(display: "Galil AR", icon: .rifle),
        "sg556": Entry(display: "SG 553", icon: .rifle),
        "aug": Entry(display: "AUG", icon: .rifle),
        // Снайперские
        "awp": Entry(display: "AWP", icon: .sniper),
        "ssg08": Entry(display: "SSG 08", icon: .sniper),
        "scar20": Entry(display: "SCAR-20", icon: .sniper),
        "g3sg1": Entry(display: "G3SG1", icon: .sniper),
        // Гранаты
        "hegrenade": Entry(display: "HE-граната", icon: .grenade),
        "flashbang": Entry(display: "Флешбанг", icon: .grenade),
        "smokegrenade": Entry(display: "Дымовая граната", icon: .grenade),
        "molotov": Entry(display: "Молотов", icon: .grenade),
        "incgrenade": Entry(display: "Зажигательная", icon: .grenade),
        "decoy": Entry(display: "Муляж", icon: .grenade),
        // Холодное оружие и прочее
        "knife": Entry(display: "Нож", icon: .knife),
        "taser": Entry(display: "Электрошокер (Zeus x27)", icon: .pistol),
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
