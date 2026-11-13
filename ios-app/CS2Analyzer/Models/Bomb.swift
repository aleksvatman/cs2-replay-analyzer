import Foundation

enum BombEnd: String, Codable, Equatable {
    case exploded
    case defused
}

/// Бомба одного раунда. Поля установки/конца — `nil`, если этого не было.
struct Bomb: Codable, Equatable {
    var plantTick: Int?
    var x: Int?
    var y: Int?
    var z: Int?
    var planter: Int?
    var endTick: Int?
    var endType: BombEnd?
    var defuser: Int?
    /// Кто держит бомбу до установки: `[индекс снимка, игрок]`, игрок `-1` значит на земле.
    var carrier: [BombCarrier] = []
    /// Позиция бомбы на каждом снимке от начала раунда до установки (префикс `Round.ticks`).
    var trackX: [Int] = []
    var trackY: [Int] = []
    var trackZ: [Int] = []
    /// Выброс и подбор бомбы.
    var drops: [BombSpot] = []
    var pickups: [BombSpot] = []
}

/// `[индекс снимка, игрок]`.
struct BombCarrier: Codable, Equatable {
    var snapshotIndex: Int
    var player: Int

    init(snapshotIndex: Int, player: Int) {
        self.snapshotIndex = snapshotIndex
        self.player = player
    }

    init(from decoder: Decoder) throws {
        var c = try decoder.unkeyedContainer()
        snapshotIndex = try c.decode(Int.self)
        player = try c.decode(Int.self)
    }

    func encode(to encoder: Encoder) throws {
        var c = encoder.unkeyedContainer()
        try c.encode(snapshotIndex)
        try c.encode(player)
    }
}

/// `[tick, игрок, x, y, z]`: позиция игрока по ближайшему снимку.
struct BombSpot: Codable, Equatable {
    var tick: Int
    var player: Int
    var x: Int
    var y: Int
    var z: Int

    init(tick: Int, player: Int, x: Int, y: Int, z: Int) {
        self.tick = tick
        self.player = player
        self.x = x
        self.y = y
        self.z = z
    }

    init(from decoder: Decoder) throws {
        var c = try decoder.unkeyedContainer()
        tick = try c.decode(Int.self)
        player = try c.decode(Int.self)
        x = try c.decode(Int.self)
        y = try c.decode(Int.self)
        z = try c.decode(Int.self)
    }

    func encode(to encoder: Encoder) throws {
        var c = encoder.unkeyedContainer()
        try c.encode(tick)
        try c.encode(player)
        try c.encode(x)
        try c.encode(y)
        try c.encode(z)
    }
}
