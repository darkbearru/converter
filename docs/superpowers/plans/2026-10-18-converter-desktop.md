# Converter Desktop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Создать работающее Tauri + Vue приложение для macOS/Windows с локальным и URL-импортом, очередью, восстановлением загрузок и двумя независимыми прогрессами.

**Architecture:** Headless Rust application service объединяет библиотеки A/B через долговременную очередь и `PlatformStorage`; Tauri предоставляет узкие команды и события. Vue отображает snapshots и отправляет действия, не обрабатывает содержимое файлов. Тот же service и UI затем используются мобильной оболочкой.

**Tech Stack:** Tauri 2, Vue 3/TypeScript/Vite, Rust/Tokio, serde; Vitest/Vue Test Utils, Playwright; Windows NSIS с offline WebView2 installer.

**Spec:** [Спецификация](../specs/2026-10-18-dsl-mdx-converter-design.md), §3, §5, §8–11. Обязательны [общий план](2026-10-18-converter-implementation.md), контракты [A](2026-10-18-converter-core.md) и [B](2026-10-18-converter-download.md).

## Global Constraints

- Основные платформы первой версии — macOS и Windows.
- Отдельные индикаторы скачивания и конвертации для каждого задания.
- Промежуточные обновления ограничены частотой 5 раз в секунду; переходы состояний и последнее событие отправляются сразу.
- Очередь; одна активная конвертация. Словарь и его ресурсы полностью получены до конвертации.
- Русский интерфейс/ошибки, исходные файлы только для чтения, результаты не перезаписываются.
- `converter-core` не получает Tauri handles/URI; полные URL не попадают в публичные DTO/отчёт.
- Windows 10 22H2/Windows 11 x64, установщик NSIS с `webviewInstallMode: offlineInstaller`.
- Все остальные ограничения общего плана обязательны.

## Review Focus

1. Событие старого запуска или запоздавший initial snapshot: UI не откатывается; C1/C2.
2. Два файла, один без размера: общий процент отсутствует, известный файл показывает свой; C1/C2.
3. Возобновление после закрытия и устаревший output path: скачанные байты сохраняются, ошибка экспорта не требует новой конвертации; C1/C3.
4. Пауза первого задания не должна остановить готовое второе; C1.
5. Windows Unicode/длинный путь, занятый файл и существующая директория результата: исходники целы, публикация либо полная, либо явно неуспешная; C3/C5.

## Файловые границы и DTO

Создать crate `crates/converter-app` для очереди/состояния/хранилища без зависимости от Tauri. В `src-tauri` оставить конфигурацию, команды и интеграцию с native plugins. Это даёт возможность проверять orchestration без запуска WebView. Frontend создаётся в `src/`, contracts — в `src/shared/contracts.ts`, bridge — `src/shared/native.ts`; браузерный mock используется только в tests/dev harness.

`SourceInput = Local { import_id: String } | Remote { url: String, expected_sha256: Option<String> }`; URL принимается только в draft, в публичном snapshot его нет. `JobDraft { source: SourceInput, resources: Vec<SourceInput>, output: OutputChoice, options: ReadOptions, metadata_override: Option<DictionaryMetadata>, auto_start: bool }`. `OutputChoice = DesktopFolder(PathBuf) | AppLibrary`.

`JobSnapshot { id: String, generation: u64, run_id: String, sequence: u64, state: JobState, download: DownloadJobProgress, conversion: ConversionJobProgress, result: Option<ResultInfo>, export: ExportSnapshot }`. `JobState` включает все состояния §8 спецификации. `DownloadJobProgress { kind: Local | Remote, files: Vec<DownloadProgress>, received_bytes: u64, confirmed_bytes: u64, total_bytes: Option<u64>, percent: Option<f64> }`; `ConversionJobProgress { status: NotStarted | Running | Succeeded | Failed | Cancelled | Interrupted, current: Option<StageProgress>, completed_stages: Vec<StageProgress> }`. Событие `JobEvent { snapshot: JobSnapshot }` передаёт identifiers/sequence внутри snapshot; в JSON используются `snake_case` поля и строковые enum values.

