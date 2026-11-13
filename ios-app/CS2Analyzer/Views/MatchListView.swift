import SwiftUI

struct MatchListView: View {
    let items: [MatchSummary]
    let loadErrors: [String]
    @Binding var selection: String?

    var body: some View {
        List(selection: $selection) {
            if !loadErrors.isEmpty {
                Section("Не удалось загрузить") {
                    ForEach(loadErrors, id: \.self) { error in
                        Text(error)
                            .font(.caption)
                            .foregroundStyle(.red)
                    }
                }
            }
            ForEach(items) { item in
                MatchRow(item: item).tag(item.id)
            }
        }
        .navigationTitle("Матчи")
        .overlay {
            if items.isEmpty && loadErrors.isEmpty {
                ContentUnavailableView(
                    "Матчей пока нет",
                    systemImage: "tray",
                    description: Text("Локальный разбор демки на устройстве появится позже.")
                )
            }
        }
    }
}

private struct MatchRow: View {
    let item: MatchSummary

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(item.map)
                .font(.headline)
            Text("\(item.scoreT) : \(item.scoreCT) · \(item.roundsCount) раундов · \(item.playersCount) игроков")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .padding(.vertical, 2)
        .accessibilityElement(children: .combine)
    }
}

#Preview {
    NavigationStack {
        MatchListView(
            items: [
                MatchSummary(id: "1", map: "de_mirage", scoreT: 11, scoreCT: 13, roundsCount: 24, playersCount: 10),
            ],
            loadErrors: [],
            selection: .constant(nil)
        )
    }
}
