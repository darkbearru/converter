# Converter Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Получить проверяемый CLI и независимую Rust-библиотеку, конвертирующие локальные DSL/DZ/ZIP в совместимые MDX/MDD без загрузки всей базы в RAM.

**Architecture:** Подготовка источников создаёт локальный `InputBundle`; реестр выбирает `DictionaryReader`. DSL-модуль последовательно отдаёт статьи в дисковый spool, внешняя сортировка разворачивает ключи, общий writer пишет блоки MDict. Публикация готовой директории отделена от создания файлов.

**Tech Stack:** Rust stable; serde, thiserror, encoding_rs, flate2 с `rust_backend`, ZIP с deflate, SHA-256; mdictlib только как независимый тестовый ридер; CLI для разработчика.

**Spec:** [Спецификация](../specs/2026-10-18-dsl-mdx-converter-design.md), разделы 4–8, 11. [Общий план и ограничения](2026-10-18-converter-implementation.md).

## Global Constraints

- Все ограничения общего плана обязательны; ядро не зависит от Tauri и сети.
- Незашифрованные MDict v2 `.mdx`/`.mdd`, zlib; `converter.css` включён в MDD.
- Начальный бюджет буфера сортировки — 64 MiB на macOS/Windows и 16 MiB на мобильных платформах.
- Декодированное тело статьи и итоговый HTML по отдельности ограничены 8 MiB в UTF-8.
- Русские ошибки, комментарии и названия тестов; технические идентификаторы — на английском.
- Исходники словарей не изменяются; дублирующиеся статьи не перезаписываются.

## Review Focus

1. Один ресурс пересекает много блоков: извлечение даёт исходный SHA-256; A1/A5.
2. `~`, экранирование и UTF-16 разделены границей буфера: результат совпадает с чтением целого текста; A3/A4.
3. ZIP содержит `CON`, `AUX`, `a\\b`, `../x` и Unicode-коллизии: корректные логические имена сохраняются, неоднозначные/опасные отклоняются; A2.
4. Ключи с пунктуацией, регистром, CJK и дубликатами: независимый точечный поиск работает на границах блоков; A1/A5.
5. Отмена или ошибка публикации после готовности MDX: не появляется половина результата, исходники целы; A6.

## Файловая структура и общие контракты

Создать `Cargo.toml` workspace, `rust-toolchain.toml`, `.gitignore`, crate `crates/converter-core`, crate `crates/converter-cli`. При создании каждого crate добавлять его member; не ссылаться на ещё не существующие каталоги. Модульные каталоги: `model/`, `prepare/`, `formats/`, `dsl/`, `mdict/`, `pipeline/`; в каждом `mod.rs` только публичные exports и связывание.

В A1 определить следующие публичные типы в `model/`: `PreparedFile { id: String, display_name: String, path: PathBuf }`, `InputBundle { main: PreparedFile, abbreviations: Option<PreparedFile>, resources: Vec<ResourceRecord> }`, `ResourceRecord { key: String, path: PathBuf }`, `DictionaryMetadata { title, description, index_language, contents_language, source_encoding: String }`, `DictionaryEntry { id: u64, source_line: u64, parent_id: Option<u64>, headwords: Vec<String>, display_headword: String, html: String, resource_keys: Vec<String> }`, `MdxRecord { key: String, html: String }`, `Diagnostic`, `ReadOptions`, `Limits`, `WriteSummary`, `ConversionSummary`, `ConvertedArtifacts`, `ConversionRequest`.

`ConversionRequest { bundle: InputBundle, options: ReadOptions, metadata_override: Option<DictionaryMetadata>, staging_dir: PathBuf }`; `ConvertedArtifacts { directory: PathBuf, mdx: PathBuf, mdd: PathBuf, report: PathBuf, summary: ConversionSummary }`. Единый alias `Result<T>` использует `ConverterError { code, message, source_location }`; коды стабильны, сообщения русские. `ReadOptions` содержит выбранную кодировку либо `Auto`.