`generation` увеличивается и сохраняется до нового worker, `run_id` обновляется, `sequence` возрастает внутри запуска. Merge snapshots сравнивает пару `(generation, sequence)`; поэтому поздний initial get и старые events безопасны. Сам pipeline выдаёт только CoreEvent из A; service добавляет идентичность запуска. Terminal/state events отправляются сразу, throttle применяется только к промежуточным progress.

`PlatformStorage` — `#[async_trait]`, `Send + Sync`: `async import(selection: PlatformSelection, ctx: &RunContext) -> AppResult<Vec<PreparedFile>>`, `create_staging(job_id: &str, output: &OutputChoice) -> AppResult<PathBuf>`, `async publish(artifacts: ConvertedArtifacts, output: &OutputChoice, ctx: &RunContext) -> AppResult<PublishedResult>`, `async export(result: &PublishedResult, choice: ExportChoice, ctx: &RunContext) -> AppResult<ExportSnapshot>`. `PlatformSelection = LocalPaths(Vec<PathBuf>) | NativeHandles(Vec<String>)`; `ExportChoice = Folder(PathBuf) | DocumentHandle(String) | ZipDocumentHandle(String)`. `PublishedResult` содержит принадлежащую приложению/назначению директорию и список файлов; `ResultInfo` раскрывает UI только ID, имена, размеры и разрешённые действия. Реальные пути и handles не выводятся как технические поля продуктового интерфейса.

## Task 1: C1 — Долговременная очередь, связывание A/B и snapshot-прогресс

**Files:** Create `crates/converter-app/{Cargo.toml,src/lib.rs}`, `src/contracts.rs`, `src/jobs/{mod,manager,state,store,scheduler,progress}.rs`, `src/storage/mod.rs`, `examples/export_contract_fixtures.rs`; Test `tests/{job_lifecycle,progress_contract,persistence}.rs`; root workspace member. HTTP tests используют dev-only `converter-test-support` из B2, без копирования сервера.

**Interfaces:** Consumes core/downloader/storage contracts. Produces `JobManager::open(data_dir: PathBuf, storage: Arc<dyn PlatformStorage>) -> AppResult<Self>` и async `import(selection: PlatformSelection) -> AppResult<Vec<ImportedSourceInfo>>`, `create(JobDraft) -> AppResult<JobSnapshot>`, `start(id: &str)`, `pause_download(id: &str)`, `resume_download(id: &str)`, `cancel(id: &str)`, `update(id: &str, options: JobOptionsUpdate)`, `list() -> Vec<JobSnapshot>`, `diagnostics(id: &str, offset: u64, limit: u32) -> DiagnosticPage`, `export(id: &str, ExportChoice) -> AppResult<ExportSnapshot>`. Control-методы возвращают новый `JobSnapshot`. `ImportedSourceInfo { import_id, display_name, bytes: Option<u64>, is_directory: bool }` не раскрывает path. `subscribe() -> broadcast::Receiver<JobEvent>`; `JobOptionsUpdate` содержит только options/metadata/output/resource assignment для `awaiting_input`. Example `export_contract_fixtures` сериализует контролируемые snapshots для frontend tests C2.

- [x] **Step 1:** В `не_конвертирует_неполный_источник`/`пауза_не_блокирует_очередь` fake storage и controlled HTTP удерживают attachment. В `позднее_событие_не_откатывает_готовность` передать предыдущую generation. Assert:

  ```rust
  assert_eq!(conversion_invocations_before_all_downloads, 0);
  assert_eq!(max_active_conversions, 1);
  assert!(second_job_completed_while_first_paused);
  assert_eq!(after_stale_event.state, JobState::Succeeded);
  assert!(mixed_known_and_unknown.download.total_bytes.is_none());
  ```

  Дополнительно crash/reopen: downloads → paused, conversion → interrupted; completed bytes/results сохранены, temporary conversion files очищены. Тест clock выдаёт сотни CoreEvent за секунду: progress ≤5/s, state/final event без задержки.
