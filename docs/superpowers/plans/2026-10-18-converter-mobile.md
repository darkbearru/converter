# Converter Mobile Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Получить функциональные iOS/Android сборки общего конвертера с системными документами, URL-докачкой, раздельными прогрессами и сохранением MDX/MDD.

**Architecture:** UI, JobManager, downloader и core используются из A/B/C. Небольшой локальный Tauri plugin реализует потоковый перенос между системными документами и sandbox. Ядро получает только локальные подготовленные пути; мобильный lifecycle сохраняет контрольные точки, а экспорт отделён от завершения конвертации.

**Tech Stack:** Tauri 2 mobile, Rust, Vue/TypeScript, Swift/UIKit, Kotlin/Storage Access Framework; Xcode/iOS simulator, Android SDK/NDK/JDK и emulator.

**Spec:** [Спецификация](../specs/2026-10-18-dsl-mdx-converter-design.md), §5.2, §8–11. Обязательны [общий план](2026-10-18-converter-implementation.md) и `PlatformStorage`/JobManager/NativeBridge из [C](2026-10-18-converter-desktop.md).

## Global Constraints

- Минимальная версия приложения — iOS 15; Android 10/API 29.
- iOS device `aarch64-apple-ios`, simulator `aarch64-apple-ios-sim`; Android ARM64 и x86_64, без 32-bit ABI.
- Начальный бюджет буфера сортировки — 16 MiB на мобильных платформах.
- Отдельные индикаторы скачивания и конвертации для каждого задания.
- Работать на переднем плане; после остановки процесса download → paused с checkpoint, conversion → interrupted.
- Сначала полные локальные копии источников, затем конвертация; не интерпретировать `content://` как обычный путь.
- Выбор папки не требуется: ZIP с DSL/ресурсами и системные документы; готовые файлы/ZIP сохраняются явно.
- Сборка симулятора, запуск, device signing и распространение — разные проверки. Публикации в магазинах нет.
- Все остальные ограничения общего плана обязательны.

## Review Focus

1. Провайдер вернул URI без стабильного пути/размера: копирование работает потоково, core получает локальный файл; D1.
2. Пользователь отменил picker/экспорт: нет ошибочного failed conversion, готовые результаты остаются; D1/D2.
3. Приложение ушло в фон во время SHA/checkpoint/MDX: нет ложной готовности, восстановление не теряет download cache; D2.
4. Экспортирован MDX, а MDD сохранить не удалось: статус показывает частичный экспорт и позволяет повторить без нового GET/core; D2/D3.
5. Safe area, крупный текст, неизвестный total: кнопки и подписи обоих прогрессов доступны на маленьком экране; D2/D3.

## Файловые границы и контракт моста

Создать локальный plugin `plugins/document-bridge` с Rust façade `src/{lib,model,commands}.rs`, iOS `ios/Sources/{DocumentBridgePlugin,DocumentAccess,StreamCopy}.swift`, Android `android/src/main/java/dev/dictionary/converter/documents/{DocumentBridgePlugin,DocumentAccess,StreamCopy}.kt` и нативными tests. Plugin member добавляется только при создании; на desktop façade компилируется без мобильного runtime, команды возвращают typed Unsupported.

Публичный Rust API plugin: `pick_documents() -> Result<PickOutcome>`, `import_document(handle: NativeDocumentHandle, destination_id: String, control: CopyControl) -> Result<ImportedDocument>`, `export_documents(source_ids: Vec<String>, as_zip: bool, control: CopyControl) -> Result<ExportOutcome>`. Это async-операции через `run_mobile_plugin`; `PickOutcome = Selected(Vec<NativeDocumentHandle>) | Cancelled`, `ExportOutcome = Saved(Vec<ExportedDocument>) | Cancelled | Partial { saved, failures }`. Handle содержит opaque ID, отображаемое имя и `Option<u64>` размера; не передаёт raw URI в core/публичный отчёт.

`ImportedDocument { id, local_path, display_name, bytes }` возвращается только Rust-оболочке, которая делает `PreparedFile` и регистрирует import ID. `CopyControl` поддерживает отмену и события скопированных байтов, буфер 1 MiB. `destination_id` и source IDs разрешаются в собственном native registry: frontend не задаёт произвольный путь записи. Выбор и время жизни security-scoped URL/SAF URI принадлежат plugin. Сами байты не идут через JS/IPC.

Экспортная реализация использует системный документный диалог подходящей платформы: iOS экспортирует уже готовые локальные URL через document picker, Android пишет в URI, предоставленный SAF. Внутренняя разница не просачивается в JobManager; `PlatformStorage::export` преобразует результат в общий ExportSnapshot. Если ZIP нужен, его сначала потоково готовит Rust-адаптер в своём каталоге и потом передаёт bridge.

