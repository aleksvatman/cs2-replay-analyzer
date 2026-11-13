import SwiftUI

/// Повтор раунда (этап I4, `ipad-doc.md` разд. 7): воспроизведение по реальному времени
/// (`ReplayClock`) с управлением скоростью, игроками, гранатами (траектория + область
/// дыма/огня), бомбой, убийствами (линия + лента) и ослеплениями. Таймлайн — ползунок по
/// тикам раунда с отметками событий.
struct ReplayView: View {
    let match: MatchModel
    let round: Round

    @State private var clock: ReplayClock
    @State private var lastTickDate: Date?
    @State private var showLegend = false

    init(match: MatchModel, round: Round) {
        self.match = match
        self.round = round
        _clock = State(
            initialValue: ReplayClock(
                startTick: round.startTick, endTick: round.endTick, tickRate: match.tickRate
            )
        )
    }

    private var calibration: RadarCalibration? {
        RadarCalibrationStore.calibration(for: match.map)
    }

    var body: some View {
        Group {
            if let calibration {
                if let image = Self.loadImage(calibration) {
                    // Радар и управление занимают весь доступный экран; лента убийств —
                    // поверх радара (верхний правый угол), легенда — не здесь, а в боковой
                    // панели (`inspector`), скрытой по умолчанию.
                    VStack(spacing: 12) {
                        ZStack(alignment: .topTrailing) {
                            radar(calibration: calibration, image: image)
                            killFeed
                                .padding(10)
                                .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 10))
                                .padding(8)
                        }
                        .frame(maxHeight: .infinity)
                        controls
                    }
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
        .navigationTitle("Повтор — раунд \(round.number)")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button {
                    showLegend.toggle()
                } label: {
                    Image(systemName: "list.bullet.circle")
                }
                .accessibilityIdentifier("replay-legend-toggle")
                .accessibilityLabel("Обозначения")
            }
        }
        .inspector(isPresented: $showLegend) {
            legend
                .padding()
        }
        .onReceive(Timer.publish(every: 1.0 / 30.0, on: .main, in: .common).autoconnect()) { date in
            if let lastTickDate {
                clock.advance(by: date.timeIntervalSince(lastTickDate))
            }
            lastTickDate = date
        }
    }

    // MARK: - Радар и Canvas

    @ViewBuilder
    private func radar(calibration: RadarCalibration, image: UIImage) -> some View {
        GeometryReader { geometry in
            let side = min(geometry.size.width, geometry.size.height)
            ZStack {
                Image(uiImage: image)
                    .resizable()
                    .frame(width: side, height: side)
                Canvas { context, _ in
                    draw(context: context, calibration: calibration, side: side)
                }
                .frame(width: side, height: side)
                .accessibilityLabel(radarAccessibilityLabel)
            }
            .frame(width: geometry.size.width, height: geometry.size.height, alignment: .top)
        }
        .aspectRatio(1, contentMode: .fit)
    }

    /// Canvas непрозрачен для VoiceOver — описываем текущее состояние словами вместо точек.
    private var radarAccessibilityLabel: String {
        let interpolation = SnapshotInterpolation(tick: clock.tick, startTick: round.startTick, step: 4)
        let hpIndex = min(interpolation.index0, max(round.ticks.count - 1, 0))
        var aliveT = 0, aliveCT = 0
        for player in match.players {
            guard round.teams.indices.contains(player.index), let team = round.teams[player.index],
                  round.tracks.indices.contains(player.index) else { continue }
            let track = round.tracks[player.index]
            guard track.hp.indices.contains(hpIndex), track.hp[hpIndex] > 0,
                  !hasDiedBy(playerIndex: player.index, tick: clock.tick) else { continue }
            if team == .t { aliveT += 1 } else { aliveCT += 1 }
        }
        return "Радар, тик \(clock.tick) из \(round.endTick). Живых T: \(aliveT), живых CT: \(aliveCT)."
    }

    private func draw(context: GraphicsContext, calibration: RadarCalibration, side: CGFloat) {
        let projection = RadarProjection(calibration: calibration)
        let pixelsPerUnit = side / CGFloat(calibration.imageSize)
        let tick = clock.tick
        let interpolation = SnapshotInterpolation(tick: tick, startTick: round.startTick, step: 4)

        func pixel(x: Int, y: Int) -> CGPoint {
            let raw = projection.point(x: x, y: y)
            return CGPoint(x: raw.x * pixelsPerUnit, y: raw.y * pixelsPerUnit)
        }

        drawGrenades(context: context, tick: tick, pixel: pixel)
        drawBomb(context: context, tick: tick, interpolation: interpolation, pixel: pixel)
        drawPlayers(context: context, tick: tick, interpolation: interpolation, pixel: pixel)
        drawRecentKills(context: context, tick: tick, pixel: pixel)
    }

    /// Снимки идут раз в 4 тика (`ipad-doc.md` разд. 7, 16 кадров в секунду); на глаз это
    /// заметно дёргается даже на 1x, поэтому позиции и углы (`PlayerTrack`, `Bomb.trackX/Y/Z`)
    /// интерполируются между соседними снимками по доле тика между ними.
    private struct SnapshotInterpolation {
        let index0: Int
        let index1: Int
        let fraction: Double

        init(tick: Int, startTick: Int, step: Int) {
            let offset = max(0, tick - startTick)
            index0 = offset / step
            index1 = index0 + 1
            fraction = Double(offset % step) / Double(step)
        }

        func point(x: [Int], y: [Int]) -> CGPoint? {
            guard x.indices.contains(index0) else { return nil }
            let i1 = min(index1, x.count - 1)
            let x0 = Double(x[index0]), y0 = Double(y[index0])
            let x1 = Double(x[i1]), y1 = Double(y[i1])
            return CGPoint(
                x: x0 + (x1 - x0) * fraction,
                y: y0 + (y1 - y0) * fraction
            )
        }

        /// Угол через 360° по кратчайшему пути (десятые градуса на входе).
        func angleRadians(_ values: [Int]) -> Double? {
            guard values.indices.contains(index0) else { return nil }
            let i1 = min(index1, values.count - 1)
            let a0 = Double(values[index0]) / 10
            let a1 = Double(values[i1]) / 10
            var delta = a1 - a0
            delta -= (360 * (delta / 360).rounded(.toNearestOrAwayFromZero))
            return (a0 + delta * fraction) * .pi / 180
        }
    }

    private func drawPlayers(
        context: GraphicsContext, tick: Int, interpolation: SnapshotInterpolation, pixel: (Int, Int) -> CGPoint
    ) {
        var context = context
        guard round.teams.count == match.players.count, round.tracks.count == match.players.count else {
            return
        }
        let hpIndex = min(interpolation.index0, max(round.ticks.count - 1, 0))

        for player in match.players {
            guard let team = round.teams[player.index] else { continue }
            let track = round.tracks[player.index]
            guard let gamePoint = interpolation.point(x: track.x, y: track.y),
                  track.hp.indices.contains(hpIndex) else { continue }

            let point = pixel(Int(gamePoint.x.rounded()), Int(gamePoint.y.rounded()))
            // Последнее убийство раунда приходит на тик `round_end` и в `hp` не видно —
            // смерть определяем и по `events.kills`, а не только по `hp == 0`.
            let alive = track.hp[hpIndex] > 0 && !hasDiedBy(playerIndex: player.index, tick: tick)
            let blinded = isBlinded(playerIndex: player.index, tick: tick)
            let color: Color = team == .t ? .orange : .blue

            let dotRadius: CGFloat = 6
            let dotRect = CGRect(
                x: point.x - dotRadius, y: point.y - dotRadius,
                width: dotRadius * 2, height: dotRadius * 2
            )
            context.opacity = alive ? 1 : 0.35
            context.fill(Path(ellipseIn: dotRect), with: .color(color))
            context.stroke(Path(ellipseIn: dotRect), with: .color(blinded ? .yellow : .white), lineWidth: blinded ? 3 : 1)

            if alive, let yawRadians = interpolation.angleRadians(track.yaw) {
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

    /// Гранаты: траектория полёта до текущего тика и, у дыма/огня, область эффекта.
    private func drawGrenades(context: GraphicsContext, tick: Int, pixel: (Int, Int) -> CGPoint) {
        var context = context
        for grenade in round.grenades {
            // Область эффекта (дым/огонь): рисуется, пока активна, независимо от того,
            // долетел ли снаряд (сам снаряд уже закончил полёт к этому моменту).
            if let area = grenade.area, tick >= area.startTick, tick <= (area.endTick ?? round.endTick) {
                let center = pixel(area.x, area.y)
                let radius: CGFloat = grenade.type == .smoke ? 36 : 22
                let color: Color = grenade.type == .smoke ? .gray : .red
                let rect = CGRect(x: center.x - radius, y: center.y - radius, width: radius * 2, height: radius * 2)
                context.opacity = 0.35
                context.fill(Path(ellipseIn: rect), with: .color(color))
                context.opacity = 1
            }

            guard tick >= grenade.throwTick, tick <= grenade.endTick, !grenade.ticks.isEmpty else { continue }

            var trail = Path()
            var started = false
            var lastPoint: CGPoint?
            for i in grenade.ticks.indices {
                guard grenade.ticks[i] <= tick else { break }
                let point = pixel(grenade.x[i], grenade.y[i])
                if !started {
                    trail.move(to: point)
                    started = true
                } else {
                    trail.addLine(to: point)
                }
                lastPoint = point
            }
            guard started else { continue }

            context.stroke(trail, with: .color(grenadeColor(grenade.type)), lineWidth: 1.5)
            if let lastPoint {
                let radius: CGFloat = 4
                let rect = CGRect(x: lastPoint.x - radius, y: lastPoint.y - radius, width: radius * 2, height: radius * 2)
                context.fill(Path(ellipseIn: rect), with: .color(grenadeColor(grenade.type)))
            }
        }
    }

    private func grenadeColor(_ kind: GrenadeKind) -> Color {
        switch kind {
        case .smoke: return .gray
        case .he: return .red
        case .flash: return .yellow
        case .fire: return .orange
        case .decoy: return .purple
        }
    }

    /// Бомба: до установки следует позицию несущего игрока по `trackX/Y/Z` (шаг снимков
    /// раунда), после установки — неподвижна в точке `bomb.x/y`.
    private func drawBomb(
        context: GraphicsContext, tick: Int, interpolation: SnapshotInterpolation, pixel: (Int, Int) -> CGPoint
    ) {
        var context = context
        let bomb = round.bomb
        var point: CGPoint?

        if let plantTick = bomb.plantTick, tick >= plantTick, let x = bomb.x, let y = bomb.y {
            point = pixel(x, y)
        } else if let gamePoint = interpolation.point(x: bomb.trackX, y: bomb.trackY) {
            point = pixel(Int(gamePoint.x.rounded()), Int(gamePoint.y.rounded()))
        }

        guard let point else { return }
        let size: CGFloat = 8
        let rect = CGRect(x: point.x - size / 2, y: point.y - size / 2, width: size, height: size)
        context.fill(Path(rect), with: .color(.red))
        context.stroke(Path(rect), with: .color(.white), lineWidth: 1)
    }

    /// Недавние убийства (последние ~2 секунды игрового времени): линия убийца-жертва по их
    /// позиции на тик убийства.
    private func drawRecentKills(context: GraphicsContext, tick: Int, pixel: (Int, Int) -> CGPoint) {
        var context = context
        let fadeWindow = max(match.tickRate * 2, 1)
        for kill in round.events.kills where kill.tick <= tick && tick - kill.tick <= fadeWindow {
            guard let killerPoint = position(playerIndex: kill.killer, atTick: kill.tick, pixel: pixel),
                  let victimPoint = position(playerIndex: kill.victim, atTick: kill.tick, pixel: pixel) else {
                continue
            }
            let progress = Double(tick - kill.tick) / Double(fadeWindow)
            var line = Path()
            line.move(to: killerPoint)
            line.addLine(to: victimPoint)
            context.opacity = 1 - progress
            context.stroke(line, with: .color(.red), style: StrokeStyle(lineWidth: 2, dash: [4, 3]))
            context.opacity = 1
        }
    }

    private func position(playerIndex: Int, atTick tick: Int, pixel: (Int, Int) -> CGPoint) -> CGPoint? {
        guard round.tracks.indices.contains(playerIndex) else { return nil }
        let track = round.tracks[playerIndex]
        let index = min(max(0, (tick - round.startTick) / 4), max(track.x.count - 1, 0))
        guard track.x.indices.contains(index) else { return nil }
        return pixel(track.x[index], track.y[index])
    }

    private func hasDiedBy(playerIndex: Int, tick: Int) -> Bool {
        round.events.kills.contains { $0.victim == playerIndex && $0.tick <= tick }
    }

    private func isBlinded(playerIndex: Int, tick: Int) -> Bool {
        round.events.blinds.contains { blind in
            guard blind.victim == playerIndex else { return false }
            let endTick = blind.tick + (blind.durationMs * match.tickRate) / 1000
            return tick >= blind.tick && tick <= endTick
        }
    }

    // MARK: - Управление и таймлайн

    private var controls: some View {
        VStack(spacing: 8) {
            timeline
            HStack {
                Button {
                    clock.togglePlay()
                } label: {
                    Image(systemName: clock.isPlaying ? "pause.fill" : "play.fill")
                }
                .buttonStyle(.bordered)
                .accessibilityIdentifier("replay-play-pause")
                .accessibilityLabel(clock.isPlaying ? "Пауза" : "Воспроизвести")

                Picker("Скорость", selection: Binding(get: { clock.speed }, set: { clock.speed = $0 })) {
                    ForEach(ReplayClock.Speed.allCases) { speed in
                        Text(speed.label).tag(speed)
                    }
                }
                .pickerStyle(.segmented)

                Spacer()
                Text("Тик \(clock.tick) / \(round.endTick)")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .monospacedDigit()
            }
        }
    }

    private var timeline: some View {
        let range = Double(round.startTick)...Double(max(round.endTick, round.startTick + 1))
        return ZStack(alignment: .leading) {
            GeometryReader { geometry in
                ForEach(Array(eventMarks.enumerated()), id: \.offset) { _, mark in
                    let fraction = (Double(mark.tick) - range.lowerBound) / (range.upperBound - range.lowerBound)
                    Circle()
                        .fill(mark.color)
                        .frame(width: 6, height: 6)
                        .position(x: geometry.size.width * CGFloat(fraction), y: 4)
                }
            }
            .frame(height: 10)
            .offset(y: -14)
            // Декоративные отметки поверх слайдера — сам слайдер даёт VoiceOver позицию.
            .accessibilityHidden(true)

            Slider(
                value: Binding(
                    get: { Double(clock.tick) },
                    set: { clock.seek(toTick: Int($0.rounded())) }
                ),
                in: range
            )
        }
    }

    private struct EventMark {
        let tick: Int
        let color: Color
    }

    private var eventMarks: [EventMark] {
        var marks = round.events.kills.map { EventMark(tick: $0.tick, color: .red) }
        if let freezeEndTick = round.freezeEndTick {
            marks.append(EventMark(tick: freezeEndTick, color: .green))
        }
        if let plantTick = round.bomb.plantTick {
            marks.append(EventMark(tick: plantTick, color: .yellow))
        }
        if let defuseEndTick = round.bomb.endTick, round.bomb.endType == .defused {
            marks.append(EventMark(tick: defuseEndTick, color: .blue))
        }
        return marks
    }

    // MARK: - Лента убийств

    /// Поверх радара (верхний правый угол, `ZStack` в `body`) — не занимает отдельного
    /// места в раскладке.
    private var killFeed: some View {
        let recent = round.events.kills
            .filter { $0.tick <= clock.tick }
            .suffix(5)
            .reversed()

        return VStack(alignment: .trailing, spacing: 4) {
            ForEach(Array(recent.enumerated()), id: \.offset) { _, kill in
                HStack(spacing: 4) {
                    Text(playerName(kill.killer))
                        .foregroundStyle(teamColor(for: kill.killer))
                    Image(systemName: WeaponNames.icon(kill.weapon(in: match)).rawValue)
                        .font(.caption2)
                        .accessibilityHidden(true)
                    Text("[\(WeaponNames.displayName(kill.weapon(in: match)))]")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                    if kill.flags & Kill.headshot != 0 {
                        Image(systemName: "staroflife.fill")
                            .font(.caption2)
                            .accessibilityHidden(true)
                    }
                    Text(playerName(kill.victim))
                        .foregroundStyle(teamColor(for: kill.victim))
                }
                .font(.caption)
                .accessibilityElement(children: .combine)
                .accessibilityLabel(killAccessibilityLabel(kill))
            }
        }
    }

    private func killAccessibilityLabel(_ kill: Kill) -> String {
        let weapon = WeaponNames.displayName(kill.weapon(in: match))
        let headshot = kill.flags & Kill.headshot != 0 ? ", в голову" : ""
        return "\(playerName(kill.killer)) убил \(playerName(kill.victim)) из \(weapon)\(headshot)"
    }

    /// Цвет команды игрока в этом раунде (как у точки на радаре); `nil` (мир, нет данных
    /// о стороне) оставляет цвет по умолчанию.
    private func teamColor(for playerIndex: Int) -> Color {
        guard round.teams.indices.contains(playerIndex), let team = round.teams[playerIndex] else {
            return .primary
        }
        return team == .t ? .orange : .blue
    }

    // MARK: - Легенда

    private struct LegendRow: Identifiable {
        let id = UUID()
        let swatch: AnyView
        let text: String
    }

    private func legendDot(_ color: Color) -> AnyView {
        AnyView(Circle().fill(color).frame(width: 12, height: 12).accessibilityHidden(true))
    }

    private func legendIcon(_ name: String) -> AnyView {
        AnyView(Image(systemName: name).frame(width: 12, height: 12).accessibilityHidden(true))
    }

    private func legendLine(_ color: Color, dashed: Bool = false) -> AnyView {
        AnyView(
            Rectangle()
                .fill(color)
                .frame(width: 16, height: dashed ? 1 : 2)
                .accessibilityHidden(true)
        )
    }

    private var playerLegendRows: [LegendRow] {
        [
            LegendRow(swatch: legendDot(.orange), text: "Игрок, T"),
            LegendRow(swatch: legendDot(.blue), text: "Игрок, CT"),
            LegendRow(swatch: legendIcon("circle.dashed"), text: "Тусклая точка — мёртв"),
            LegendRow(swatch: legendLine(.orange), text: "Линия от точки — направление взгляда"),
            LegendRow(swatch: legendIcon("circle.circle"), text: "Жёлтое кольцо — ослеплён"),
            LegendRow(swatch: legendLine(.red, dashed: true), text: "Линия — убийство (затухает)"),
            LegendRow(swatch: legendIcon(WeaponNames.Icon.gun.rawValue), text: "Убийство из оружия"),
            LegendRow(swatch: legendIcon("staroflife.fill"), text: "Хедшот"),
        ]
    }

    private var grenadeLegendRows: [LegendRow] {
        [
            LegendRow(swatch: legendDot(.red), text: "Бомба (квадрат)"),
            LegendRow(swatch: legendDot(.gray), text: "Дымовая граната / область дыма"),
            LegendRow(swatch: legendDot(.red), text: "HE-граната"),
            LegendRow(swatch: legendDot(.yellow), text: "Флешбанг"),
            LegendRow(swatch: legendDot(.orange), text: "Молотов/зажигательная, область огня"),
            LegendRow(swatch: legendDot(.purple), text: "Муляж (decoy)"),
        ]
    }

    private var timelineLegendRows: [LegendRow] {
        [
            LegendRow(swatch: legendDot(.red), text: "Убийство"),
            LegendRow(swatch: legendDot(.green), text: "Конец заморозки"),
            LegendRow(swatch: legendDot(.yellow), text: "Установка бомбы"),
            LegendRow(swatch: legendDot(.blue), text: "Разминирование"),
        ]
    }

    private var legend: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 8) {
                Text("Обозначения").font(.title3.bold())
                ForEach(playerLegendRows) { row in
                    HStack(spacing: 8) {
                        row.swatch
                        Text(row.text).font(.caption)
                    }
                }
                Divider()
                Text("Гранаты и бомба").font(.subheadline.bold())
                ForEach(grenadeLegendRows) { row in
                    HStack(spacing: 8) {
                        row.swatch
                        Text(row.text).font(.caption)
                    }
                }
                Divider()
                Text("Отметки на таймлайне").font(.subheadline.bold())
                ForEach(timelineLegendRows) { row in
                    HStack(spacing: 8) {
                        row.swatch
                        Text(row.text).font(.caption)
                    }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private func playerName(_ index: Int) -> String {
        guard index >= 0, let player = match.players.first(where: { $0.index == index }) else {
            return "мир"
        }
        return player.name
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

private extension Kill {
    /// `weapon` в событиях это индекс в `MatchModel.weapons`, значение там — внутреннее
    /// имя игры (`ak47`), подходящее для `WeaponNames`.
    func weapon(in match: MatchModel) -> String {
        match.weapons.indices.contains(weapon) ? match.weapons[weapon] : ""
    }
}
