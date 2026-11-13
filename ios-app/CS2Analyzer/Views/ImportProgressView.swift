import SwiftUI

/// Экран разбора демки на устройстве (этап I9): прогресс по стадиям или ошибка.
/// Честное предупреждение про отмену (`ipad-doc.md` разд. 10) — закрытие этого листа не
/// останавливает разбор, он всё равно досчитает до конца.
struct ImportProgressView: View {
    let state: ImportState
    let onDismissError: () -> Void

    var body: some View {
        VStack(spacing: 16) {
            switch state {
            case .idle:
                EmptyView()
            case .parsing(let progress):
                ProgressView(value: Double(progress.percent), total: 100) {
                    Text(stageLabel(progress.stage))
                }
                .accessibilityElement(children: .combine)
                .accessibilityLabel("\(stageLabel(progress.stage)), \(progress.percent)%")
                Text("Разбор уже начался и продолжится, даже если закрыть этот экран.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
            case .failed(let message):
                ContentUnavailableView {
                    Label("Не удалось разобрать демку", systemImage: "exclamationmark.triangle")
                } description: {
                    Text(message)
                }
                Button("Закрыть", action: onDismissError)
            }
        }
        .padding()
        .frame(minWidth: 320, minHeight: 200)
    }

    private func stageLabel(_ stage: ParseStage) -> String {
        switch stage {
        case .events: return "Заголовок и события…"
        case .players: return "Позиции игроков…"
        case .grenades: return "Гранаты…"
        case .assemble: return "Сборка результата…"
        case .done: return "Готово"
        }
    }
}

#Preview {
    ImportProgressView(state: .parsing(ParseProgress(stage: .players, percent: 42)), onDismissError: {})
}

#Preview {
    ImportProgressView(state: .failed("демо повреждена"), onDismissError: {})
}
