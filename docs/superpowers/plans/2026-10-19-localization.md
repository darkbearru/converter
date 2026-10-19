# Локализация Converter: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking. Предлагаемый способ — выполнение в текущем чате; сначала пользователь согласует этот план.

**Goal:** Русский и английский интерфейсы приложения и сайта с автоматическим выбором основного языка ОС и отдельными ссылками поддержки.

**Architecture:** Vue I18n и реактивная locale PrimeVue, общие JSON-каталоги приложения для Vue/Tauri, структурированные сообщения Rust с совместимостью старых журналов. Нативные настройки языка хранятся отдельно от задач; сайт генерирует два статических варианта из одних данных релиза.

**Tech Stack:** Vue 3, TypeScript, Vue I18n, установленный PrimeVue 4, Tauri 2/Rust, sys-locale, Node.js, GitHub Actions/Pages, Vitest, Playwright.

**Spec:** [2026-10-19-localization-design.md](../specs/2026-10-19-localization-design.md), проект для согласования вместе с планом.

## Global Constraints

- Первые языки: русский (`ru`) и английский (`en`).
- При автоматическом выборе использовать основной язык системы. Если он поддерживается, выбрать его; иначе выбрать английский. Вторичные предпочтительные языки не подменяют основной.
- Русская ссылка поддержки: `https://pay.cloudtips.ru/p/7e782291`. Английский сервис пока не выбран: английские элементы поддержки скрыты.
- Целевые сборки: macOS Apple Silicon и Windows x64. Intel macOS и мобильные сборки в эту работу не входят.
- Сначала согласовать план, затем приступать к реализации.
- Русские единицы сохраняются: `байт`, `Кб`, `Мб`, `Гб`, `Тб`; английские: `bytes`, `KB`, `MB`, `GB`, `TB`.
- Содержимое словарей и алгоритмы конвертации не зависят от языка интерфейса; старые журналы читаются без переписывания.
- Никаких запросов перевода, геолокации или новых сетевых проверок при переключении языка.
- Не менять minor/major без прямого указания; сохранить текущую автоматическую смену patch-версии при сборке.
- Не включать `.idea/` и посторонние пользовательские файлы в коммиты.

## Review Focus

- Основной `de-DE` при вторичном `ru-RU` даёт английский, а не русский: тест выбора в задаче 1.
- Повреждённые настройки и отказ записи не блокируют запуск и не создают ложного сообщения об успешном сохранении: тесты задачи 1.
- Переключение языка во время конвертации и при открытом диалоге сохраняет задачи, фокус и введённые значения: сценарий задачи 3.
- Параметры диагностики с `{}`, кавычками и HTML не становятся шаблоном или разметкой; группы не зависят от текста перевода: задачи 2 и 3.
- Вложенные URL сайта корректно загружают стили/изображения и скачивают именно установщики; отсутствие EN-доната не оставляет пустой элемент: задача 4.

## Общие данные и границы файлов

Новые `config/locales.json` — только метаданные языков, fallback `en` и `support_url`; `locales/ru.json` и `locales/en.json` — одинаковые деревья ключей `app`, `primevue`, `native`, `messages`, `issues`. Vue импортирует каталоги, Tauri включает те же файлы в бинарник. Приложение не читает файлы переводов с сайта.

`website/locales/ru.json` и `website/locales/en.json` содержат тексты сайта. `website/index.html` остаётся общим шаблоном. `scripts/pages/build.mjs` использует общую конфигурацию языков, каталоги сайта и существующий JSON релиза.

Новые поля Rust/TypeScript: `message_key: string | null`, `message_args: Record<string, string | number>`. В чтении старого JSON оба поля необязательны; Rust использует `#[serde(default)]`. Существующие `code` и `message` обязательны и сохраняются. Для причины повторного скачивания добавить необязательное `restart_message` с теми же полями и `message`, оставив `restart_reason` для совместимости.

### Задача 1. Язык приложения, сохранение и общая конфигурация

