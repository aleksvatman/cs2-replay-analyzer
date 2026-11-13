import Foundation

/// Короткие списки-кортежи одного раунда, первым всегда тик. Игрок это `Player.index`,
/// `-1` значит «никто» (урон от мира, нет ассистента). Названия оружия в событиях — это
/// внутренние имена игры (`ak47`), отдельные записи в `MatchModel.weapons` от инвентаря (`AK-47`).
struct RoundEvents: Codable, Equatable {
    var kills: [Kill] = []
    var damage: [Damage] = []
    var blinds: [Blind] = []
}

/// `[tick, killer, victim, assister, weapon, flags]`.
/// Флаги: 1 хедшот, 2 через дым, 4 через стену, 8 без прицела, 16 убийца ослеплён, 32 помощь флешкой.
struct Kill: Codable, Equatable {
    var tick: Int
    var killer: Int
    var victim: Int
    var assister: Int
    var weapon: Int
    var flags: Int

    static let headshot = 1
    static let throughSmoke = 2
    static let wallbang = 4
    static let noScope = 8
    static let attackerBlind = 16
    static let flashAssist = 32

    init(tick: Int, killer: Int, victim: Int, assister: Int, weapon: Int, flags: Int) {
        self.tick = tick
        self.killer = killer
        self.victim = victim
        self.assister = assister
        self.weapon = weapon
        self.flags = flags
    }

    init(from decoder: Decoder) throws {
        var c = try decoder.unkeyedContainer()
        tick = try c.decode(Int.self)
        killer = try c.decode(Int.self)
        victim = try c.decode(Int.self)
        assister = try c.decode(Int.self)
        weapon = try c.decode(Int.self)
        flags = try c.decode(Int.self)
    }

    func encode(to encoder: Encoder) throws {
        var c = encoder.unkeyedContainer()
        try c.encode(tick)
        try c.encode(killer)
        try c.encode(victim)
        try c.encode(assister)
        try c.encode(weapon)
        try c.encode(flags)
    }
}

/// `[tick, attacker, victim, weapon, healthDmg, armorDmg, victimHpAfter]`. `healthDmg` сырое
/// значение игры, может превышать остаток здоровья жертвы.
struct Damage: Codable, Equatable {
    var tick: Int
    var attacker: Int
    var victim: Int
    var weapon: Int
    var healthDmg: Int
    var armorDmg: Int
    var victimHpAfter: Int

    init(tick: Int, attacker: Int, victim: Int, weapon: Int, healthDmg: Int, armorDmg: Int, victimHpAfter: Int) {
        self.tick = tick
        self.attacker = attacker
        self.victim = victim
        self.weapon = weapon
        self.healthDmg = healthDmg
        self.armorDmg = armorDmg
        self.victimHpAfter = victimHpAfter
    }

    init(from decoder: Decoder) throws {
        var c = try decoder.unkeyedContainer()
        tick = try c.decode(Int.self)
        attacker = try c.decode(Int.self)
        victim = try c.decode(Int.self)
        weapon = try c.decode(Int.self)
        healthDmg = try c.decode(Int.self)
        armorDmg = try c.decode(Int.self)
        victimHpAfter = try c.decode(Int.self)
    }

    func encode(to encoder: Encoder) throws {
        var c = encoder.unkeyedContainer()
        try c.encode(tick)
        try c.encode(attacker)
        try c.encode(victim)
        try c.encode(weapon)
        try c.encode(healthDmg)
        try c.encode(armorDmg)
        try c.encode(victimHpAfter)
    }
}

/// `[tick, thrower, victim, durationMs]`.
struct Blind: Codable, Equatable {
    var tick: Int
    var thrower: Int
    var victim: Int
    var durationMs: Int

    init(tick: Int, thrower: Int, victim: Int, durationMs: Int) {
        self.tick = tick
        self.thrower = thrower
        self.victim = victim
        self.durationMs = durationMs
    }

    init(from decoder: Decoder) throws {
        var c = try decoder.unkeyedContainer()
        tick = try c.decode(Int.self)
        thrower = try c.decode(Int.self)
        victim = try c.decode(Int.self)
        durationMs = try c.decode(Int.self)
    }

    func encode(to encoder: Encoder) throws {
        var c = encoder.unkeyedContainer()
        try c.encode(tick)
        try c.encode(thrower)
        try c.encode(victim)
        try c.encode(durationMs)
    }
}
