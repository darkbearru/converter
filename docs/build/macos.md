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

Для Intel:

```sh
rustup target add x86_64-apple-darwin
npm run build:macos:intel
```

Эта команда добавляет desktop Rust target. Мобильные SDK/targets не требуются. CI отдельно собирает ARM на `macos-15` и Intel на `macos-15-intel`, по [матрице GitHub](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).

Собранные DMG и ZIP с `.app`, сохраняющий права и ресурсы через `ditto`, копируются в `.work/packages/<target>/`. `build-info.json` содержит commit, признак изменённых исходников, версии инструментов и SHA-256 lock-файлов. `SHA256SUMS` относится к готовым архивам. Сборка использует `cargo --locked`; CI сохраняет пакеты как artifacts на 14 дней и не публикует release.

Данные очереди, приватные URL-манифесты и кэш находятся в `~/Library/Application Support/dev.dictionary.converter`. Кэш подтверждённых загрузок переживает закрытие. Выбранные папки назначения регистрируются нативным диалогом; JavaScript не получает произвольный доступ к диску.

Для публичного распространения отдельно нужны Developer ID и notarization. Текущие тестовые пакеты не считаются нотарифицированным выпуском; секреты подписи в репозитории отсутствуют. Проверенные сценарии и ограничения: [desktop-native.md](../validation/desktop-native.md).
