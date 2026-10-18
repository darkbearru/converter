# Сборка и запуск ядра

Для разработки нужен Rust 1.99.1; версия закреплена в rust-toolchain.toml. CLI компилируется в самостоятельный исполняемый файл, без PHP/Python и без установленного Rust на компьютере пользователя. SQLite включён в сборку. Tauri/Vue оболочка добавляется отдельным этапом.

```sh
cargo test --locked -p converter-core -p converter-cli
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo build --locked --release -p converter-cli
```

На macOS/Linux бинарник находится в `target/release/converter-cli`, на Windows — `target/release/converter-cli.exe`.

```sh
converter-cli convert --source dictionary.dsl --resources dictionary.files --output result
converter-cli convert --source package.zip --output result --encoding auto
```

`--resources` принимает папку, ZIP или переданный словарь сокращений; флаг можно повторить. ZIP с несколькими основными словарями создаёт несколько результатов. Поддерживаются `--encoding auto|utf8|utf16le|utf16be|windows1251` и `--profile desktop|mobile`. Mobile здесь меняет бюджет памяти, не устанавливает мобильные SDK и не создаёт мобильное приложение.

Прогресс и предупреждения выводятся в stderr; stdout содержит JSON `results` с директориями и счётчиками. Код выхода: 0 при успехе, в том числе с явно указанными предупреждениями; 1 при ошибке; 130 при Ctrl-C. Исходники открываются только на чтение.

Результат публикуется каталогом `<имя>-converted`, затем `<имя>-converted-1` и т.д. В нём находятся `dictionary.mdx`, `dictionary.mdd`, `conversion-report.json`. Staging создаётся на том же томе. Атомарное переименование не заменяет существующую цель; копирование с другого тома не используется как скрытый fallback.

Контрольный ZIP с авторским DSL, сокращением, PNG и WAV генерируется Rust-примером:

```sh
cargo run --locked -p converter-core --example generate_acceptance_fixture -- .work/reader-acceptance
cargo run --locked -p converter-cli -- convert --source .work/reader-acceptance/acceptance.zip --output .work/reader-acceptance/output
```

Генератор требует новый каталог и не перезаписывает существующие данные. Крупный DSL для ручных измерений создаёт `generate_large_fixture`; нагрузочный тест описан в [протоколе](../validation/pipeline-2026-10-18.md).
