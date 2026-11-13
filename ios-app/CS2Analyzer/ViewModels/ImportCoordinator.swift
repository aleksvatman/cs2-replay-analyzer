import Foundation
import Observation

enum ImportState: Equatable {
    case idle
    case parsing(ParseProgress)
    case failed(String)
}

/// Состояние экрана импорта демки (этап I9): следит за прогрессом `MatchLibrary.importDemo`
/// и ошибками — сам импорт делает `MatchLibrary`, этот класс только отражает его на UI.
@Observable
@MainActor
final class ImportCoordinator {
    private(set) var state: ImportState = .idle

    func importDemo(_ url: URL, into library: MatchLibrary) async -> MatchSummary? {
        state = .parsing(ParseProgress(stage: .events, percent: 0))
        do {
            let summary = try await library.importDemo(at: url) { [weak self] progress in
                Task { @MainActor in
                    self?.state = .parsing(progress)
                }
            }
            state = .idle
            return summary
        } catch {
            state = .failed(error.localizedDescription)
            return nil
        }
    }

    func reportPickerFailure(_ error: Error) {
        state = .failed(error.localizedDescription)
    }

    func dismissError() {
        state = .idle
    }
}
