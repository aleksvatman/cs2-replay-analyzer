import XCTest

/// UI-тест для визуальной проверки: список → раунд → повтор, со скриншотом
/// каждого экрана (виден в отчёте `xcodebuild test`/Xcode). Это не проверка данных
/// (то делает `CS2AnalyzerTests`) — только что экран открывается и не падает.
/// Только на симуляторе: `CS2AnalyzerUITests` на реальном устройстве ставит отдельный
/// `-Runner.app`, который остаётся лишней иконкой на рабочем столе.
final class ReplayUITests: XCTestCase {
    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    func testOpenMirageRoundReplay() throws {
        let app = XCUIApplication()
        app.launch()
        attach(app, name: "01-match-list")

        tapRow(app, startingWith: "de_mirage")
        attach(app, name: "02-match-detail")

        // Раздел "Раунды" идёт после сводки и списка игроков — на части экранов строка
        // раунда оказывается ниже видимой области, на других видна сразу; прокручиваем
        // только если понадобится (`tapRow` сам решает).
        tapRow(app, startingWith: "Раунд 1,", scrollableDetailPane: true)
        attach(app, name: "03-round-summary")

        tapRow(app, startingWith: "Повтор")
        waitForRadarScreen(app)
        attach(app, name: "04-replay")

        // Кнопка воспроизведения появляется только если есть и калибровка, и картинка
        // радара (иначе показан `ContentUnavailableView` без элементов управления).
        let playButton = app.buttons["replay-play-pause"]
        if playButton.waitForExistence(timeout: 2) {
            playButton.tap()
            RunLoop.current.run(until: Date().addingTimeInterval(0.5))
            attach(app, name: "05-replay-playing")
        }
    }

    func testOpenAncientRoundReplay() throws {
        let app = XCUIApplication()
        app.launch()

        tapRow(app, startingWith: "de_ancient")
        tapRow(app, startingWith: "Раунд 1,", scrollableDetailPane: true)
        tapRow(app, startingWith: "Повтор")
        waitForRadarScreen(app)
        attach(app, name: "ancient-radar")
    }

    /// Либо сама картинка радара, либо явное предупреждение об её отсутствии — падать
    /// в любом случае не должно (см. `Resources/Maps/README.md`).
    private func waitForRadarScreen(_ app: XCUIApplication) {
        let radarTitle = app.navigationBars["Повтор — раунд 1"]
        let missingImageWarning = app.staticTexts["Нет картинки радара"]
        let missingCalibrationWarning = app.staticTexts["Нет калибровки радара"]
        XCTAssertTrue(
            radarTitle.waitForExistence(timeout: 5)
            || missingImageWarning.waitForExistence(timeout: 1)
            || missingCalibrationWarning.waitForExistence(timeout: 1)
        )
    }

    /// Строка списка/раунда — это `Button` (`NavigationLink`) с меткой, которая
    /// объединяет все дочерние тексты ("Раунд 1, T, Уничтожены все CT"); точное
    /// совпадение не подходит, поэтому ищем по началу строки. `List` в SwiftUI иногда
    /// держит в дереве доступности лишнюю (невидимую) копию строки — берём первый
    /// элемент, который действительно `isHittable`, и при необходимости докручиваем
    /// (`scrollableDetailPane`) жестом по экрану — он не завязан на конкретный размер
    /// iPad, в отличие от свайпа от фиксированных координат окна.
    @discardableResult
    private func tapRow(
        _ app: XCUIApplication, startingWith prefix: String,
        scrollableDetailPane: Bool = false, timeout: TimeInterval = 8
    ) -> Bool {
        let predicate = NSPredicate(format: "label BEGINSWITH %@", prefix)
        let deadline = Date().addingTimeInterval(timeout)

        repeat {
            if let button = firstHittable(app.buttons.matching(predicate)) {
                button.tap()
                return true
            }
            if let text = firstHittable(app.staticTexts.matching(predicate)) {
                text.tap()
                return true
            }
            if scrollableDetailPane {
                // Свайп через `.swipeUp()` конкретного элемента (не ручной
                // `press(forDuration:thenDragTo:)` по координатам — тот не долистывал
                // `List`, похоже, не попадая в его жестовый распознаватель). Берём первую
                // ячейку заново на каждой попытке — она всегда есть, в отличие от
                // конкретной подписи, которая после прокрутки уйдёт с экрана.
                // `app.cells` видит ячейки и сайдбара, и detail-колонки вперемешку;
                // detail-колонка заметно шире (сайдбар у iPad в раскрытом виде уже),
                // так отличаем одну от другой без привязки к конкретному макету.
                let cells = app.cells
                var detailCell: XCUIElement?
                for i in 0..<cells.count {
                    let cell = cells.element(boundBy: i)
                    if cell.exists, cell.frame.width > 500 {
                        detailCell = cell
                        break
                    }
                }
                detailCell?.swipeUp()
            }
            RunLoop.current.run(until: Date().addingTimeInterval(0.2))
        } while Date() < deadline

        XCTFail("Не нашёл кликабельный элемент, начинающийся с «\(prefix)»")
        return false
    }

    private func firstHittable(_ query: XCUIElementQuery) -> XCUIElement? {
        for i in 0..<query.count {
            let element = query.element(boundBy: i)
            if element.exists, element.isHittable {
                return element
            }
        }
        return nil
    }

    private func attach(_ app: XCUIApplication, name: String) {
        let screenshot = app.screenshot()
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