`WriteSummary { entries: u64, key_blocks: u64, record_blocks: u64, total_bytes: u64 }`; `ConversionSummary { source_articles: u64, output_keys: u64, resources: u64, warnings: u64, stages: Vec<StageProgress>, sort_metrics: SortMetrics }`; `SortMetrics { merge_passes: u32, peak_sort_bytes: usize }`. В tests переменная `metrics` — `artifacts.summary.sort_metrics`, а не отдельный неописанный источник измерений.

`RunContext { scratch_dir: PathBuf, limits: Limits, control: RunControl, progress: Arc<dyn Fn(CoreEvent) + Send + Sync> }`; `RunControl` — клонируемый сигнал отмены, `cancel(&self)`, `check(&self) -> Result<()>`. `CoreEvent` передаёт `StageProgress { stage: ConversionStage, completed: u64, total: Option<u64>, unit: ProgressUnit, articles: u64, resources: u64 }`, диагностику или завершение этапа. Этапы: `Inspect`, `Read`, `Sort`, `WriteMdx`, `WriteMdd`, `Validate`, `Publish`; единицы: `Bytes`, `Entries`, `Resources`. Идентификаторы запуска присваивает оболочка, не ядро.

Начальные защитные значения `Limits`: 64/16 MiB sort buffer по профилю, 8 MiB тело и HTML, 64 варианта заголовка, 100_000 элементов ZIP, 32 GiB распакованных данных, 1_000:1 коэффициент распаковки, 32 входных run-файла на проход объединения, 1 MiB буфер копирования ресурса. Это конечные конфигурируемые лимиты; превышение — диагностическая ошибка, без потери части базы. Суммарный RSS измеряется отдельно.

## Task 1: A1 — Совместимая блочная запись MDX/MDD и контракты ядра

**Files:** Create `Cargo.toml`, `rust-toolchain.toml`, `.gitignore`, `crates/converter-core/{Cargo.toml,src/lib.rs}`, `src/model/{mod,error,entry,limits,progress,request}.rs`, `src/mdict/{mod,header,collation,key_blocks,record_blocks,writer}.rs`; Test `tests/mdict_conformance.rs`, `tests/support/{mod,mdict_reader,context}.rs` внутри core crate.

**Interfaces:** Produces типы выше; `write_mdx<W: Write + Seek>(records: impl Iterator<Item = Result<MdxRecord>>, metadata: &DictionaryMetadata, out: W, ctx: &RunContext) -> Result<WriteSummary>`; `write_mdd<W: Write + Seek>(resources: impl Iterator<Item = Result<ResourceRecord>>, out: W, ctx: &RunContext) -> Result<WriteSummary>`. Входы уже отсортированы; writer проверяет порядок, хранит сводки на диске. Test helpers `read_mdx(&Path) -> Vec<(String, String)>`, `read_mdd(&Path) -> Vec<(String, Vec<u8>)>` используют mdictlib с проверкой checksum, не собственный parser; `context(&Path) -> RunContext` создаёт конечные тестовые лимиты.

- [x] **Step 1:** Использовать глобальный stable Rust, обновлённый по прямому запросу пользователя; определить точную версию через `rustup run stable rustc --version` и записать её в `rust-toolchain.toml` (проверенная версия — `1.99.1`). Создать минимальный workspace и conformance-тест до writer. Проверить MSRV mdictlib и Tauri, сохранить `Cargo.lock`. Повторное обновление глобального stable не требуется; точная версия в rust-toolchain.toml может вызвать установку одноимённого versioned toolchain, при этом глобальный default остаётся stable.
- [x] **Step 2:** В `пишет_совместимые_блоки_и_дубликаты` сформировать отсортированные `MdxRecord` с `a-b`, `ab`, двумя `яблоко` и CJK; принудительно малые блоки. Assert:

  ```rust
  assert_eq!(read_mdx(&mdx).iter().filter(|(k, _)| k == "яблоко").count(), 2);
  assert_eq!(read_mdd(&mdd)[0].1, original_resource);
  assert_eq!(summary.entries, input_records as u64);
  ```

  Дополнительно независимым `MdxFile::lookup` проверить первую/последнюю запись, пунктуацию и границы блоков; `MddFile::lookup_span().copy_to()` сверить хеш ресурса больше блока. Сигнатуры helpers реализовать в test support через независимую библиотеку.