**Files:**
- Create: `config/locales.json`, `locales/ru.json`, `locales/en.json`.
- Create: `src/shared/i18n/index.ts`, `src/shared/i18n/primevue.ts`, `src/shared/i18n/preferences.ts`.
- Create: `src-tauri/src/preferences.rs`, `src-tauri/src/localization.rs`.
- Modify: `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `Cargo.lock`.
- Modify: `src/main.ts`, `src/shared/theme.ts`, `src/shared/contracts.ts`, `src/shared/native.ts`, `src-tauri/src/lib.rs`, `src-tauri/src/commands.rs`.
- Test: `tests/ui/localization.test.ts`, тестовые модули новых Rust-файлов, `tests/ui/setup.ts`, `tests/ui/harness/bridge.ts`, `tests/ui/harness/main.ts`.

**Interfaces:**
- `Locale = 'ru' | 'en'`; `LanguagePreference = 'system' | Locale`.
- `AppPreferences = { language: LanguagePreference; locale: Locale; support_url: string | null }`.
- Bridge: `getAppPreferences(): Promise<AppPreferences>`, `setAppLanguage(language: LanguagePreference): Promise<AppPreferences>`; Tauri-команды `get_app_preferences`, `set_app_language`.
- Rust: `resolve_locale(language: LanguagePreference, system_locale: Option<&str>) -> Locale` использует только основной тег ОС.
- Frontend: `createLocalization(initial: AppPreferences, bridge: NativeBridge)` создаёт отдельный экземпляр Vue I18n и реактивные настройки; его `setLanguage(language: LanguagePreference): Promise<void>` сохраняет выбор через bridge и затем применяет возвращённые настройки.

- [x] Написать проверки: `resolve_locale(system, ru-RU) == ru`, `en-US == en`, `de-DE == en`, `None == en`; ручной `ru` на английской ОС остаётся `ru`. Вторичный язык не передаётся в выбор.
- [x] Добавить проверки восстановления сохранённого выбора, повреждённого файла и ошибки атомарной записи. Ошибка записи не изменяет подтверждённый выбор в интерфейсе.
- [x] Запустить новые тесты и убедиться, что они падают из-за отсутствующей реализации.
- [x] Добавить Vue I18n совместимой стабильной версии, зафиксировать lockfile; добавить `sys-locale`. Реализовать загрузку настроек до создания нативного меню и до первого отображения Vue. Файл: `app_config_dir/preferences.json`; повреждённый файл сохранять как `preferences.invalid-<timestamp>.json` перед заменой.
- [x] Подключить локальные каталоги с fallback `en`; реактивно обновлять locale PrimeVue. Изолировать экземпляры i18n в тестах. Harness поддерживает `lang=ru|en` и имитацию системной локали без доступа к ОС.
- [x] Проверить: `npm test -- --run tests/ui/localization.test.ts`, `cargo test -p converter-desktop`, `npm run typecheck`. Ожидается успешное прохождение.
- [x] После успешной проверки зафиксировать этап коммитом `feat: add persistent application locale` при выполнении согласованного плана.

### Задача 2. Структурированные ошибки и предупреждения

**Files:**
- Modify: `crates/converter-core/src/model/error.rs`, `crates/converter-core/src/pipeline/report.rs` и места создания сообщений в `dsl/`, `prepare/`, `mdict/`, `pipeline/`, `formats/`.
- Modify: `crates/converter-download/src/model.rs`, `transfer.rs`, `resume.rs`, `http.rs`, `policy.rs`, `cache.rs`, `checkpoint.rs`, `progress.rs`.
- Modify: `crates/converter-app/src/contracts.rs`, `jobs/`, `storage/`, места создания AppError в `src-tauri/src/`.
- Modify: `src/shared/contracts.ts`, `locales/ru.json`, `locales/en.json`, `scripts/check-contracts.mjs`, `tests/ui/fixtures/contracts.json`.
- Create: `src/shared/i18n/messages.ts`, `crates/converter-core/tests/localized_messages.rs`.
- Test: существующие `diagnostic_policy.rs`, `dsl_service_tags.rs`, `conversion_pipeline.rs`, `crates/converter-download/tests/`, `crates/converter-app/tests/`.

**Interfaces:**
- Rust-ошибки и Diagnostic сохраняют `message_key` и отсортированную карту скалярных `message_args`; конвертация в AppError переносит оба поля без потерь.
- `formatMessage(value: { message: string; message_key?: string | null; message_args?: Record<string, string | number> }): string` возвращает перевод ключа или исходный `message`, если метаданных нет/ключ неизвестен.
- Поле `restart_message` опционально и проходит через снимки/события; при его отсутствии интерфейс использует старое `restart_reason`.

- [x] Написать тесты старого JSON без новых полей, сохранения параметров при преобразовании ошибок, неизвестного ключа и параметров `"<b>{name}</b>"`. Параметры должны оставаться буквальным текстом.
- [x] Закрепить группировку: одинаковый отсутствующий ресурс с разным переводом даёт одну группу в пределах источника; разные источники остаются разными группами. Сохраняются текущая нормализация путей/NFC и отдельные предупреждения для неизвестных тегов.
- [x] Запустить новые тесты и подтвердить ожидаемые падения.
- [x] Добавить ключи и параметры во все принадлежащие приложению сообщения, видимые в desktop; не выводить перевод из `code` или регулярных выражений. Существующий `message` оставить совместимым fallback для JSON и CLI.
- [x] Перевести группировку новых диагностик на ключ/параметры; убрать зависимость от русского префикса `Отсутствует ресурс:`. Старые сохранённые журналы не перегруппировывать.
- [x] Сохранить обработку журналов на диске и постраничное чтение; не загружать весь список предупреждений ради смены языка.
- [x] Проверить: `cargo test -p converter-core -p converter-download -p converter-app`, `npm run check:contracts`, `npm test -- --run tests/ui/localization.test.ts`. Ожидаются прежние счётчики и успешное чтение старых контрактов.
- [x] Зафиксировать этап коммитом `feat: add locale independent diagnostic metadata`.

### Задача 3. Перевод интерфейса и нативных действий

**Files:**
- Modify: `src/App.vue`, `src/features/composer/`, `jobs/`, `sources/`, `results/`, `progress/`, `updates/AppDialogs.vue`.
- Modify: `src/shared/format.ts`, `src/shared/native.ts`, `src/shared/ui/ProblemReportButton.vue`, `src/shared/ui/dialog.ts`, `src/styles/app.css` только для размещения переключателя и подтверждённых переполнений.
- Modify: `src-tauri/src/app_menu.rs`, `storage/picker.rs`, `commands.rs`, `issue_report.rs`, `updates.rs`, `localization.rs`.
- Modify: `locales/ru.json`, `locales/en.json`, `tests/ui/setup.ts`, существующие UI-тесты, `tests/ui/harness/bridge.ts`.
- Create: `tests/ui/e2e/localization.spec.ts`, `scripts/tests/locales.test.mjs`.

**Interfaces:**
- Компоненты используют `useI18n()`; состояние хранит коды/ключи/параметры, а не заранее переведённые строки. Это относится и к ошибкам действия в store/диалогах.
- В `format.ts` заменить фиксированные таблицы на `stageLabel(stage)`, `stateLabel(state)`, `transferLabel(state)`; `formatCount`, `formatBytes`, `stageCounter`, `errorMessage` используют текущую locale. Количества формировать из ключей каталога и числа, без передачи русских форм из компонентов.
- Rust: `native_text(locale, key, args)` форматирует только собственные строки каталогов `native`, `messages`, `issues`; параметры не исполняются как шаблон повторно.
- Существующие `openSupportPage()` и `openIssue(id?)` не меняют сигнатуру: native читает подтверждённую locale из состояния. При `support_url == null` команда возвращает структурированное `support_unavailable`, не открывая браузер.

- [x] Добавить проверки всех ключей и плейсхолдеров RU/EN, склонения `1, 2, 5, 11, 21`, английских singular/plural, размеров и целых процентов. Проверять отсутствие русского интерфейсного текста в EN на синтетических данных; кириллица внутри словаря допустима.
- [x] Добавить сценарий: открыть диалог и ввести ссылку, переключить язык, сохранить ввод и активную задачу; новые диагностические тексты переключаются, количество и progress остаются прежними. Проверить отдельно старую запись без ключей.
- [x] Запустить новые сценарии и подтвердить падение до перевода.
- [x] Перенести UI-строки, подсказки, aria-метки и сообщения native в каталоги. Локализовать собственные заголовки picker и About, обновлять About через `set_text` без пересоздания всего системного меню.
- [x] Добавить переключатель в About с подписью «Язык / Language» и вариантами из спецификации. Не создавать повторное уведомление об обновлении при смене языка; сохранить правило скрытия «Проверить обновления», когда доступно «Скачать».
- [x] Брать URL поддержки из конфигурации в обоих местах приложения; скрывать элементы в EN. Сохранить строку copyright/thanks и выравнивание иконки в RU.
- [x] Перевести подготовку GitHub issue и сохранить ограничения приватности/длины URL. Системные технические данные и коды остаются неизменными.
- [x] Проверить: `node --test scripts/tests/locales.test.mjs`, `npm test -- --run`, `npm run test:e2e -- tests/ui/e2e/localization.spec.ts`, `cargo test -p converter-desktop`, `npm run typecheck`.
- [x] Зафиксировать этап коммитом `feat: localize desktop interface in Russian and English`.

### Задача 4. Двуязычная страница проекта

**Files:**
- Create: `website/locales/ru.json`, `website/locales/en.json`, `website/interface-en.jpg`, `website/language.js`.
- Modify: `website/index.html`, `website/style.css`, `website/ORIGIN.md`, `scripts/pages/build.mjs`, `tests/pages/site.test.mjs`, `.github/workflows/pages.yml`.
- Consumes: `config/locales.json`, существующий `website/interface.jpg`, данные последнего стабильного релиза.

**Interfaces:**
- Существующий CLI генератора сохраняется: `node scripts/pages/build.mjs --output <folder>`; JSON релиза читается из stdin.
- Выход: `index.html` (полный EN fallback), `ru/index.html`, `en/index.html`, общий `language.js`, стили/иконки, RU/EN изображения и прежний `release.json`.
- `/converter/ru/` и `/converter/en/` имеют собственные canonical; корень указывает canonical на `/converter/en/`. Переключатель всегда ведёт на явный языковой URL.
- `website/language.js` выбирает язык только для корня: сохранённый ручной выбор сайта > основной `navigator.language` > `en`. Явные `/ru/` и `/en/` не перенаправляются; переход по переключателю запоминает язык с обработкой отказа localStorage.

- [x] Дополнить тесты: одинаковая версия и точные DMG/EXE URL во всех страницах, соответствующие `lang`/title/description/alt, рабочие пути общих ресурсов с вложенных URL. EN не содержит CloudTips и пустых элементов поддержки. Для корня `ru-RU` выбирает `/ru/`, `en-US`, `de-DE` и неизвестный язык — `/en/`; вторичный `ru-RU` не влияет. Явный `/ru/` остаётся русским на английском браузере.
- [x] Проверить, что прежние тесты отказа для draft/prerelease, отсутствующего или неоднозначного установщика остаются активны; запустить и подтвердить падение новых тестов.
- [x] Разделить шаблон и тексты с безопасным экранированием подстановок. Сохранить структуру, стили и пользовательские правки страницы. Добавить RU/English-переключатель, hreflang `ru`, `en`, `x-default` на корень и небольшой скрипт выбора языка. Без JavaScript доступны полный контент, переключатель и скачивание; ошибка localStorage не блокирует переход.
- [x] Снять английский интерфейс из harness с синтетическими данными, записать происхождение в `website/ORIGIN.md`; использовать его на EN-странице. Существующий RU-скриншот сохранить, если он соответствует результату.
- [x] Добавить `config/**` в paths workflow Pages; сохранить чтение одного `/releases/latest`, события published/edited и действующие ограничения стабильного релиза.
- [x] Проверить `node --test tests/pages/*.test.mjs` и локально просмотреть все три URL, обе темы/ширины. Убедиться, что забор данных релиза не выполняется браузером посетителя.
- [x] Зафиксировать этап коммитом `feat: add bilingual product website` без push/deploy до разрешения.

### Задача 5. Общая проверка и сборки для проверки пользователем

**Files:** `tests/ui/e2e/localization.spec.ts`, `.github/workflows/ci.yml` при необходимости включения новых тестов, `docs/validation/localization.md`.

- [x] Выполнить `npm run typecheck`, `npm run check:contracts`, `npm run format:check`, `npm test -- --run`, `npm run test:build`, `node --test tests/pages/*.test.mjs`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`.
- [x] Выполнить `npm run test:e2e` в существующих Chromium/WebKit light/dark-проектах. Новые сценарии последовательно покрывают оба языка; размеры 740×500 и 540×400. Проверить длинные названия/пути, историю, ошибки, список предупреждений, About, отсутствие фокуса на кнопках закрытия при открытии.
- [x] Сверить поддержку новых сообщений с инвентаризацией мест создания ошибок: ни один обычный путь ошибки desktop не должен оставаться только русским из-за пропущенного ключа. Полный список предупреждений остаётся обычным списком, компактное предупреждение — PrimeVue Message.
- [x] Зафиксировать границы нативной проверки: выбор основного языка, сохранение и меню проверены тестами Rust, интерфейс — harness и production CSP. Ручная смена языка ОС и нативная визуальная приёмка остаются пользователю; см. `docs/validation/localization.md`.
- [x] По согласованной реализации собрать macOS Apple Silicon командой `APPLE_SIGNING_IDENTITY=- npm run build:macos`, проверить подпись/артефакт и передать пользователю. Повторная упаковка той же версии — `-- --reuse-version`; не увеличивать patch случайным повтором сборки.
- [ ] Windows x64: отложено по текущему указанию пользователя «пока ничего не публикуй»; существующий GitHub Actions будет запущен после разрешения на push. Intel macOS исключён из матрицы desktop.
- [x] Записать команды, результаты и известное ограничение старых сообщений в `docs/validation/localization.md`. Публикация релиза и Pages остаётся отдельным действием после разрешения пользователя; до него не запускать публичный deploy. Учитывать, что push изменений сайта в `develop` уже запускает Pages.

## Предлагаемый порядок приёмки

1. Согласование этого плана и спецификации.
2. Реализация задач 1–4 и проверки задачи 5, без публикации.
3. Пользователь проверяет macOS; исправляются найденные проблемы.
4. После разрешения на отправку изменений — Windows CI и проверка Windows, затем разрешённый релиз и Pages.

Документ прошёл самопроверку: автодетект использует основной язык, отсутствие EN-доната обработано во всех местах, старая история совместима, сайт обновляется из единого релиза. Решения о первом minor/major-релизе и английском платёжном сервисе не блокируют реализацию языкового слоя.
