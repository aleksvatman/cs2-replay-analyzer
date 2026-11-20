import Foundation
import IosBridge

/// Стадии разбора, как в `parser_core::Stage`/`Cs2Stage` (`ios-bridge/src/lib.rs`).
enum ParseStage: UInt32, Equatable {
    case events = 0
    case players = 1
    case grenades = 2
    case assemble = 3
    case done = 4
}

struct ParseProgress: Equatable {
    let stage: ParseStage
    let percent: Int
}

enum LocalParserError: Error, LocalizedError {
    case invalidArgument(String)
    case io(String)
    case parseFailed(String)
    case internalError(String)

    var errorDescription: String? {
        switch self {
        case .invalidArgument(let message): return "Неверный путь к файлу: \(message)"
        case .io(let message): return "Ошибка чтения или записи: \(message)"
        case .parseFailed(let message): return "Не удалось разобрать демку: \(message)"
        case .internalError(let message): return "Внутренняя ошибка моста: \(message)"
        }
    }
}

/// Тонкая обёртка над C-интерфейсом `ios-bridge` (этап I8, `ios-bridge/src/lib.rs`).
///
/// `parse` синхронный и тяжёлый по CPU — как и `parser-core` сам по себе, поток выбирает
/// вызывающая сторона (`Task.detached(priority: .userInitiated)`, не главный поток).
/// Отмены нет: однажды начавшись, разбор идёт до конца и удерживает память, даже если
/// вызывающая сторона перестала ждать результат (`ipad-doc.md` разд. 10).
enum LocalParser {
    /// Разбирает демку по пути `demoPath` и пишет gzip-сжатый JSON в `outPath` (как
    /// описано в `docs/format.md`), перезаписывая файл, если он уже есть.
    static func parse(
        demoPath: String,
        outPath: String,
        onProgress: @escaping (ParseProgress) -> Void
    ) throws {
        final class ProgressBox {
            let callback: (ParseProgress) -> Void
            init(_ callback: @escaping (ParseProgress) -> Void) { self.callback = callback }
        }

        let box = ProgressBox(onProgress)
        let userData = Unmanaged.passRetained(box).toOpaque()
        defer { Unmanaged<ProgressBox>.fromOpaque(userData).release() }

        let status = demoPath.withCString { demoC in
            outPath.withCString { outC in
                cs2_parse_file(
                    demoC, outC,
                    { stage, percent, userData in
                        guard let userData else { return }
                        let box = Unmanaged<ProgressBox>.fromOpaque(userData).takeUnretainedValue()
                        let parsedStage = ParseStage(rawValue: stage) ?? .events
                        box.callback(ParseProgress(stage: parsedStage, percent: Int(percent)))
                    },
                    userData
                )
            }
        }

        guard status != 0 else { return }
        let message = Self.takeLastError() ?? "неизвестная ошибка"
        switch status {
        case 1: throw LocalParserError.invalidArgument(message)
        case 2: throw LocalParserError.io(message)
        case 3: throw LocalParserError.parseFailed(message)
        default: throw LocalParserError.internalError(message)
        }
    }

    private static func takeLastError() -> String? {
        guard let pointer = cs2_last_error() else { return nil }
        defer { cs2_string_free(pointer) }
        return String(cString: pointer)
    }

    /// Версия крейта `ios-bridge` (`Cargo.toml`), для экрана диагностики (этап I6). Строка
    /// статическая ('static в Rust) — в отличие от `cs2_last_error`, её не нужно освобождать.
    static var bridgeVersion: String {
        String(cString: cs2_bridge_version())
    }
}