## Task D1: Нативный потоковый импорт/экспорт документов

**Files:** Create plugin выше, `plugins/document-bridge/{Cargo.toml,build.rs,permissions/default.toml}`, `ios/Tests/{DocumentAccessTests,StreamCopyTests}.swift`, Android `src/test/.../{DocumentAccessTest,StreamCopyTest}.kt`; Create `src-tauri/src/storage/mobile.rs`; Modify src-tauri dependency/capabilities.

**Interfaces:** Produces bridge API выше и `MobileStorage: PlatformStorage` из C. Consumes C contracts и A RunControl/PreparedFile. Перед Swift-реализацией прочитать актуальный `write-swift`; native bridge работать по `Tauri Mobile Plugin Development`, не через выдуманные API.

- [ ] **Step 1:** До реализации native копирования написать тесты с временными streams/документами: URI без filesystem path, unknown length, ресурс больше буфера, revoked permission и отмена. Assert:

  ```text
  SHA256(imported.local_path) == SHA256(source)
  максимальный буфер копирования <= 1_048_576 байтов
  picker cancel -> PickOutcome.Cancelled
  revoked access -> code "document_access_revoked"
  ```

  Compile/test façade на desktop. Для Android использовать `./gradlew :document-bridge:testDebugUnitTest` из `src-tauri/gen/android`; имя модуля зафиксировать в generated settings.gradle. Для iOS включить bridge tests в тестовый target `DocumentBridgeTests` workspace `src-tauri/gen/apple/Converter.xcworkspace` и запускать `xcodebuild -workspace src-tauri/gen/apple/Converter.xcworkspace -scheme DocumentBridgeTests -destination 'platform=iOS Simulator,id=<booted-UDID>' test` из корня после подготовки SDK. Фактический UDID получить через `xcrun simctl list devices booted`.
- [ ] **Step 2:** Подготовить полный Xcode, simulator, CocoaPods и Android JDK/SDK/NDK только при исполнении этого этапа; системные установки проводить через обычные разрешения среды. Сгенерировать mobile проекты Tauri, подключить plugin и реальные тестовые targets; создать явно названный workspace `Converter.xcworkspace` с generated project и тестовым target, если generated workspace называется иначе. Запустить native tests до реализации StreamCopy: ожидается FAIL на отсутствующей реализации. Первоначальный отсутствующий SDK не отмечать как FAIL продукта или успешный тест.
- [ ] **Step 3:** Реализовать Swift DocumentAccess с ограниченным временем security-scoped access и координацией файлов, Kotlin SAF через ContentResolver streams. Данные потоково копируются в sandbox до входа в core; registry handles освобождается на завершении/отмене. iOS picker cancel и Android result cancellation не создают failed conversion. Не запрашивать manage-all-files permission.
- [ ] **Step 4:** Запустить native tests и `cargo test --locked -p converter-app`; ожидается PASS. В симуляторе/эмуляторе вручную выбрать DSL/ZIP из Documents/provider и экспортировать небольшой fixture; сравнить хеши исходных и полученных документов. Проверить, что память не растёт на размер одиночного ресурса.
- [ ] **Step 5:** Commit `feat: добавить мобильный доступ к документам`.

## Task D2: Мобильная очередь, lifecycle и интерфейс результата

**Files:** Modify `crates/converter-app/src/jobs/{manager,state,store}.rs`, `src-tauri/src/{lib,events}.rs`, `src-tauri/src/storage/mobile.rs`, `src/features/{sources,progress,results}/`, `src/styles/app.css`; Create `crates/converter-app/tests/mobile_lifecycle.rs`, `tests/ui/mobile_export.test.ts`, `docs/validation/mobile-scenarios.md`.

**Interfaces:** Produces `JobManager::suspend_for_background() -> AppResult<()>`: download pause/checkpoint, активная конвертация cooperative stop → Interrupted, без публикации. `AppLibrary` хранит готовые результаты независимо от временного каталога. Bridge import/export outcomes отображаются через C NativeBridge, без второй очереди. `ConversionJobProgress.status` поддерживает `Interrupted` наряду с другими состояниями.

- [ ] **Step 1:** В `фон_сохраняет_докачку_но_не_половину_конвертации` и `повтор_экспорта_не_запускает_core` проверить service с fake lifecycle и export failure:

  ```rust
  assert_eq!(download_job.state, JobState::DownloadPaused);
  assert_eq!(conversion_job.state, JobState::Interrupted);
  assert!(cache.verify_prefix(&source_id)?.confirmed_bytes > 0);
  assert_eq!(core_invocations_after_export_retry, core_invocations_before);
  assert_eq!(get_count_after_export_retry, get_count_before);
  ```

  В UI test проверить partial export: MDX сохранён, MDD не сохранён, готовая конвертация остаётся готовой, есть действие повтора. В progress tests подтвердить раздельные индикаторы на 390×844 и отсутствие ложного процента.
