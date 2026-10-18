# Converter Download Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Получать словари по прямым HTTP/HTTPS-ссылкам с отдельным прогрессом, проверкой локальной копии и восстановлением после обрыва/перезапуска до начала конвертации.

**Architecture:** Нативный Rust downloader сохраняет `.part` и атомарные контрольные точки в постоянном кэше. Продолжение разрешено только для подтверждённого префикса и той же версии ресурса; завершение возвращает локальный файл. Он не зависит от DSL/Tauri; CLI и оболочка связывают готовый файл с reader.

**Tech Stack:** Rust, Tokio, reqwest 0.13 с `default-features = false`, `rustls` и `stream`; serde, SHA-256, UUID, URL; контролируемый HTTP fixture server в Rust-тестах.

**Spec:** [Спецификация](../specs/2026-10-18-dsl-mdx-converter-design.md), §4.3, §8, §10, §11.3. [Общий план](2026-10-18-converter-implementation.md); CLI-контракты из [A](2026-10-18-converter-core.md).

## Global Constraints

- Все ограничения общего плана обязательны; `converter-download` не зависит от `converter-core`, WebView и браузерных cookies.
- Сначала полностью скачать и проверить все источники задания; потом конвертировать.
- Отдельные индикаторы скачивания и конвертации для каждого задания.
- HTTP/HTTPS, DSL/DZ/ZIP; сохранение без автоматической HTTP-декомпрессии.
- Сильный ETag и `Range`/`If-Range`; при небезопасной докачке — полная загрузка с явной причиной.
- `.part` и манифесты не удаляются очисткой временных файлов конвертации.
- Русские сообщения; полный URL с токенами не включается в журнал или отчёт.

## Review Focus

1. Процесс погиб между сохранением порции и манифеста: непроверенный хвост не используется; B1.
2. Сервер возвращает `200`/`416` или другой ETag на resume: нельзя дописывать другую версию; B2.
3. Редирект меняет конечный ресурс, а одинаковый ETag выдан другим URL: сохранённая часть не переносится автоматически; B2.
4. Размер неизвестен, один attachment готов, второй ещё качается: есть байты без вымышленного общего процента, конвертация не запущена; B3/B4.
5. Обрыв/повтор/нехватка места: счётчики отражают сохранённые данные, повтор не уничтожает пригодный префикс; B1/B3.

## Карта файлов и контракты

Создать crate `crates/converter-download` с `src/{lib,model,checkpoint,cache,http,resume,transfer,progress,policy}.rs`. Production не использует внешнюю утилиту скачивания. Общий fixture server находится в отдельном dev-only crate `crates/converter-test-support/src/{lib,http}.rs`, а не в приватном tests-модуле загрузчика: им пользуются download, CLI и app integration tests. Сервер имеет сценарии `StableRange`, `DisconnectAt`, `IgnoreRange`, `ChangeEtag`, `BadRange`, `UnknownLength`, `Redirect`, `HtmlInstead`, `EncodedResponse`; tests/support загрузчика содержит только локальные cache helpers.

Публичные типы B1: `DownloadRequest { source_id: String, url: Url, expected_sha256: Option<String> }`, `DownloadPolicy { max_bytes: u64, checkpoint_bytes: u64, connect_timeout: Duration, idle_timeout: Duration, max_redirects: u32, retry_delays: Vec<Duration> }`, `CompletedDownload { source_id: String, path: PathBuf, display_name: String, bytes: u64, sha256: String }`.

`TransferState = Pending | Downloading | RetryWait | Paused | Verifying | Completed | Failed | Cancelled`. `DownloadProgress { source_id: String, state: TransferState, received_bytes: u64, confirmed_bytes: u64, total_bytes: Option<u64>, bytes_per_second: Option<f64>, restart_reason: Option<String> }`. Для группы файлов агрегат строит оболочка, не библиотека. В `CompletedDownload` нет URL; request/manifest никогда не получают производный `Debug`, печатающий токены.

`TransferControl` клонируется и имеет `pause()`, `cancel()`, `state()`; отмена/пауза сохраняют пригодную контрольную точку. `TransferOutcome = Completed(CompletedDownload) | Paused | Cancelled`. `DownloadManager::new(cache: DownloadCache, policy: DownloadPolicy) -> Result<Self>` и `async fetch(&self, request: DownloadRequest, control: TransferControl, progress: Arc<dyn Fn(DownloadProgress) + Send + Sync>) -> Result<TransferOutcome, DownloadError>`.

Конкретные стартовые значения policy: max_bytes 32 GiB на файл; checkpoint 1 MiB; connect timeout 15 s, idle timeout 30 s; максимум 10 redirects; три дополнительных повтора с задержками 2/5/15 s для сетевого обрыва, 408/429/5xx. Корректный `Retry-After` учитывается до 5 минут с возможностью паузы; превышение даёт приостановку с объяснением. Тесты используют управляемое время, не реальные минуты ожидания. Для fixture tests HTTP разрешён; HTTPS не отключает проверку сертификатов.

## Task B1: Долговременный кэш и проверяемые контрольные точки

