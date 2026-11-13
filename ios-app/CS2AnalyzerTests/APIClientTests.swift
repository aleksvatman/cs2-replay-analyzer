import XCTest
@testable import CS2Analyzer

final class APIClientTests: XCTestCase {
    private let baseURL = URL(string: "https://example.test")!

    override func tearDown() {
        URLProtocolStub.handler = nil
        super.tearDown()
    }

    private func makeClient(token: String? = "secret") -> APIClient {
        APIClient(baseURL: baseURL, tokenProvider: { token }, session: URLProtocolStub.makeSession())
    }

    func testListMatchesDecodesItems() async throws {
        let json = """
        [{"id":"abc","status":"done","map":"de_mirage","uploadedAt":1,"sizeBytes":2,
          "originalName":"m.dem","rounds":24,"score":[11,13],"schemaVersion":1}]
        """.data(using: .utf8)!

        URLProtocolStub.handler = { request in
            XCTAssertEqual(request.url, self.baseURL.appendingPathComponent("matches"))
            XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer secret")
            return (HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: nil)!, json)
        }

        let items = try await makeClient().listMatches()
        XCTAssertEqual(items.count, 1)
        XCTAssertEqual(items[0].id, "abc")
        XCTAssertEqual(items[0].status, .done)
        XCTAssertEqual(items[0].score, [11, 13])
    }

    func testStatusDecodesParsingProgress() async throws {
        let json = """
        {"id":"abc","status":"parsing","uploadedAt":1,"sizeBytes":2,
         "stage":"players","percent":42}
        """.data(using: .utf8)!

        URLProtocolStub.handler = { request in
            (HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: nil)!, json)
        }

        let detail = try await makeClient().status(id: "abc")
        XCTAssertEqual(detail.status, .parsing)
        XCTAssertEqual(detail.stage, "players")
        XCTAssertEqual(detail.percent, 42)
    }

    func testUnauthorizedMapsToReadableError() async throws {
        let json = """
        {"error":{"code":"unauthorized","message":"missing or invalid token"}}
        """.data(using: .utf8)!

        URLProtocolStub.handler = { request in
            (HTTPURLResponse(url: request.url!, statusCode: 401, httpVersion: nil, headerFields: nil)!, json)
        }

        do {
            _ = try await makeClient().listMatches()
            XCTFail("expected an error")
        } catch let error as ServerError {
            XCTAssertEqual(error, .server(code: "unauthorized", message: "missing or invalid token", status: 401, retryAfter: nil))
            XCTAssertEqual(error.errorDescription, "Неверный токен доступа.")
        }
    }

    func testTooManyRequestsCarriesRetryAfter() async throws {
        let json = """
        {"error":{"code":"too_many_requests","message":"slow down"}}
        """.data(using: .utf8)!

        URLProtocolStub.handler = { request in
            (
                HTTPURLResponse(
                    url: request.url!, statusCode: 429, httpVersion: nil,
                    headerFields: ["Retry-After": "30"]
                )!,
                json
            )
        }

        do {
            _ = try await makeClient().listMatches()
            XCTFail("expected an error")
        } catch let error as ServerError {
            XCTAssertEqual(error.errorDescription, "Слишком много запросов, повторите через 30 с.")
        }
    }

    func testCheckHealthSucceedsWithoutToken() async throws {
        URLProtocolStub.handler = { request in
            XCTAssertNil(request.value(forHTTPHeaderField: "Authorization"))
            return (HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: nil)!, Data("ok".utf8))
        }
        try await makeClient(token: nil).checkHealth()
    }

    func testNetworkFailureMapsToNetworkError() async throws {
        URLProtocolStub.handler = { _ in throw URLError(.notConnectedToInternet) }

        do {
            _ = try await makeClient().listMatches()
            XCTFail("expected an error")
        } catch let error as ServerError {
            guard case .network = error else {
                return XCTFail("expected .network, got \(error)")
            }
        }
    }

    func testDownloadDataSavesFileAndReturnsETag() async throws {
        let body = Data("{\"schemaVersion\":1}".utf8)
        URLProtocolStub.handler = { request in
            (
                HTTPURLResponse(
                    url: request.url!, statusCode: 200, httpVersion: nil,
                    headerFields: ["ETag": "\"abc\""]
                )!,
                body
            )
        }

        let destination = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: destination) }

        let result = try await makeClient().downloadData(id: "abc", knownETag: nil, to: destination)
        XCTAssertFalse(result.notModified)
        XCTAssertEqual(result.etag, "\"abc\"")
        XCTAssertEqual(try Data(contentsOf: destination), body)
    }

    func testDownloadDataNotModifiedKeepsKnownETag() async throws {
        URLProtocolStub.handler = { request in
            XCTAssertEqual(request.value(forHTTPHeaderField: "If-None-Match"), "\"abc\"")
            return (HTTPURLResponse(url: request.url!, statusCode: 304, httpVersion: nil, headerFields: nil)!, Data())
        }

        let destination = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        let result = try await makeClient().downloadData(id: "abc", knownETag: "\"abc\"", to: destination)
        XCTAssertTrue(result.notModified)
        XCTAssertEqual(result.etag, "\"abc\"")
    }
}
