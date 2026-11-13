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
        case quad = 4

        var id: Double { rawValue }

        var label: String {
            switch self {
            case .quarter: return "0.25×"
            case .half: return "0.5×"
            case .normal: return "1×"
            case .double: return "2×"
            case .quad: return "4×"
            }
        }
    }

    let startTick: Int
    let endTick: Int
    private let tickRate: Int

    private(set) var tick: Int
    var isPlaying: Bool = false
    var speed: Speed = .normal

    init(startTick: Int, endTick: Int, tickRate: Int) {
        self.startTick = startTick
        self.endTick = max(endTick, startTick)
        self.tickRate = max(tickRate, 1)
        self.tick = startTick
    }

    /// Индекс снимка (шаг `step` тиков, в формате это 4) для текущего тика — не позже него.
    func snapshotIndex(step: Int = 4) -> Int {
        max(0, (tick - startTick) / step)
    }

    func seek(toTick newTick: Int) {
        tick = newTick.clamped(to: startTick...endTick)
        if tick >= endTick {
            isPlaying = false
        }
    }

    /// Продвигает тик на `deltaSeconds` реального времени, если сейчас воспроизведение идёт.
    /// Останавливается на `endTick`, не переходя его.
    func advance(by deltaSeconds: Double) {
        guard isPlaying, deltaSeconds > 0 else { return }
        let deltaTicks = deltaSeconds * Double(tickRate) * speed.rawValue
        let newTick = Double(tick) + deltaTicks
        if newTick >= Double(endTick) {
            tick = endTick
            isPlaying = false
        } else {
            tick = max(startTick, Int(newTick))
        }
    }

    func togglePlay() {
        if tick >= endTick {
            tick = startTick
        }
        isPlaying.toggle()
    }
}

private extension Int {
    func clamped(to range: ClosedRange<Int>) -> Int {
        Swift.min(Swift.max(self, range.lowerBound), range.upperBound)
    }
}
