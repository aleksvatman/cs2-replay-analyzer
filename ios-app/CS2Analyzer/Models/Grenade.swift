import Foundation

enum GrenadeKind: String, Codable, Equatable {
    case smoke
    case he
    case flash
    /// Молотов и зажигательная граната — один тип, рисуются одинаково.
    case fire
    case decoy
}

/// Область, которую оставляет дым или огонь.
struct Area: Codable, Equatable {
    var startTick: Int
    /// `nil`, если эффект не закончился до конца раунда (демка не пишет события конца).
    var endTick: Int?
    var x: Int
    var y: Int
    var z: Int
}

/// Одна брошенная граната. Траектория содержит точку на каждом тике от `throwTick` до
/// `endTick`, поэтому отрезками между точками можно нарисовать и излом после удара о стену.
/// `ticks` не совпадает по индексам с `Round.ticks` (там шаг 4).
struct Grenade: Codable, Equatable {
    var type: GrenadeKind
    /// `Player.index` бросившего.
    var thrower: Int
    var throwTick: Int
    /// Конец полёта: взрыв HE, вспышка, раскрытие дыма, начало огня.
    var endTick: Int
    var ticks: [Int] = []
    var x: [Int] = []
    var y: [Int] = []
    var z: [Int] = []
    /// Только у дыма и огня.
    var area: Area?
}
