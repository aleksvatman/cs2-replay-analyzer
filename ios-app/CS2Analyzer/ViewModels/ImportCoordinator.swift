import Foundation
import Observation

enum ImportState: Equatable {
    case idle
    case parsing(ParseProgress)
    case failed(String)
}

/// Состояние экрана импорта демки (этап I9): следит за прогрессом `MatchLibrary.importDemo`
/// и ошибками — сам импорт делает `MatchLibrary`, этот класс только отражает его на UI.
/// Ошибки ещё и пишутся в `ErrorLog` (этап I6, диагностика в `SettingsView`).
@Observable
@MainActor
final class ImportCoordinator {
    private(set) var state: ImportState = .idle
    private let errorLog: ErrorLog

    init(errorLog: ErrorLog = .shared) {
        self.errorLog = errorLog
    }

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
            let message = error.localizedDescription
            state = .failed(message)
            errorLog.append("Импорт демки: \(message)")
            return nil
        }
    }

    func reportPickerFailure(_ error: Error) {
        let message = error.localizedDescription
        state = .failed(message)
        errorLog.append("Выбор файла: \(message)")
    }

    func dismissError() {
        state = .idle
    }
}
