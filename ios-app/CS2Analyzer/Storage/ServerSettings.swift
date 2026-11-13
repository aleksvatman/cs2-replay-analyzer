import Foundation

/// Адрес сервера хранится в `UserDefaults` (это не секрет), токен — в `TokenStore`
/// (Keychain), не в `UserDefaults` и не в логах (`ipad-doc.md` разд. 8). Адрес по умолчанию
/// — локальный сервер для разработки (`cargo run -p server`).
@Observable
final class ServerSettings {
    private static let addressKey = "server.address"
    private static let defaultAddress = "http://127.0.0.1:8080"

    var address: String {
        didSet { UserDefaults.standard.set(address, forKey: Self.addressKey) }
    }

    var token: String {
        didSet { TokenStore.save(token) }
    }

    init() {
        address = UserDefaults.standard.string(forKey: Self.addressKey) ?? Self.defaultAddress
        token = TokenStore.load() ?? ""
    }

    var baseURL: URL? {
        guard let url = URL(string: address), url.scheme == "http" || url.scheme == "https" else {
            return nil
        }
        return url
    }

    func makeClient() -> APIClient? {
        guard let baseURL else { return nil }
        return APIClient(baseURL: baseURL, tokenProvider: { [weak self] in self?.token })
    }
}