**Files:** Create crate manifest/lib/model/checkpoint/cache; `tests/checkpoint_recovery.rs`, `tests/support/cache.rs`; Modify workspace.

**Interfaces:** Produces публичные модели выше, `DownloadCache::open(root: PathBuf) -> Result<Self>`, `load(source_id: &str) -> Result<Option<DownloadManifest>>`, `verify_prefix(source_id: &str) -> Result<ResumePoint>`, `commit_checkpoint(source_id: &str, checkpoint: Checkpoint) -> Result<()>`, `completed(source_id: &str) -> Result<Option<CompletedDownload>>`, `remove(source_id: &str) -> Result<()>`. `Checkpoint { confirmed_bytes, chunk_hashes }`; `ResumePoint { confirmed_bytes, validator, effective_url, total_bytes }`. URL и validator доступны только downloader, наружу не сериализуются.

- [ ] **Step 1:** В `восстанавливает_только_подтвержденный_префикс` сохранить порцию, записать checkpoint и дописать неподтверждённый хвост; пересоздать cache и проверить:

  ```rust
  assert_eq!(cache.verify_prefix(&id)?.confirmed_bytes, 1_048_576);
  assert_eq!(std::fs::metadata(part)?.len(), 1_048_576);
  assert!(cache.completed(&id)?.is_none());
  assert_eq!(tampered.unwrap_err().code, "checkpoint_hash_mismatch");
  ```

  Fault injection отдельно моделирует сбой до/после flush, до/после замены манифеста и ошибку записи при исчерпании места. Метаданные никогда не опережают сохранённые байты; неправильный source ID не даёт путь наружу.
- [ ] **Step 2:** Run `cargo test -p converter-download --test checkpoint_recovery`; ожидается FAIL.
- [ ] **Step 3:** Реализовать manifest version 1, `sync_data` перед checkpoint, атомарную replace-операцию для манифеста с отдельной Windows-проверкой. Хранить committed generation так, чтобы при сбое можно выбрать последнюю целую версию; не считать простой `rename` с overwrite одинаковым на всех ОС. Проверять размер и хеши порций потоково; файлы кэша называются source ID, не HTTP-именем. Отдельный completed marker устанавливать только после полной проверки.
- [ ] **Step 4:** Run Step 2 с `--locked`; PASS для всех crash points, hash mismatch, manifest version mismatch, очистки только собственного source ID и cache reopen. Защитить неполную загрузку от одновременных writers одного source ID.
- [ ] **Step 5:** Commit `feat: сохранить контрольные точки скачивания`.

## Task B2: HTTP-докачка и смена версии источника

**Files:** Create `src/{http,resume,policy}.rs`, `tests/http_resume.rs`, `crates/converter-test-support/{Cargo.toml,src/lib.rs,src/http.rs,examples/serve_fixture.rs}`; Modify workspace и dev-dependencies загрузчика.

**Interfaces:** Consumes B1 cache/models. Produces `plan_response(point: Option<&ResumePoint>, response: &ResponseMetadata) -> Result<ResponseAction>` с `ResponseAction = StartFresh | Append { offset: u64 } | Restart { reason: String } | Reject { code: String }`; `ResponseMetadata` включает status, effective URL, encoding, strong ETag, Content-Range и length. Это чистая функция без записи байтов. `http::open(request, point, policy)` возвращает проверенные metadata и stream для B3.

Test-only API: `FixtureServer::spawn(scenario: Scenario, payload: Vec<u8>) -> FixtureServer` асинхронно запускает сервер; `url(&self) -> String`, `requests(&self) -> Vec<RecordedRequest>`, `release(&self)` управляют адресом, журналом и удержанным ответом. `RecordedRequest { method: String, headers: BTreeMap<String, String>, range_start: Option<u64> }`. Example `serve_fixture` обслуживает тот же протокол для нативных smoke-сценариев, не входит в packaged app.

- [ ] **Step 1:** В `докачивает_неизменный_ресурс` fixture server обрывает файл после checkpoint и принимает продолжение; в `не_склеивает_версии` прогнать сценарии `200`, `416`, changed/weak/no ETag, wrong start/total, redirect to another final URL:

  ```rust
  assert_eq!(request_headers["range"], "bytes=1048576-");
  assert_eq!(request_headers["if-range"], "\"fixture-v1\"");
  assert_eq!(plan_response(Some(&point), &full_200)?, ResponseAction::StartFresh);
  assert!(!matches!(bad_range_action, ResponseAction::Append { .. }));
  ```

- [ ] **Step 2:** Run `cargo test -p converter-download --test http_resume`; ожидается FAIL.
- [ ] **Step 3:** Реализовать GET без обязательного HEAD, `Accept-Encoding: identity`, отключённые gzip/brotli/deflate/zstd features и rejected non-identity response. Привязать ResumePoint к конечному URI и сильному ETag; проверять арифметику Content-Range и лимит bytes до записи. Redirect policy ограничивает 10 переходов и запрещает HTTPS→HTTP. `200`/`416`/ненадёжный validator запускают новый файл с нулевого offset, не append. Сохранять пригодную старую генерацию до принятия новых headers.
- [ ] **Step 4:** Run Step 2 с `--locked`; PASS, включая HEAD=405, URL без расширения, query token, CORS-неразрешённый сервер, неожиданное HTTP encoding, redirect loop, timeout и ответ короче объявленного Content-Length. Проверить, что cookies/auth не заимствуются из браузера.
- [ ] **Step 5:** Commit `feat: добавить проверяемую HTTP-докачку`.