- [x] **Step 2:** Run `cargo test -p converter-app --test job_lifecycle --test progress_contract --test persistence`; ожидается FAIL.
- [x] **Step 3:** Реализовать manager и файловый job store с schema version и атомарными snapshots. Идентификаторы и пути создаёт service; imports регистрируются до Local source ID. Native download-cache, jobs, scratch и results — отдельные каталоги. Один активный source transfer и одна активная конвертация, workers освобождаются на паузе/awaiting_input. `spawn_blocking` для core, async для сети. Job download total — `None`, пока хотя бы один remote total неизвестен; завершение скачивания сохраняется при старте core.
- [x] **Step 4:** Run Step 2 с `--locked`; PASS. Проверить Ctrl-C/shutdown cancellation, restart generation, отсутствие URL tokens в DTO и отказ удалить in-use cache. В fake storage проверить same-filesystem staging на desktop; `publish` повторно использует A6.
- [x] **Step 5:** Commit `feat: добавить очередь и независимые снимки прогресса`.

## Task 2: C2 — Vue UI, формы источников и два прогресса

**Files:** Create `package.json`, `package-lock.json`, `index.html`, `vite.config.ts`, `vitest.config.ts`, `tsconfig*.json`, `src/{main.ts,App.vue}`, `src/shared/{contracts,native,format}.ts`, `src/features/sources/SourceForm.vue`, `src/features/jobs/{store.ts,JobQueue.vue,JobSettings.vue}`, `src/features/progress/{DownloadProgress.vue,ConversionProgress.vue}`, `src/features/results/{ResultPanel.vue,DiagnosticsList.vue}`, `src/styles/{tokens,app}.css`, `tests/ui/{progress,job_store,source_form}.test.ts`, `tests/ui/setup.ts`, `tests/ui/fixtures/`.

**Interfaces:** Consumes Rust snapshots from C1. `NativeBridge` exposes `listFormats`, `pickSources`, `pickOutput`, `createJob`, `startJob`, `pauseDownload`, `resumeDownload`, `cancelJob`, `updateJob`, `getJobs`, `readDiagnostics`, `exportResult`, `onJobEvent`; concrete arguments map to C1/C3. `createJobStore(bridge: NativeBridge)` exposes `jobs`, `mergeSnapshot(snapshot)`, `dispose()` and action methods. Компоненты прогресса принимают только соответствующий progress object; не запрашивают файлы/сеть сами.

- [x] **Step 1:** В tests задать русские сценарии и Rust-compatible JSON fixtures:

  ```ts
  expect(screen.getByRole('progressbar', { name: 'Скачивание' })).toHaveAttribute('aria-valuenow', '50');
  expect(screen.getByRole('progressbar', { name: 'Конвертация' })).not.toHaveAttribute('aria-valuenow');
  expect(screen.getByText('Локальный файл')).toBeVisible();
  expect(store.jobs[id].state).toBe('succeeded'); // после события старой generation
  ```

  Сценарии: download 50% + conversion not started; download completed остаётся при conversion running; unknown total не имеет aria-valuenow/процента; локальный файл не рисует ложные 0%; pause/restart reason; два remote файла с одним unknown; counter больше 4 GiB форматируется без 32-битного overflow. Получать fixtures из C1 example `export_contract_fixtures` и сравнивать с checked-in JSON.
- [x] **Step 2:** Run `npm test -- --run tests/ui/progress.test.ts tests/ui/job_store.test.ts tests/ui/source_form.test.ts`; ожидается FAIL на отсутствующих компонентах. Перед запуском установить Vue/Vite/TypeScript/Vitest/Test Utils/testing-library для Vue, jsdom и jest-dom; настроить jsdom и `@testing-library/jest-dom/vitest` через setup.ts и зафиксировать lockfile. Не писать реализацию до фиксации отрицательного результата теста, а не ошибки отсутствующего test runner.
- [x] **Step 3:** Реализовать адаптивный русский UI: формы files/URL, SHA optional, resource URL, output/options; очередь, отдельные прогрессы, результат и постраничный журнал. Store слушает events до initial snapshot и merge по generation/sequence. Для неопределённого total отсутствует значение progress; conversion показывает текущий stage/unit и completed stages, не взвешенный общий процент. Кнопка «Продолжить» относится к download. Применить актуальные CSS/style и frontend-polish навыки на этапе UI-реализации; не добавлять словарный редактор/браузер.
- [x] **Step 4:** Run Step 2, затем `npm run typecheck`; PASS. Формы явно показывают ошибки URL/кодировки/assignment без показа runtime identifiers; keyboard/focus и текстовые статусы доступны без цвета. Нет HTML-вставки из diagnostics.
- [x] **Step 5:** Commit `feat: показать очередь и раздельный прогресс в Vue`.

