import Foundation

/// Результат `downloadData`: `notModified` значит, что сервер ответил `304` и локальный
/// файл по `knownETag` уже актуален — `fileURL`/`etag` в этом случае не меняются.
struct DownloadResult {
    var fileURL: URL
    var etag: String?
    var notModified: Bool
}

/// Клиент API сервера (`ipad-doc.md` разд. 8, `server-doc.md` разд. 5). `baseURL` — адрес
/// без завершающего `/matches` и т.п. (например `https://example.com` или
/// `http://127.0.0.1:8080` для разработки); `tokenProvider` читает токен из `TokenStore`
/// при каждом запросе, а не один раз при создании клиента.
final class APIClient: NSObject {
    let baseURL: URL
    let tokenProvider: () -> String?

    /// Короткие запросы (список, статус, удаление, health) — обычная сессия.
    private let session: URLSession
    /// Только для загрузки демки: фоновая сессия переживает сворачивание приложения
    /// (`ipad-doc.md` разд. 8), в отличие от обычной.
    private lazy var backgroundSession: URLSession = {
        let config = URLSessionConfiguration.background(
            withIdentifier: "com.avatman.cs2analyzer.upload-\(UUID().uuidString)"
        )
        config.sessionSendsLaunchEvents = true
        config.isDiscretionary = false
        return URLSession(configuration: config, delegate: self, delegateQueue: nil)
    }()

    private let lock = NSLock()
    private var uploadContinuations: [Int: CheckedContinuation<(Data, URLResponse), Error>] = [:]
    private var uploadResponseData: [Int: Data] = [:]

    init(baseURL: URL, tokenProvider: @escaping () -> String?, session: URLSession = URLSession(configuration: .default)) {
        self.baseURL = baseURL
        self.tokenProvider = tokenProvider
        self.session = session
    }

    private static let decoder: JSONDecoder = JSONDecoder()

    // MARK: - Запросы

    private func authorizedRequest(_ url: URL) -> URLRequest {
        var request = URLRequest(url: url)
        if let token = tokenProvider(), !token.isEmpty {
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        }
        return request
    }

    func checkHealth() async throws {
        let (_, response) = try await perform(URLRequest(url: Endpoints.health(base: baseURL)))
        try Self.throwIfError(response: response, data: Data())
    }

    func listMatches() async throws -> [MatchListItem] {
        let (data, response) = try await perform(authorizedRequest(Endpoints.matches(base: baseURL)))
        try Self.throwIfError(response: response, data: data)
        return try decode([MatchListItem].self, from: data)
    }

    func status(id: String) async throws -> MatchDetail {
        let (data, response) = try await perform(authorizedRequest(Endpoints.match(base: baseURL, id: id)))
        try Self.throwIfError(response: response, data: data)
        return try decode(MatchDetail.self, from: data)
    }

    func delete(id: String) async throws {
        var request = authorizedRequest(Endpoints.match(base: baseURL, id: id))
        request.httpMethod = "DELETE"
        let (data, response) = try await perform(request)
        try Self.throwIfError(response: response, data: data)
    }

    func reparse(id: String) async throws {
        var request = authorizedRequest(Endpoints.reparse(base: baseURL, id: id))
        request.httpMethod = "POST"
        let (data, response) = try await perform(request)
        try Self.throwIfError(response: response, data: data)
    }

    /// Загружает демку из файла (не из памяти, `ipad-doc.md` разд. 8) фоновой сессией.
    func upload(fileURL: URL, fileName: String) async throws -> UploadResponse {
        var request = authorizedRequest(Endpoints.matches(base: baseURL))
        request.httpMethod = "POST"
        request.setValue("application/octet-stream", forHTTPHeaderField: "Content-Type")
        request.setValue(fileName, forHTTPHeaderField: "X-File-Name")
        let (data, response) = try await performUpload(request, fileURL: fileURL)
        try Self.throwIfError(response: response, data: data)
        return try decode(UploadResponse.self, from: data)
    }

