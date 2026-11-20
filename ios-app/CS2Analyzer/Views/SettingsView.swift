import SwiftUI
import UIKit

/// Настройки (этап I6, `ipad-doc.md` разд. 11): лимит размера кэша, ручное удаление
/// закэшированных матчей, диагностика. Без онлайн-разбора (разд. 8a) выбора источника и
/// серверных настроек нет.
struct SettingsView: View {
    let library: MatchLibrary
    let settings: AppSettings

    @State private var copiedErrorLog = false

    var body: some View {
        NavigationStack {
            List {
                Section("Кэш матчей") {
                    LabeledContent("Сейчас занимает", value: formattedSize(library.cacheSizeBytes))
                    Picker("Максимальный размер", selection: maxCacheBinding) {
                        ForEach(AppSettings.cacheSizeOptionsMB, id: \.self) { megabytes in
                            Text(formattedSize(megabytes * 1_048_576)).tag(megabytes * 1_048_576)
                        }
                    }
                    Button("Очистить кэш", role: .destructive) {
                        for item in cachedItems {
                            library.deleteCached(id: item.id)
                        }
                    }
                    .disabled(cachedItems.isEmpty)
                }

                if !cachedItems.isEmpty {
                    Section("Разобранные на устройстве матчи") {
                        ForEach(cachedItems) { item in
                            Text("\(item.map) · \(item.roundsCount) раундов")
                        }
                        .onDelete { offsets in
                            for index in offsets {
                                library.deleteCached(id: cachedItems[index].id)
                            }
                        }
                    }
                }

                Section("Диагностика") {
                    LabeledContent("Версия приложения", value: appVersion)
                    LabeledContent("Версия формата", value: "\(supportedSchemaVersion)")
                    LabeledContent("Версия ios-bridge", value: LocalParser.bridgeVersion)
                    Button(copiedErrorLog ? "Журнал скопирован" : "Скопировать журнал ошибок") {
                        copyErrorLog()
                    }
                }
            }
            .navigationTitle("Настройки")
        }
    }

    /// Матчи кэша без подложенных вручную образцов (у них нет записи в `MatchStore`, поэтому
    /// `deleteCached` на них и так не действует, но явная фильтрация честнее, чем полагаться
    /// на побочный эффект).
    private var cachedItems: [MatchSummary] {
        library.items.filter { $0.id != "sample-mirage" && $0.id != "sample-ancient" }
    }

    private var maxCacheBinding: Binding<Int> {
        Binding(
            get: { settings.maxCacheBytes },
            set: { newValue in
                settings.maxCacheBytes = newValue
                library.enforceCacheLimit()
            }
        )
    }

    private var appVersion: String {
        let bundle = Bundle.main
        let shortVersion = bundle.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "?"
        let buildNumber = bundle.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? "?"
        return "\(shortVersion) (\(buildNumber))"
    }

    private func formattedSize(_ bytes: Int) -> String {
        ByteCountFormatter.string(fromByteCount: Int64(bytes), countStyle: .file)
    }

    private func copyErrorLog() {
        let text = ErrorLog.shared.formattedForCopying()
        UIPasteboard.general.string = text.isEmpty ? "Журнал пуст." : text
        copiedErrorLog = true
    }
}

#Preview {
    SettingsView(library: MatchLibrary(), settings: AppSettings())
}
