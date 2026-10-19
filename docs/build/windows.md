# Windows x64

Для разработки нужны Windows, Node.js 24.18.0/npm 12.2.0, Rust MSVC 1.99.1, Visual Studio Build Tools с C++ workload и Windows SDK. На GitHub Actions это предоставляет `windows-latest`. Мобильные SDK не используются.

```powershell
npm ci
cargo test --locked -p converter-core --test mdict_conformance
cargo test --locked -p converter-download --test checkpoint_recovery
cargo test --locked -p converter-app --test desktop_storage
npm run build:windows
```

Эта команда автоматически увеличивает третье число версии и синхронизирует манифесты npm, Rust и Tauri. Если версия уже подготовлена сборкой macOS для того же выпуска, используйте `npm run build:windows -- --reuse-version`. GitHub Actions всегда использует подготовленную версию из коммита. Подробнее о нумерации — в [инструкции macOS](macos.md).

Настоящий NSIS installer получается в `target/x86_64-pc-windows-msvc/release/bundle/nsis/`. Копия, сведения о сборке и SHA-256 — в `.work/packages/x86_64-pc-windows-msvc/`. Тот же набор собирается Windows job в `desktop.yml`; конфигурация на macOS сама по себе не считается подтверждением Windows-сборки.

`tauri.windows.conf.json` задаёт установку для текущего пользователя, русский/английский интерфейс и `offlineInstaller`: полный установщик WebView2 включается в NSIS. Для получения самого пакета требуется интернет при сборке. Готовый пакет рассчитан на установку без скачивания WebView2; это режим, описанный в [документации Tauri](https://v2.tauri.app/distribute/windows-installer/#offline-installer). Его проверка на системе без runtime фиксируется отдельно от обычного запуска на runner с уже установленным WebView2.

Данные находятся в `%APPDATA%\dev.dictionary.converter`. Папка кэша не очищается при закрытии приложения. Результаты публикуются отдельной директорией с суффиксом при совпадении имён. Ошибка сохранения не выдаёт частичные файлы за готовые и позволяет повторить публикацию.

## Протокол чистой VM

Использовать только disposable Windows VM, где WebView2 изначально отсутствует. Не удалять runtime с рабочего компьютера ради проверки.

1. Сохранить NSIS и его SHA-256, проверить соответствие `build-info.json`.
2. Сделать snapshot VM; записать Windows version и отсутствие WebView2.
3. Отключить сеть VM, установить NSIS и открыть Converter.
4. Через настоящий picker выбрать контрольный локальный ZIP и папку с кириллицей. Проверить MDX, MDD, JSON и поиск в независимом ридере.
5. Включить сеть для контролируемого URL-сервера: обрыв → пауза → закрытие → повторный запуск → продолжение. Сопоставить Range/If-Range, байты и итоговую SHA.
6. Проверить отмену, совпадение имён, повтор из кэша без нового GET и экспорт в другую папку.

Наличие конфигурации offlineInstaller не заменяет этот протокол. Выполненные проверки и недоступные среды перечислены в [desktop-native.md](../validation/desktop-native.md). Кодовая подпись Windows — отдельный этап публичного распространения; сертификаты не хранятся в проекте.
