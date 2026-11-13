import Foundation

/// Один раунд. Снимки идут каждые 4 тика; у каждого игрока массивы той же длины, что
/// `ticks` (кроме weapon/inventory — это списки изменений).
struct Round: Codable, Equatable {
    /// С 1, в порядке игры (овертайм продолжает счёт).
    var number: Int
    var startTick: Int
    /// `nil`, если раунд кончился раньше конца заморозки (например, сдача).
    var freezeEndTick: Int?
    var endTick: Int
    var winner: Team?
    /// Тики снимков: `startTick`, +4, +8, ... до `endTick`.
    var ticks: [Int]
    /// Сторона каждого игрока в этом раунде, индекс = `Player.index`.
    var teams: [Team?]
    /// Данные игроков, индекс = `Player.index`.
    var tracks: [PlayerTrack]
    var events: RoundEvents
    var bomb: Bomb
    /// Гранаты, брошенные в этом раунде (по `throwTick`).
    var grenades: [Grenade]
    /// Сводка каждого игрока за раунд, индекс = `Player.index`.
    var summary: [PlayerRound]
    /// Код причины конца раунда из игры (1 взрыв, 7 разминирование, 8 убиты все T,
    /// 9 убиты все CT, 18 сдача CT). Неизвестные коды передаются как есть.
    var reason: Int
}

/// Снимки одного игрока в одном раунде. Все массивы индексируются как `Round.ticks`.
struct PlayerTrack: Codable, Equatable {
    /// Позиция в десятых игровой единицы. У мёртвого сохраняется последняя известная позиция.
    var x: [Int] = []
    var y: [Int] = []
    var z: [Int] = []
    /// Десятые градуса: yaw -1800..1800, pitch -900..900.
    var yaw: [Int] = []
    var pitch: [Int] = []
    /// 0..100, 0 значит мёртв.
    var hp: [Int] = []
    /// Смены активного оружия.
    var weapon: [WeaponChange] = []
    /// Деньги на каждом снимке.
    var money: [Int] = []
    /// Смены инвентаря.
    var inventory: [InventoryChange] = []
    /// Потрачено за закупку (`cash_spent_this_round` на последнем снимке до конца заморозки).
    var spent: Int = 0
}

/// `[индекс снимка, id оружия]`. Id индексирует `MatchModel.weapons`.
struct WeaponChange: Codable, Equatable {
    var snapshotIndex: Int
    var weaponId: Int

    init(snapshotIndex: Int, weaponId: Int) {
        self.snapshotIndex = snapshotIndex
        self.weaponId = weaponId
    }

    init(from decoder: Decoder) throws {
        var c = try decoder.unkeyedContainer()
        snapshotIndex = try c.decode(Int.self)
        weaponId = try c.decode(Int.self)
    }

    func encode(to encoder: Encoder) throws {
        var c = encoder.unkeyedContainer()
        try c.encode(snapshotIndex)
        try c.encode(weaponId)
    }
}

/// `[индекс снимка, броня, флаги, [id предметов]]`. Флаги: бит 0 шлем, бит 1 набор сапёра.
/// Запись на первом снимке и при каждом изменении.
struct InventoryChange: Codable, Equatable {
    var snapshotIndex: Int
    var armor: Int
    var flags: Int
    var itemIds: [Int]

    static let helmetFlag = 1
    static let defuseKitFlag = 2

    init(snapshotIndex: Int, armor: Int, flags: Int, itemIds: [Int]) {
        self.snapshotIndex = snapshotIndex
        self.armor = armor
        self.flags = flags
        self.itemIds = itemIds
    }

    init(from decoder: Decoder) throws {
        var c = try decoder.unkeyedContainer()
        snapshotIndex = try c.decode(Int.self)
        armor = try c.decode(Int.self)
        flags = try c.decode(Int.self)
        itemIds = try c.decode([Int].self)
    }

    func encode(to encoder: Encoder) throws {
        var c = encoder.unkeyedContainer()
        try c.encode(snapshotIndex)
        try c.encode(armor)
        try c.encode(flags)
        try c.encode(itemIds)
    }
}

/// Сводка игрока за раунд: считается из `events`.
struct PlayerRound: Codable, Equatable {
    var kills: Int = 0
    var deaths: Int = 0
    var assists: Int = 0
    var damage: Int = 0
    var headshots: Int = 0
    var flashAssists: Int = 0
    /// Kill, Assist, Survived или Traded (союзник убил убийцу не позже 5 с после смерти).
    var kast: Bool = false
}