## Task 3: C3 — Tauri IPC и настоящий desktop файловый сценарий

**Files:** Create `src-tauri/{Cargo.toml,build.rs,tauri.conf.json}`, `src-tauri/capabilities/main.json`, `src-tauri/src/{main,lib,commands,events}.rs`, `src-tauri/src/storage/{mod,picker}.rs`, `crates/converter-app/src/storage/desktop.rs`, `crates/converter-app/tests/desktop_storage.rs`; Modify native bridge, npm scripts/workspace. Установить официальные dialog/opener плагины; не включать произвольный shell/fs доступ для frontend.

**Interfaces:** Tauri команды snake_case соответствуют NativeBridge; `pick_sources` регистрирует импорт и возвращает безопасные source IDs, `pick_output` возвращает выбор назначения; `create_job`, `start_job`, `pause_download`, `resume_download`, `cancel_job`, `update_job`, `get_jobs`, `read_diagnostics`, `export_result`, `list_formats`. Event name `job:event`; Rust `run()` имеет mobile entry point через `cfg_attr(mobile, tauri::mobile_entry_point)` для D. Browser bridge по умолчанию недоступен в packaged app, mock включён только dev/test build.

- [x] **Step 1:** В app integration test `не_экспортирует_за_пределы_результата`/`публикует_в_папку_с_кириллицей` проверить реальные `PlatformStorage` desktop adapters, занятый target и collision suffix:

  ```rust
  assert!(published.directory.starts_with(&chosen_destination));
  assert_eq!(source_hash_before, source_hash_after);
  assert_eq!(failed_publish.visible_final_files, 0);
  assert_ne!(first_result.directory, second_result.directory);
  ```

  Run `cargo test -p converter-app --test desktop_storage` до реализации adapter; ожидается FAIL.
- [x] **Step 2:** Реализовать permissions/capabilities, команды, событийный forwarder и desktop picker. Уважать выбранные источники; открыть результат через ID, не произвольную командную строку. Определить app identifier `dev.dictionary.converter`, productName `Converter`, version `0.1.0`. CLI не упаковывать как sidecar: app линкует Rust-библиотеки.
- [x] **Step 3:** Run `cargo test --locked -p converter-app --test desktop_storage`, `npm run typecheck`, `npm run build`; ожидается PASS. Затем `npm run tauri -- dev`: реальный локальный fixture → MDX/MDD в выбранной папке, настоящий диалог и отмена.
- [x] **Step 4:** В настоящем Tauri приложении запустить URL controlled-server fixture: обрыв → pause → закрытие → запуск → resume → conversion. Зафиксировать два progress blocks, byte counters после reopen, отсутствие нового GET при повторной конвертации. Это нативный smoke, browser mock его не заменяет.
- [x] **Step 5:** Commit `feat: связать Tauri с локальным и URL-конвейером`.

## Task 4: C4 — Frontend сценарии, адаптивность и контракт прогресса

**Files:** Create `playwright.config.ts`, `tests/ui/harness/{main.ts,bridge.ts,index.html}`, `tests/ui/e2e/{conversion,download_progress,responsive}.spec.ts`, `docs/validation/desktop-ui.md`; Modify package scripts.

**Interfaces:** Harness implements NativeBridge из C2; scripted scenarios выдаются теми же JSON DTO, в том числе старой generation. Scripts: `test:e2e` запускает Playwright; `test` — Vitest, `typecheck` — vue-tsc/tsc, `build` — production frontend, `tauri` — CLI Tauri. Harness не попадает в production dist.