- [x] **Step 3:** Run `cargo test -p converter-core --test mdict_conformance`; ожидается FAIL из-за отсутствующего writer/экспортов, затем реализовать только контракты и writer. UTF-16LE XML header, 64-битная v2 геометрия, Adler-32 и zlib; компаратор ровно из спецификации. Сводки ключей/записей spool на диске; ресурс допускает пересечение record blocks. Проверить отсортированность и переполнение размеров.
- [x] **Step 4:** Run `cargo test --locked -p converter-core --test mdict_conformance`; ожидается PASS, включая malformed order, NUL key, отмену и `Write + Seek`-приёмник, возвращающий ошибку записи. `cargo fmt --all -- --check` проходит.
- [x] **Step 5:** Commit только workspace, model/writer и их тесты: `feat: добавить потоковую запись MDX и MDD`.

## Task 2: A2 — Подготовка локального DSL/DZ/ZIP и переносимых ресурсов

**Files:** Create core `src/prepare/{mod,bundle,archive,resource_paths}.rs`, `tests/source_preparation.rs`, `tests/fixtures/README.md`; Test fixtures генерируются тестом в `tempfile`, небольшие исходные DSL разрешены в Git.

**Interfaces:** Consumes `PreparedFile`, `InputBundle`, `ResourceRecord`, `RunContext`. Produces `prepare_source(main: &PreparedFile, attachments: &[PreparedFile], ctx: &RunContext) -> Result<Vec<InputBundle>>` и `resolve_resource(bundle: &InputBundle, logical_key: &str) -> Result<Option<ResourceRecord>>`. Несколько bundles означают несколько основных словарей; `_abrv.dsl` привязывается, а не становится обычным заданием.

- [x] **Step 1:** В `архив_сохраняет_логические_имена` создать ZIP с двумя DSL, `_abrv.dsl`, вложенным PNG и ресурсом `AUX.png`. В `не_теряет_коллизии_и_не_выходит_из_архива` добавить `../x`, drive/UNC пути, symlink и неоднозначные NFC/case варианты. Assert:

  ```rust
  assert_eq!(bundles.len(), 2);
  assert!(resolve_resource(&bundles[0], "AUX.png")?.is_some());
  assert_eq!(unsafe_archive.unwrap_err().code, "unsafe_archive_path");
  assert_eq!(ambiguous.unwrap_err().code, "ambiguous_resource");
  ```

- [x] **Step 2:** Run `cargo test -p converter-core --test source_preparation`; ожидается FAIL на отсутствующем prepare API.
- [x] **Step 3:** Реализовать виртуальные логические пути и физические файлы с внутренними ID, явную привязку attachments, лимиты до/во время извлечения и hash-сравнение коллизий. Проверять пути обоих синтаксисов на всех ОС. Для папок сохранять доступ только к разрешённому корню; не следовать наружным ссылкам. Nested/encrypted ZIP возвращает поддерживаемую диагностическую ошибку.
- [x] **Step 4:** Run `cargo test --locked -p converter-core --test source_preparation`; PASS также для лимитов, отсутствующего ресурса, единственного case fallback с предупреждением и отмены. Проверить неизменность хешей исходников.
- [x] **Step 5:** Commit `feat: подготовить локальные словари и ZIP-ресурсы`.

## Task 3: A3 — Реестр форматов, кодировки и структурный DSL-парсер

**Files:** Create core `src/formats/{mod,reader,registry}.rs`, `src/dsl/{mod,decoder,lexer,parser,metadata}.rs`, `tests/{reader_contract,dsl_structure}.rs`, `tests/support/collect_sink.rs`.