- [ ] **Step 2:** Run `cargo test -p converter-app --test mobile_lifecycle` и `npm test -- --run tests/ui/mobile_export.test.ts`; ожидается FAIL.
- [ ] **Step 3:** Связать native background/lifecycle с service, хранить results/cache отдельно, использовать mobile Limits. Создать Rust `mobile_export.rs` для ZIP и сопоставления результатов отдельных файлов. UI поддерживает выбор документов/URL, «Сохранить ZIP»/«Сохранить файлы», safe area, крупный текст и keyboard; desktop drag/drop не обязателен для mobile. Событие ухода в фон останавливает новые работы; возвращение не авторизует автоматический restart interrupted conversion.
- [ ] **Step 4:** Run Step 2 с `--locked` для Cargo, frontend typecheck и responsive e2e; PASS. В настоящем mobile app проверить импорт и URL-сценарий без доступа к соседним файлам, уход в фон/возврат, остановку процесса/перезапуск, partial export и отдельные прогрессы. Неполная download SHA-проверка после процесса повторяется по локальному файлу, не выставляет Completed заранее.
- [ ] **Step 5:** Commit `feat: адаптировать очередь и экспорт для мобильных платформ`.

## Task D3: Сборки и итоговая платформенная матрица

**Files:** Create `src-tauri/tauri.{ios,android}.conf.json`, `src-tauri/gen/apple/PrivacyInfo.xcprivacy`, platform entitlements/configs и Android Manifest; `docs/build/{ios,android}.md`, `docs/validation/release-matrix.md`, `.github/workflows/mobile.yml`; Modify npm scripts/.gitignore. Generated native source/config хранить в Git; build outputs, local.properties, signing profiles/keys — исключить.

**Interfaces:** npm scripts `build:ios:sim` → `tauri ios build --debug --target aarch64-sim`, `build:ios` → `tauri ios build --target aarch64`; `build:android` → `tauri android build --apk --target aarch64 x86_64`; `build:android:aab` → `tauri android build --aab --target aarch64 x86_64`. iOS 15 и Android minSdkVersion 29 задаются в config; Android INTERNET permission и ABI scopes явны. Опции CLI соответствуют [Tauri CLI](https://v2.tauri.app/reference/cli/), а не Rust triple-именам.

- [ ] **Step 1:** Прочитать команды сборки из фактически установленной Tauri CLI, проверить минимальные ОС, разрешения и privacy manifest; вручную убедиться, что production bundle не включает UI harness, debug fixtures, webdriver test server и приватные URL. Перед изменением конфигурации добавить простой packaging assertion только на эти реальные риски, без тестов, повторяющих весь JSON-конфиг.
- [ ] **Step 2:** Реализовать scripts/config/instructions и mobile CI build jobs. iOS simulator build не требует выдавать его за подписанный device archive. Android debug signing предназначен для emulator smoke; release signing — отдельная операция. Не публиковать в магазины и не создавать аккаунты/ключи пользователя.
- [ ] **Step 3:** Run `npm run build:ios:sim` и `npm run build:android`; ожидаются simulator `.app` и APK ARM64/x86_64. Установить через `xcrun simctl install`/`adb install` в подготовленные устройства, запустить и выполнить local ZIP → MDX/MDD и URL interruption/resume flow. Controlled HTTP server из B2 доступен Android emulator через `10.0.2.2`, iOS simulator — через host loopback; не подставлять Android `127.0.0.1` как адрес host. Не отключать production TLS-проверки ради fixture. Проверить экспортируемые файлы независимым reader на host после сохранения.
- [ ] **Step 4:** Run библиотечные тесты, frontend typecheck/unit/e2e и повторить только затронутые mobile native checks. Заполнить release-matrix четырьмя платформами: сборка, установка, запуск, local flow, URL resume, два прогресса, cancellation, independent MDX/MDD read, signing. Непроведённые device/signing проверки указать отдельно; не заменять их simulator build или browser WebKit.
- [ ] **Step 5:** Commit `build: подготовить iOS и Android сборки и матрицу проверок`.

После D3 провести независимое итоговое ревью согласованных изменений и проверить матрицу со спецификацией. Готовность объявляется по фактически выполненным критериям; публикация релиза/магазинов в объём не входит.
