# macOS

Нужны Node.js 24.18.0, npm 12.2.0, Rust из `rust-toolchain.toml`, Xcode Command Line Tools. Для основной desktop-сборки здесь использованы CLT; полноценный Xcode потребуется для последующего iOS этапа. Python и PHP не нужны приложению и не входят в пакет.

```sh
npm ci
npm test -- --run
npm run typecheck
cargo test --locked --workspace --all-targets
npm run build:macos
```

Apple Silicon: `target/aarch64-apple-darwin/release/bundle/macos/Converter.app`, DMG рядом в `bundle/dmg`. Переносить приложение можно из DMG в Applications обычным способом; исходники и словари внутри `.app` не хранятся.

Intel временно исключён из CI и обязательных файлов релиза. Отдельная сборка при необходимости:

```sh
rustup target add x86_64-apple-darwin
npm run build:macos:intel
```

Каждая новая локальная сборка через `build:macos`, `build:macos:intel` или `build:windows` автоматически увеличивает patch: `0.2.0 → 0.2.1`. Major/minor сохраняются. До запуска Tauri синхронизируются `package.json`, `package-lock.json`, `Cargo.toml`, `Cargo.lock` и `tauri.conf.json`. Эти изменения входят в следующий коммит. Проверки, `npm run build` (только Vue) и dev-режим версию не меняют. При ошибке упаковки зарезервированный номер сохраняется.

Остальные платформы того же выпуска, а также повтор после ошибки собираются с уже подготовленной версией:

```sh
npm run build:macos -- --reuse-version
```

Сборки в одном checkout запускаются последовательно. GitHub Actions использует версию из коммита с `--reuse-version`, поэтому macOS Apple Silicon и Windows одного выпуска получают один номер. Перед отправкой нового выпуска нужно сначала подготовить его локальной сборкой; для подготовки номера без упаковки есть `node scripts/build-version.mjs`. Повтор workflow сохраняет тот же номер. Для отдельной сборки Intel того же выпуска используйте `npm run build:macos:intel -- --reuse-version`. Проверка механизма: `npm run test:build`.

`rustup target add` добавляет desktop Rust target. Мобильные SDK/targets не требуются. CI собирает macOS только на ARM runner `macos-15`; готовое приложение подписывается ad-hoc и проверяется через `codesign --verify --deep --strict`.

Собранные DMG и ZIP с `.app`, сохраняющий права и ресурсы через `ditto`, копируются в `.work/packages/<target>/`. `build-info.json` содержит commit, признак изменённых исходников, версии инструментов и SHA-256 lock-файлов. `SHA256SUMS` относится к готовым архивам. Сборка использует `cargo --locked`; CI сохраняет пакеты как artifacts на 14 дней и не публикует release.

Данные очереди, приватные URL-манифесты и кэш находятся в `~/Library/Application Support/dev.dictionary.converter`. Кэш подтверждённых загрузок переживает закрытие. Выбранные папки назначения регистрируются нативным диалогом; JavaScript не получает произвольный доступ к диску.

Для публичного распространения отдельно нужны Developer ID и notarization. Текущие тестовые пакеты не считаются нотарифицированным выпуском; секреты подписи в репозитории отсутствуют. Проверенные сценарии и ограничения: [desktop-native.md](../validation/desktop-native.md).
