import Foundation

/// Ошибки `RemoteParser` понятным пользователю текстом (`ipad-doc.md` разд. 8). Сеть
/// недоступна/таймаут отличаются от ответа сервера с кодом ошибки.
enum ServerError: Error, LocalizedError, Equatable {
    case network(String)
    case timeout
    /// Сервер ответил, но тело не похоже на ожидаемый JSON.
    case invalidResponse
    case server(code: String, message: String, status: Int, retryAfter: Int?)

    static func == (lhs: ServerError, rhs: ServerError) -> Bool {
        switch (lhs, rhs) {
        case (.network, .network), (.timeout, .timeout), (.invalidResponse, .invalidResponse):
            return true
        case let (.server(c1, m1, s1, r1), .server(c2, m2, s2, r2)):
            return c1 == c2 && m1 == m2 && s1 == s2 && r1 == r2
        default:
            return false
        }
    }

    var errorDescription: String? {
        switch self {
        case .network:
            return "Нет соединения с сервером."
        case .timeout:
            return "Сервер не отвечает (таймаут)."
        case .invalidResponse:
            return "Сервер прислал неожиданный ответ."
        case .server(let code, let message, _, let retryAfter):
            return Self.humanMessage(code: code, message: message, retryAfter: retryAfter)
        }
    }

    /// Код ошибки сервера (`server-doc.md` разд. 5) в понятный русский текст; неизвестный
    /// код показывает сообщение сервера как есть, а не падает молча.
    private static func humanMessage(code: String, message: String, retryAfter: Int?) -> String {
        switch code {
        case "unauthorized":
            return "Неверный токен доступа."
        case "payload_too_large":
            return "Файл больше лимита сервера."
        case "unsupported_media_type":
            return "Сервер не принял тип содержимого запроса."
        case "unprocessable":
            return "Не демка CS2 или не удалось её разобрать."
        case "too_many_requests":
            if let retryAfter {
                return "Слишком много запросов, повторите через \(retryAfter) с."
            }
            return "Слишком много запросов, повторите позже."
        case "queue_full":
            return "Очередь разбора на сервере переполнена, повторите позже."
        case "storage_full":
            return "На сервере уже максимум матчей, удалите часть, прежде чем грузить новые."
        case "not_ready":
            return "Результат разбора ещё не готов."
        case "not_found":
            return "Матч не найден на сервере."
        case "request_timeout":
            return "Загрузка прервалась — сервер перестал получать данные."
        default:
            return message
        }
    }
}
