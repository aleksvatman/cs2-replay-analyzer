import Foundation

/// Часы повтора раунда (`ipad-doc.md` разд. 7): текущее положение — игровой тик, а не
/// индекс снимка. `advance(by:)` двигает тик по реальному времени и скорости воспроизведения;
/// логика отделена от `Timer`/`TimelineView`, чтобы её можно было проверить юнит-тестом.
@Observable
final class ReplayClock {
    enum Speed: Double, CaseIterable, Identifiable {
        case quarter = 0.25
        case half = 0.5
        case normal = 1
        case double = 2
        case triple = 3

        var id: Double { rawValue }

        var label: String {
            switch self {
            case .quarter: return "0.25×"
            case .half: return "0.5×"
            case .normal: return "1×"
            case .double: return "2×"
            case .triple: return "3×"
            }
        }
    }

    let startTick: Int
    let endTick: Int
    private let tickRate: Int

    /// Тик с дробной частью: `tickRate` — это ticks/с из демки (`MatchModel.tickRate`), и
    /// на 1× тик должен расти ровно на `tickRate` в секунду, то есть воспроизведение идёт
    /// в реальном времени раунда. Копить остаток нужно именно в `Double`: округление до
    /// `Int` на каждом кадре таймера (30 Гц) систематически отстаёт от реального тика —
    /// на коротких интервалах незаметно, но за весь раунд набегает заметное отставание,
    /// и 1× выглядит медленнее настоящего темпа игры.
    private var preciseTick: Double
    var isPlaying: Bool = false
    var speed: Speed = .normal

    var tick: Int { Int(preciseTick) }

    init(startTick: Int, endTick: Int, tickRate: Int) {
        self.startTick = startTick
        self.endTick = max(endTick, startTick)
        self.tickRate = max(tickRate, 1)
        self.preciseTick = Double(startTick)
    }

    /// Индекс снимка (шаг `step` тиков, в формате это 4) для текущего тика — не позже него.
    func snapshotIndex(step: Int = 4) -> Int {
        max(0, (tick - startTick) / step)
    }

    func seek(toTick newTick: Int) {
        preciseTick = Double(newTick.clamped(to: startTick...endTick))
        if tick >= endTick {
            isPlaying = false
        }
    }

    /// Продвигает тик на `deltaSeconds` реального времени, если сейчас воспроизведение идёт.
    /// Останавливается на `endTick`, не переходя его.
    func advance(by deltaSeconds: Double) {
        guard isPlaying, deltaSeconds > 0 else { return }
        let deltaTicks = deltaSeconds * Double(tickRate) * speed.rawValue
        let newTick = preciseTick + deltaTicks
        if newTick >= Double(endTick) {
            preciseTick = Double(endTick)
            isPlaying = false
        } else {
            preciseTick = max(Double(startTick), newTick)
        }
    }

    func togglePlay() {
        if tick >= endTick {
            preciseTick = Double(startTick)
        }
        isPlaying.toggle()
    }
}

private extension Int {
    func clamped(to range: ClosedRange<Int>) -> Int {
        Swift.min(Swift.max(self, range.lowerBound), range.upperBound)
    }
}