**Interfaces:** Produces `DictionaryReader: Send + Sync` с `descriptor(&self) -> FormatDescriptor`, `probe(&self, prefix: &[u8], name: &str) -> ProbeResult`, `inspect(&self, bundle: &InputBundle, options: &ReadOptions, ctx: &RunContext) -> Result<DictionaryMetadata>`, `read(&self, bundle: &InputBundle, options: &ReadOptions, sink: &mut dyn EntrySink, ctx: &RunContext) -> Result<ReadSummary>`. `EntrySink::push(&mut self, DictionaryEntry) -> Result<()>` и `diagnostic(&mut self, Diagnostic) -> Result<()>`. `FormatRegistry::{builtins, descriptors, detect, get}`; `detect` возвращает ID либо неоднозначность. `resolve_encoding(main: &PreparedFile, choice: EncodingChoice, ctx: &RunContext) -> Result<ResolvedEncoding>` фиксирует кодировку до семантического read. `CollectSink` только в tests хранит `entries`/`diagnostics`.

`FormatDescriptor { id: String, name: String, extensions: Vec<String>, supports_resources: bool }`; `ProbeResult { confidence: u8 }` в диапазоне 0–100; `ReadSummary { source_articles: u64, warnings: u64 }`. Точные методы реестра: `builtins() -> Self`, `descriptors(&self) -> Vec<FormatDescriptor>`, `detect(&self, prefix: &[u8], name: &str) -> Result<String>`, `get(&self, id: &str) -> Result<&dyn DictionaryReader>`; равная ненулевая уверенность возвращает `ambiguous_format`, нулевая — `unsupported_format`.

- [x] **Step 1:** В `разбирает_кодировки_и_границы_буфера` прогнать одинаковый словарь в UTF-8/BOM, UTF-16LE/BE/BOM, Windows-1251 и DZ с чтением порциями 1–7 байтов. Проверить CRLF на границе порции, BOM только в начале, комментарии, два заголовка, подстатью и ошибку декодирования:

  ```rust
  assert_eq!(sink.entries[0].headwords, ["яблоко", "apple"]);
  assert_eq!(sink.entries[1].parent_id, Some(sink.entries[0].id));
  assert_eq!(invalid.unwrap_err().code, "invalid_encoding");
  assert_eq!(metadata.title, "Тестовый словарь");
  ```

  В `реестр_не_требует_изменения_pipeline` зарегистрировать тестовый reader с другим ID и проверить его обнаружение без изменений DSL/writer. Дополнительно Windows-1251 со строго ASCII-префиксом больше 64 KiB не должна ошибочно фиксироваться как UTF-8; UTF-16 без BOM запрашивает ручную кодировку.
- [x] **Step 2:** Run `cargo test -p converter-core --test dsl_structure --test reader_contract`; ожидается FAIL.
- [x] **Step 3:** Реализовать декодер с сохранением неполного символа между порциями, потоковый `MultiGzDecoder` для DZ, лексер со строками и парсер границ статей. Ограничить probe 64 KiB, сохранять корректные номера строк. Для Auto без BOM выполнить отдельную потоковую проверку всего локального/decompressed источника на UTF-8 до выбора Windows-1251; не фиксировать кодировку по одному префиксу и не менять её посреди статьи. UTF-16 без BOM определяется как требующий ручного выбора. `inspect` не удерживает базу в памяти, показывает байтовый прогресс проверки кодировки; read строго проверяет выбранную кодировку. Структурная ошибка не пропускает статью. Рендеринг подключается в A4 через тот же `DslReader`.
- [x] **Step 4:** Run команды Step 2 с `--locked`; PASS, включая отмену, `_abrv.dsl`, пустой словарь и неправильный формат без расширения. Список builtins содержит только DSL, test reader остаётся test-only.
- [x] **Step 5:** Commit `feat: добавить расширяемый читатель DSL и кодировки`.

## Task 4: A4 — Форматирование, варианты ключей, ссылки и ресурсы DSL

**Files:** Create core `src/dsl/{headwords,renderer,references,abbreviations}.rs`, `assets/converter.css`, `tests/{dsl_rendering,dsl_headwords}.rs`; Modify `src/dsl/parser.rs`, `src/dsl/mod.rs`.

