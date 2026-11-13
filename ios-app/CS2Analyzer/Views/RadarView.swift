import SwiftUI

/// Статичная карта с точками игроков одного снимка (этап I3). Сам повтор
/// (анимация, таймлайн, гранаты, бомба) — этапы I4+.
struct RadarView: View {
    let match: MatchModel
    let round: Round

    @State private var snapshotIndex: Int = 0

    private var calibration: RadarCalibration? {
        RadarCalibrationStore.calibration(for: match.map)
    }

    var body: some View {
        VStack(spacing: 12) {
            if let calibration {
                if let image = Self.loadImage(calibration) {
                    radar(calibration: calibration, image: image)
                } else {
                    ContentUnavailableView(
                        "Нет картинки радара",
                        systemImage: "photo",
                        description: Text(
                            "Калибровка для \(match.map) есть, картинки \(calibration.image) нет — "
                            + "см. Resources/Maps/README.md."
                        )
                    )
                }
            } else {
                ContentUnavailableView(
                    "Нет калибровки радара",
                    systemImage: "map",
                    description: Text("Для карты \(match.map) пока нет файла калибровки.")
                )
            }
        }
        .padding()
        .navigationTitle("Радар — раунд \(round.number)")
    }

    @ViewBuilder
    private func radar(calibration: RadarCalibration, image: UIImage) -> some View {
        let maxSnapshot = max(round.ticks.count - 1, 0)

        GeometryReader { geometry in
            let side = min(geometry.size.width, geometry.size.height)
            ZStack {
                Image(uiImage: image)
                    .resizable()
                    .frame(width: side, height: side)
                Canvas { context, _ in
                    drawPlayers(context: context, calibration: calibration, side: side)
                }
                .frame(width: side, height: side)
            }
            .frame(width: geometry.size.width, height: geometry.size.height, alignment: .top)
        }
        .aspectRatio(1, contentMode: .fit)

        if maxSnapshot > 0 {
            Slider(
                value: Binding(
                    get: { Double(snapshotIndex) },
                    set: { snapshotIndex = Int($0.rounded()) }
                ),
                in: 0...Double(maxSnapshot),
                step: 1
            )
            Text("Тик \(round.ticks[min(snapshotIndex, maxSnapshot)])")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
    }

    private func drawPlayers(context: GraphicsContext, calibration: RadarCalibration, side: CGFloat) {
        var context = context
        guard round.teams.count == match.players.count, round.tracks.count == match.players.count else {
            return
        }
        let index = min(snapshotIndex, max(round.ticks.count - 1, 0))
        let projection = RadarProjection(calibration: calibration)
        let pixelsPerUnit = side / CGFloat(calibration.imageSize)

        for player in match.players {
            guard let team = round.teams[player.index] else { continue }
            let track = round.tracks[player.index]
            guard track.x.indices.contains(index) else { continue }

            let raw = projection.point(x: track.x[index], y: track.y[index])
            let point = CGPoint(x: raw.x * pixelsPerUnit, y: raw.y * pixelsPerUnit)
            let alive = track.hp[index] > 0
            let color: Color = team == .t ? .orange : .blue

            let dotRadius: CGFloat = 6
            let dotRect = CGRect(
                x: point.x - dotRadius, y: point.y - dotRadius,
                width: dotRadius * 2, height: dotRadius * 2
            )
            context.opacity = alive ? 1 : 0.35
            context.fill(Path(ellipseIn: dotRect), with: .color(color))
            context.stroke(Path(ellipseIn: dotRect), with: .color(.white), lineWidth: 1)

            // Направление взгляда (yaw, десятые градуса; 0 — вдоль +X игры).
            if alive, track.yaw.indices.contains(index) {
                let yawDegrees = Double(track.yaw[index]) / 10
                let yawRadians = yawDegrees * .pi / 180
                let lineLength: CGFloat = 14
                let end = CGPoint(
                    x: point.x + lineLength * CGFloat(cos(yawRadians)),
                    y: point.y - lineLength * CGFloat(sin(yawRadians))
                )
                var line = Path()
                line.move(to: point)
                line.addLine(to: end)
                context.stroke(line, with: .color(color), lineWidth: 2)
            }

            context.opacity = 1
        }
    }

    private static func loadImage(_ calibration: RadarCalibration) -> UIImage? {
        let name = (calibration.image as NSString).deletingPathExtension
        let ext = (calibration.image as NSString).pathExtension
        guard let path = Bundle.main.path(forResource: name, ofType: ext.isEmpty ? "png" : ext) else {
            return nil
        }
        return UIImage(contentsOfFile: path)
    }
}
