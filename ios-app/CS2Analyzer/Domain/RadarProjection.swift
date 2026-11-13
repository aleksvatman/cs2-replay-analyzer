import CoreGraphics

/// Перевод игровых координат в пиксели картинки радара (`ipad-doc.md` разд. 6):
/// `px = (x - posX) / scale`, `py = (posY - y) / scale`.
struct RadarProjection {
    let calibration: RadarCalibration

    /// `x`, `y` — координаты из JSON (десятые игровой единицы, как в `PlayerTrack`/`Grenade`).
    /// Результат — точка в пикселях картинки радара (`0...calibration.imageSize` по обеим осям
    /// при точке внутри карты; точка может выйти за пределы, калибровка этого не гарантирует).
    func point(x: Int, y: Int) -> CGPoint {
        let gameX = Double(x) / 10
        let gameY = Double(y) / 10
        let px = (gameX - calibration.posX) / calibration.scale
        let py = (calibration.posY - gameY) / calibration.scale
        return CGPoint(x: px, y: py)
    }
}