**Interfaces:** Consumes `DslReader`, `EntrySink`, `resolve_resource`. Produces `expand_headwords(raw: &str, limit: usize) -> Result<Vec<String>>`, `render_article(ast: &ArticleAst, bundle: &InputBundle, options: &ReadOptions) -> Result<RenderedArticle>`, `RenderedArticle { html, resource_keys, diagnostics }`; `ArticleAst` и `HeadwordAst` принадлежат DSL-модулю, не общему writer. `DictionaryEntry` остаётся общей границей.

- [x] **Step 1:** В `сохраняет_семантику_и_экранирует_текст` проверить все теги из §4.1 спецификации, вложенность, `~`, экранирование, `[ref]`/`<<…>>`, сокращения, PNG/аудио, пропавший ресурс и неизвестный тег. В `разворачивает_варианты_без_потери_дубликатов` проверить несортируемые/необязательные части и ограничение 64 вариантов:

  ```rust
  assert!(!rendered.html.contains("<script>"));
  assert!(rendered.resource_keys.contains(&"media/voice.mp3".to_owned()));
  assert_eq!(expand_headwords("colo(u)r", 64)?, ["color", "colour"]);
  assert_eq!(too_many.unwrap_err().code, "headword_variants_limit");
  ```

  Проверить, что экранированные скобки не превращаются в варианты/теги; русский текст неизвестного тега виден, есть diagnostic.
- [x] **Step 2:** Run `cargo test -p converter-core --test dsl_rendering --test dsl_headwords`; ожидается FAIL.
- [x] **Step 3:** Реализовать AST-renderer, а не глобальные regex-замены. Внутренние ссылки — `entry://`, звук — `sound://`, изображения/CSS — согласованные MDD keys. Разрешённый URL-тег ограничен HTTP/HTTPS и безопасным текстом, JS/event-атрибуты не генерируются. Подстановку `~` выполнять по каноническому отображаемому заголовку; сохранять parent ID и исходный порядок. Каждый готовый HTML проверять на 8 MiB. Подключить стандартный CSS через MDD.
- [x] **Step 4:** Run Step 2 с `--locked`; PASS, включая незакрытый тег, Unicode, вложенные необязательные части и отсутствующее сокращение. Семантические assertions проверяют контент/ссылки, не случайное форматирование HTML.
- [x] **Step 5:** Commit `feat: сохранить разметку и ресурсы DSL`.

## Task 5: A5 — Дисковый pipeline, сортировка и измеримый прогресс этапов

**Files:** Create core `src/pipeline/{mod,spool,sort,link_index,convert,validate,report}.rs`, `tests/{conversion_pipeline,large_dictionary}.rs`, `examples/generate_large_fixture.rs`; Modify writer progress hooks.

**Interfaces:** Produces `convert_to_staging(request: &ConversionRequest, ctx: &RunContext) -> Result<ConvertedArtifacts>`; `EntrySpool: EntrySink`, `ExternalSort::finish(&mut self) -> Result<impl Iterator<Item = Result<MdxRecord>>>`; сигнатура сортировочного iterator внутри модуля может быть именованным `SortedRecords`, публичный pipeline не раскрывает её. `validate_artifacts(&ConvertedArtifacts, &RunContext) -> Result<()>` проверяет структуру/checksum/счётчики без удержания всех записей; тестовая независимая проверка по-прежнему использует mdictlib. В `ConversionSummary` отдельны `source_articles`, `output_keys`, `resources`, `warnings` и финальные счётчики этапов.

- [x] **Step 1:** В `конвертирует_через_несколько_проходов` задать sort budget 4 KiB и merge fan-in 2 для небольшого теста, много дублей и ссылок; событие записать test sink:

  ```rust
  assert!(metrics.merge_passes >= 2);
  assert!(metrics.peak_sort_bytes <= ctx.limits.sort_buffer_bytes);
  assert_eq!(artifacts.summary.source_articles, expected_articles);
  assert_eq!(read_mdx(&artifacts.mdx).len() as u64, expected_keys);
  assert!(events.iter().all(|e| e.total.map_or(true, |n| e.completed <= n)));
  ```

  Дополнительно MDD-resource больше 1 MiB и большое число блоков: хеш через независимый span равен входному; нет materialize полного ресурса. Для неизвестного числа статей `Read.total` относится к байтам, не выдуманным статьям.
