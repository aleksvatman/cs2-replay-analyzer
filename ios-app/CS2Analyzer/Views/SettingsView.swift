import SwiftUI

/// Адрес сервера и токен (этап I5). Полноценные настройки офлайн-режима и кэша — этап I11.
struct SettingsView: View {
    @Bindable var settings: ServerSettings
    @State private var checkResult: String?
    @State private var isChecking = false

    var body: some View {
        NavigationStack {
            Form {
                Section("Сервер") {
                    TextField("Адрес", text: $settings.address)
                        .textContentType(.URL)
                        .autocorrectionDisabled()
                        .textInputAutocapitalization(.never)
                        .keyboardType(.URL)
                    SecureField("Токен", text: $settings.token)
                        .autocorrectionDisabled()
                        .textInputAutocapitalization(.never)
                }

                Section {
                    Button {
                        checkConnection()
                    } label: {
                        if isChecking {
                            ProgressView()
                        } else {
                            Text("Проверить соединение")
                        }
                    }
                    .disabled(isChecking)

                    if let checkResult {
                        Text(checkResult)
                            .font(.caption)
                            .foregroundStyle(checkResult.hasPrefix("Ошибка") ? .red : .green)
                    }
                } footer: {
                    Text("Для разработки по умолчанию подставлен локальный сервер "
                        + "(cargo run -p server). Адрес должен быть https://, кроме localhost.")
                }
            }
            .navigationTitle("Настройки")
        }
    }

    private func checkConnection() {
        guard let client = settings.makeClient() else {
            checkResult = "Ошибка: неверный адрес сервера."
            return
        }
        isChecking = true
        checkResult = nil
        Task {
            do {
                try await client.checkHealth()
                checkResult = "Сервер отвечает."
            } catch let error as ServerError {
                checkResult = "Ошибка: \(error.errorDescription ?? "неизвестная ошибка")"
            } catch {
                checkResult = "Ошибка: \(error.localizedDescription)"
            }
            isChecking = false
        }
    }
}

#Preview {
    SettingsView(settings: ServerSettings())
}