- [x] **Step 1:** Установить Playwright как dev-dependency, настроить test:e2e script и нужные test browsers. Написать failing browser tests: local ready, URL downloading→paused→resumed→conversion→ready, unknown total, old event, длинное русское название и несколько страниц diagnostics. Проверить видимые independent statuses:

  ```ts
  await expect(page.getByRole('progressbar', { name: 'Скачивание' })).toHaveAttribute('aria-valuenow', '100');
  await expect(page.getByText('Конвертация: запись MDX')).toBeVisible();
  await expect(page.getByText('Готово', { exact: true })).toHaveCount(0); // conversion ещё идёт
  ```

- [x] **Step 2:** Run `npm run test:e2e`; ожидается FAIL на отсутствующем harness/scenarios. Затем реализовать test harness и минимально исправить обнаруженные UI-сбои. Не добавлять production network mocks.
- [x] **Step 3:** Run `npm test -- --run`, `npm run typecheck`, `npm run test:e2e`; PASS для desktop 1280×800 и mobile 390×844, keyboard-only сценария, reduced motion и крупного текста. Chromium/WebKit покрывают frontend; это не подтверждение реального iOS/macOS приложения.
- [x] **Step 4:** В `docs/validation/desktop-ui.md` сохранить результаты, скриншоты двух прогрессов и ограничения покрытия. Verify production dist не содержит harness/fixture/URL tokens.
- [x] **Step 5:** Commit `test: проверить прогресс и сценарии интерфейса`.

## Task 5: C5 — Сборки macOS/Windows и воспроизводимая проверка

**Files:** Create `.github/workflows/{ci,desktop}.yml`, `src-tauri/tauri.windows.conf.json`, `docs/build/{macos,windows}.md`, `docs/validation/desktop-native.md`; Modify tauri config/icons/.gitignore. Workflows только собирают/тестируют и сохраняют артефакты; не публикуют релиз и не требуют создания удалённого репозитория в рамках реализации.

**Interfaces:** npm `build:macos` вызывает `tauri build --target aarch64-apple-darwin`; `build:macos:intel` — `x86_64-apple-darwin`; `build:windows` на Windows — `tauri build --target x86_64-pc-windows-msvc --bundles nsis --config src-tauri/tauri.windows.conf.json`. Windows config включает offlineInstaller. CI запускает Rust tests на macOS/Windows/Linux для библиотек и frontend tests на одной ОС; нативные app bundles — на своих ОС.

- [ ] **Step 1:** До финальной упаковки прогнать conformance, checkpoint и desktop_storage tests на Windows. Ожидается PASS; обнаруженное платформенное различие сначала фиксировать падающим тестом в owning library. В отдельной disposable Windows VM подготовить offline install smoke без установленного WebView2; не удалять runtime на рабочем компьютере пользователя.
- [ ] **Step 2:** Реализовать build scripts/config/workflows и инструкции команд, SDK, подписи и путей артефактов. `Cargo.lock`, npm lock, toolchain и номера native outputs связываются с commit SHA. Подпись/нотаризация описаны отдельно; секреты не запрашивать до необходимости подписывать конкретный артефакт.
- [ ] **Step 3:** Run `npm run build:macos`, `npm run build:macos:intel` при наличии target. На Windows run `npm run build:windows`; ожидаются `.app/.dmg`, Intel bundle и NSIS setup. Наличие Windows config на Mac не засчитывается как успешная Windows-сборка.
- [ ] **Step 4:** Установить/запустить packaged macOS/Windows app и выполнить local/URL resume flow, кириллический output path, cancel и независимое открытие MDX/MDD. На disposable Windows VM отключить сеть и проверить установку с offline WebView2 и локальную конвертацию. Записать фактическую матрицу в `desktop-native.md`; недоступные среды/подпись оставить явно непроверенными, без заявления о готовности соответствующего выпуска.
- [ ] **Step 5:** Commit `build: подготовить macOS и Windows дистрибутивы`.

После C5 готов основной desktop-результат при выполненных native checks. Следующая часть — [мобильные платформы D](2026-10-18-converter-mobile.md).