- [x] **Step 2:** Run `cargo test -p converter-core --test conversion_pipeline`; ожидается FAIL.
- [x] **Step 3:** Реализовать disk spool, bounded sort runs и multi-pass merge с fan-in 32 по умолчанию, стабильным source ordinal при равных sort keys. Индекс ссылок и сводки блоков держать на диске; emit `StageProgress` по известной единице этапа. Проверять отмену между порциями. На missing reference добавлять diagnostic. `converter.css` создаётся как собственный ресурс; отчёт пишется потоково. Счётчики проверять до публикации.
- [x] **Step 4:** Run тест Step 2 и `cargo test --locked -p converter-core --test large_dictionary -- --ignored --nocapture`; первый PASS, второй генерирует вход больше 64 MiB budget вне Git, фиксирует размер, число статей, время и peak RSS. Большой тест пометить ignored, обычный маленький multi-pass остаётся в CI. Измерение RSS не заменять внутренним sort budget.
- [x] **Step 5:** Commit `feat: конвертировать большие базы с дисковой сортировкой`.

## Task 6: A6 — CLI, публикация результата и независимое открытие

**Files:** Create `crates/converter-cli/{Cargo.toml,src/main.rs,src/convert.rs,tests/local_convert.rs}`, core `src/pipeline/publication.rs`, `docs/build/core.md`, `docs/validation/core.md`; Modify root workspace.

**Interfaces:** `publish_local(artifacts: ConvertedArtifacts, destination: &Path, basename: &str, ctx: &RunContext) -> Result<PathBuf>` публикует только проверенную директорию на том же томе, выбирает `<basename>-converted[-N]`. CLI: `converter-cli convert --source PATH [--resources PATH] --output DIR [--encoding auto|utf8|utf16le|utf16be|windows1251] [--profile desktop|mobile]`; stdin/сеть здесь не нужны. Код выхода 0 — готово, 1 — ошибка, 130 — отмена; предупреждения дают 0 и явный summary.

- [x] **Step 1:** В `cli_не_перезаписывает_результат` дважды запустить fixture, проверить две разные директории и SHA исходника. В `отмена_не_публикует_половину` прервать между записью MDX/MDD и перед публикацией:

  ```rust
  assert_ne!(first_directory, second_directory);
  assert_eq!(source_hash_before, source_hash_after);
  assert!(!cancelled_result_directory.exists());
  ```

- [x] **Step 2:** Run `cargo test -p converter-cli --test local_convert`; ожидается FAIL, затем реализовать CLI, reuse `prepare_source`, `convert_to_staging`, `publish_local`. Во время работы CLI пишет прогресс этапов в stderr, финальный JSON summary — в stdout; на Ctrl-C передаёт `RunControl::cancel`.
- [x] **Step 3:** Run `cargo test --locked -p converter-core -p converter-cli`; PASS. Run `cargo run --locked -p converter-cli -- convert --source <контролируемый DSL> --output <новая папка>`; ожидаются `.mdx`, `.mdd`, отчёт без runtime PHP/Python.
- [x] **Step 4:** Открыть полученную пару файлов в доступном независимом приложении MDict. Проверить поиск, оформление, переход, картинку и звук; в `docs/validation/core.md` сохранить программу/версию и фактические результаты. Если приложения нет, явно оставить эту проверку незавершённой, не считать same-library roundtrip её заменой. Добавить реальный DSL пользователя при наличии.
- [x] **Step 5:** Commit `feat: добавить CLI и безопасную публикацию результата`.

После A6 выполнен результат A общего плана. Следующая часть — [загрузчик B](2026-10-18-converter-download.md); продуктовый UI здесь ещё не считается реализованным.
