import Foundation

/// Статистика по игрокам за матч (этап 7a парсера).
struct Stats: Codable, Equatable {
    var perPlayer: [PlayerStats] = []
}

/// Итоги игрока `player` (индекс = `Player.index`) за матч. Не зависит от названий оружия.
struct PlayerStats: Codable, Equatable {
    var player: Int
    /// Раунды, где у игрока была сторона (ушедший игрок не считает раунды после ухода).
    var rounds: Int
    var kills: Int
    var deaths: Int
    var assists: Int
    /// Убийства своих — отдельно, не входят в `kills`.
    var teamKills: Int
    /// Урон врагам: по своим, себе и от мира не считается, режется по остатку здоровья жертвы.
    var damage: Int
    var headshots: Int
    /// Доля убийств в голову, процент с одним знаком (0 без убийств).
    var hsPercent: Double
    /// `damage / rounds`, один знак.
    var adr: Double
    /// Доля раундов с Kill, Assist, Survival или Trade, один знак.
    var kastPercent: Double
    /// Помощь флешкой: убийства с флагом `Kill.flashAssist`, только по врагам.
    var flashAssists: Int
    /// Сколько раз игрок ослепил врагов (свои и себя не считаются).
    var enemiesBlinded: Int
    /// Суммарное время ослепления врагов, миллисекунды.
    var enemyBlindMs: UInt64
}
