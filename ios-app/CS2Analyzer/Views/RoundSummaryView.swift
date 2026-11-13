import SwiftUI

/// Сводка одного раунда: исход, `round.summary` и ссылка на повтор (`ReplayView`, этап I4).
struct RoundSummaryView: View {
    let match: MatchModel
    let round: Round

    var body: some View {
        List {
            Section("Раунд \(round.number)") {
                if let winner = round.winner {
                    LabeledContent("Победитель", value: winner.label)
                }
                LabeledContent("Причина", value: RoundReason.description(round.reason))
                LabeledContent("Тики", value: "\(round.startTick)...\(round.endTick)")
                if round.bomb.plantTick != nil {
                    LabeledContent("Бомба", value: bombOutcome)
                }
                NavigationLink("Повтор") {
                    ReplayView(match: match, round: round)
                }
            }

            Section("Игроки") {
                ForEach(playerRows, id: \.player.index) { row in
                    PlayerRoundRow(name: row.player.name, team: row.team, summary: row.summary)
                }
            }
        }
        .navigationTitle("Раунд \(round.number)")
    }

    private var bombOutcome: String {
        switch round.bomb.endType {
        case .exploded: return "взорвана"
        case .defused: return "разминирована"
        case nil: return "установлена"
        }
    }

    private struct Row {
        let player: Player
        let team: Team?
        let summary: PlayerRound
    }

    private var playerRows: [Row] {
        match.players.map { player in
            let team = round.teams.indices.contains(player.index) ? round.teams[player.index] : nil
            let summary = round.summary.indices.contains(player.index)
                ? round.summary[player.index]
                : PlayerRound()
            return Row(player: player, team: team, summary: summary)
        }
        .filter { $0.team != nil }
        .sorted { ($0.team?.label ?? "") < ($1.team?.label ?? "") }
    }
}

private struct PlayerRoundRow: View {
    let name: String
    let team: Team?
    let summary: PlayerRound

    var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text(name).font(.headline)
                Text("\(summary.kills)/\(summary.deaths)/\(summary.assists) · урон \(summary.damage)")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Spacer()
            if summary.kast {
                Text("KAST")
                    .font(.caption2.bold())
                    .padding(.horizontal, 6)
                    .padding(.vertical, 2)
                    .background(Color.green.opacity(0.25))
                    .clipShape(Capsule())
            }
        }
        .accessibilityElement(children: .combine)
    }
}