## Task B3: Управление передачей, завершение и отдельный прогресс

**Files:** Create `src/{transfer,progress}.rs`, `tests/{transfer_control,download_progress}.rs`; Modify public `lib.rs`.

**Interfaces:** Produces `DownloadManager::fetch` и control API из общей карты. Чистая функция `download_percent(received: u64, total: Option<u64>) -> Option<f64>` возвращает `None` при неизвестном/нулевом знаменателе, а не 0%/100%. Эмитируется прогресс одного файла; UI-throttle и групповой агрегат принадлежат C.

- [ ] **Step 1:** В `пауза_сохраняет_порцию_и_сбрасывает_скорость` остановить stream, reopen manager и продолжить. В `неизвестный_размер_не_дает_процент` и `проверяет_готовность_перед_completed` проверить:

  ```rust
  assert_eq!(download_percent(1234, None), None);
  assert_eq!(download_percent(50, Some(100)), Some(50.0));
  assert!(paused_progress.bytes_per_second.is_none());
  assert_eq!(completed.sha256, expected_hash);
  assert_eq!(bad_digest.unwrap_err().code, "download_digest_mismatch");
  ```

  Не переводить file в Completed при оборванном framing, превышении max_bytes, паузе во время SHA-проверки или ошибке записи. Самостоятельный SHA без эталона не обозначать как проверку подлинности.
- [ ] **Step 2:** Run `cargo test -p converter-download --test transfer_control --test download_progress`; ожидается FAIL.
- [ ] **Step 3:** Реализовать ограниченный потоковый буфер, checkpoint каждые 1 MiB и на управляемой паузе, возобновление с подтверждённого offset, полную SHA-проверку и completed marker. Скорость — измерение по окну последних 2 s с монотонными часами; на паузе/перезапуске окно очищается. Retry policy применяет 2/5/15 s без бесконечного цикла, control прерывает ожидание. При restart выставлять reason и новый начальный счётчик. Sanitized errors не печатают полный URL.
- [ ] **Step 4:** Run Step 2 с `--locked`; PASS с Tokio paused time/fake clock, включая 429 Retry-After, 403/истёкшую ссылку, 503 с исчерпанием попыток, cancel и повтор после успешного завершения. Последний случай проверяет локальный хеш и не выполняет новый GET.
- [ ] **Step 5:** Commit `feat: управлять скачиванием и его прогрессом`.

## Task B4: Рабочий URL → локальный источник → конвертация в CLI

**Files:** Create CLI `src/fetch.rs`, `tests/remote_convert.rs`, `docs/build/download.md`, `docs/validation/download.md`; Modify CLI manifest/main/convert и dev-dependency на `converter-test-support` из B2.

**Interfaces:** CLI `fetch --url URL --cache DIR [--sha256 HASH]` либо `fetch --source-id ID --cache DIR` для resume; stdout содержит только CompletedDownload/source ID, stderr — отдельный прогресс. `convert --url URL [--resources-url URL] --cache DIR --output DIR` сначала получает все удалённые файлы, отображает их итоги, затем вызывает A `prepare_source/convert_to_staging/publish_local`. `--source` и `--url` взаимоисключающие. Преобразование `CompletedDownload → PreparedFile` выполняет CLI, не downloader.

- [ ] **Step 1:** В `не_начинает_конвертацию_до_получения_ресурсов` server удерживает второй файл; первый готов. После отключения сети источник повторно конвертируется из cache. Проверить:

  ```rust
  assert!(conversion_events_before_second_file.is_empty());
  assert_eq!(converted_resource_hash, original_resource_hash);
  assert_eq!(get_count_after_cached_retry, get_count_before_cached_retry);
  assert!(!public_output.contains("secret-token"));
  ```

- [ ] **Step 2:** Run `cargo test -p converter-cli --test remote_convert`; ожидается FAIL.
- [ ] **Step 3:** Реализовать CLI связывание без копирования сетевой логики, Ctrl-C → pause/cancel с пригодным `.part`, sniff HTML vs input bundle после завершения. ZIP с несколькими DSL обрабатывается по правилам A2; при неоднозначности CLI завершает preflight с понятной диагностикой, GUI позднее предложит выбор.
- [ ] **Step 4:** Run `cargo test --locked -p converter-download -p converter-cli`; PASS. В `docs/validation/download.md` записать размер источника, прерванный/возобновлённый offset, исходный/итоговый SHA, фактические HTTP-запросы тестового сервера и успешное чтение результата. Два прогресса в CLI представлены разными phase-полями, не одной шкалой.
- [ ] **Step 5:** Commit `feat: конвертировать словари по ссылке через CLI`.

После B4 готов результат B общего плана. Следующая часть — [настольное приложение C](2026-10-18-converter-desktop.md).