    /// Скачивает JSON матча сразу в файл (`ipad-doc.md` разд. 8), не в память.
    /// `knownETag`/`If-None-Match` позволяют получить `304` вместо повторной загрузки.
    func downloadData(id: String, knownETag: String?, to destination: URL) async throws -> DownloadResult {
        var request = authorizedRequest(Endpoints.matchData(base: baseURL, id: id))
        if let knownETag {
            request.setValue(knownETag, forHTTPHeaderField: "If-None-Match")
        }
        let (tempURL, response) = try await download(request)
        guard let http = response as? HTTPURLResponse else { throw ServerError.invalidResponse }

        if http.statusCode == 304 {
            try? FileManager.default.removeItem(at: tempURL)
            return DownloadResult(fileURL: destination, etag: knownETag, notModified: true)
        }
        if !(200...299).contains(http.statusCode) {
            let data = (try? Data(contentsOf: tempURL)) ?? Data()
            try? FileManager.default.removeItem(at: tempURL)
            throw Self.serverError(status: http.statusCode, data: data)
        }

        if FileManager.default.fileExists(atPath: destination.path) {
            try FileManager.default.removeItem(at: destination)
        }
        try FileManager.default.moveItem(at: tempURL, to: destination)
        return DownloadResult(fileURL: destination, etag: http.value(forHTTPHeaderField: "ETag"), notModified: false)
    }

    // MARK: - Транспорт

    private func perform(_ request: URLRequest) async throws -> (Data, URLResponse) {
        do {
            return try await session.data(for: request)
        } catch {
            throw Self.transportError(error)
        }
    }

    private func download(_ request: URLRequest) async throws -> (URL, URLResponse) {
        do {
            return try await session.download(for: request)
        } catch {
            throw Self.transportError(error)
        }
    }

    private func performUpload(_ request: URLRequest, fileURL: URL) async throws -> (Data, URLResponse) {
        try await withCheckedThrowingContinuation { continuation in
            let task = backgroundSession.uploadTask(with: request, fromFile: fileURL)
            lock.lock()
            uploadContinuations[task.taskIdentifier] = continuation
            uploadResponseData[task.taskIdentifier] = Data()
            lock.unlock()
            task.resume()
        }
    }

    private static func transportError(_ error: Error) -> ServerError {
        let nsError = error as NSError
        if nsError.domain == NSURLErrorDomain, nsError.code == NSURLErrorTimedOut {
            return .timeout
        }
        return .network(nsError.localizedDescription)
    }

    private func decode<T: Decodable>(_ type: T.Type, from data: Data) throws -> T {
        do {
            return try Self.decoder.decode(type, from: data)
        } catch {
            throw ServerError.invalidResponse
        }
    }

    /// Бросает `ServerError.server` для статусов вне 2xx; `304` (`If-None-Match`) и пустые
    /// тела (`DELETE`, `health`) не считаются ошибкой.
    private static func throwIfError(response: URLResponse, data: Data) throws {
        guard let http = response as? HTTPURLResponse else { throw ServerError.invalidResponse }
        guard !(200...299).contains(http.statusCode) else { return }
        throw serverError(status: http.statusCode, data: data, headers: http)
    }

    private static func serverError(status: Int, data: Data, headers: HTTPURLResponse? = nil) -> ServerError {
        let retryAfter = headers?.value(forHTTPHeaderField: "Retry-After").flatMap(Int.init)
        guard let body = try? decoder.decode(ServerErrorBody.self, from: data) else {
            return .server(code: "unknown", message: "Сервер вернул код \(status).", status: status, retryAfter: retryAfter)
        }
        return .server(code: body.error.code, message: body.error.message, status: status, retryAfter: retryAfter)
    }
}

extension APIClient: URLSessionDataDelegate {
    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
        lock.lock()
        uploadResponseData[dataTask.taskIdentifier, default: Data()].append(data)
        lock.unlock()
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        lock.lock()
        let continuation = uploadContinuations.removeValue(forKey: task.taskIdentifier)
        let data = uploadResponseData.removeValue(forKey: task.taskIdentifier) ?? Data()
        lock.unlock()
        guard let continuation else { return }
        if let error {
            continuation.resume(throwing: Self.transportError(error))
        } else if let response = task.response {
            continuation.resume(returning: (data, response))
        } else {
            continuation.resume(throwing: ServerError.invalidResponse)
        }
    }
}
