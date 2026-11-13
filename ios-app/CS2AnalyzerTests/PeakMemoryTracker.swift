import Dispatch
import Foundation

/// Опрашивает резидентную память процесса в фоне, пока идёт долгая синхронная операция
/// (замеры I10) — `residentMemoryBytes()` сама по себе даёт только точку в моменте вызова,
/// а пиковое значение внутри `LocalParser.parse` (сам вызов блокирует поток) нужно ловить
/// отдельным потоком.
final class PeakMemoryTracker {
    private var timer: DispatchSourceTimer?
    private var peakBytes: UInt64 = 0
    private let queue = DispatchQueue(label: "peak-memory-tracker")

    func start(intervalMs: Int = 50) {
        peakBytes = residentMemoryBytes()
        let timer = DispatchSource.makeTimerSource(queue: queue)
        timer.schedule(deadline: .now(), repeating: .milliseconds(intervalMs))
        timer.setEventHandler { [weak self] in
            guard let self else { return }
            let current = residentMemoryBytes()
            if current > self.peakBytes { self.peakBytes = current }
        }
        timer.resume()
        self.timer = timer
    }

    @discardableResult
    func stop() -> UInt64 {
        timer?.cancel()
        timer = nil
        return peakBytes
    }
}
