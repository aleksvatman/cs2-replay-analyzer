import SwiftUI

struct MatchDetailView: View {
    let match: MatchModel

    var body: some View {
        NavigationStack {
            List {
                Section("Сводка") {
                    LabeledContent("Карта", value: match.map)
                    LabeledContent("Счёт", value: "\(match.score.t) : \(match.score.ct)")
                    LabeledContent("Раундов", value: "\(match.rounds.count)")
                    LabeledContent("Тиков в секунду", value: "\(match.tickRate)")
                }

                Section("Игроки") {
                    ForEach(match.stats.perPlayer.sorted(by: playerStatsOrder), id: \.player) { stats in
                        PlayerStatsRow(name: match.playerName(stats.player) ?? "#\(stats.player)", stats: stats)
                    }
                }

                Section("Раунды") {
                    ForEach(match.rounds, id: \.number) { round in
                        NavigationLink(value: round.number) {
                            RoundRow(round: round)
                        }
                    }
                }
            }
            .navigationTitle(match.map)
            .navigationDestination(for: Int.self) { number in
                if let round = match.rounds.first(where: { $0.number == number }) {
                    RoundSummaryView(match: match, round: round)
                }
            }
        }
        // Своя `NavigationStack` в detail-колонке: список раундов открывается без
        // замены содержимого sidebar (`NavigationSplitView`, этап I0).
        .id(match.map)
    }

    private func playerStatsOrder(_ lhs: PlayerStats, _ rhs: PlayerStats) -> Bool {
        if lhs.kills != rhs.kills { return lhs.kills > rhs.kills }
        return lhs.damage > rhs.damage
    }
}

private struct PlayerStatsRow: View {
    let name: String
    let stats: PlayerStats

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(name)
                .font(.headline)
            Text(
                "\(stats.kills)/\(stats.deaths)/\(stats.assists)"
                + " · ADR \(String(format: "%.1f", stats.adr))"
                + " · HS \(String(format: "%.0f", stats.hsPercent))%"
                + " · KAST \(String(format: "%.0f", stats.kastPercent))%"
            )
            .font(.caption)
            .foregroundStyle(.secondary)
        }
        .padding(.vertical, 2)
        .accessibilityElement(children: .combine)
    }
}

private struct RoundRow: View {
    let round: Round

    var body: some View {
        HStack {
            Text("Раунд \(round.number)")
            Spacer()
            if let winner = round.winner {
                Text(winner.label)
                    .font(.caption.bold())
                    .padding(.horizontal, 6)
                    .padding(.vertical, 2)
                    .background(winner == .t ? Color.orange.opacity(0.25) : Color.blue.opacity(0.25))
                    .clipShape(Capsule())
            }
            Text(RoundReason.description(round.reason))
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .accessibilityElement(children: .combine)
    }
}
