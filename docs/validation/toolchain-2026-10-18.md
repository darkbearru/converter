# Проверка глобального Rust

Дата: 18 октября 2026 года. Платформа: macOS ARM64.

По прямому запросу пользователя глобальный Rust обновлён до последнего доступного stable через официальные `rustup self update` и `rustup update stable`.

| Компонент | До | После |
| --- | --- | --- |
| rustup | 1.26.0 | 1.29.1 |
| rustc | 1.72.1 | 1.99.1 (`58cf8f9c3`, 2026-10-13) |
| cargo | 1.72.1 | 1.99.1 (`5f94df478`, 2026-08-27) |
| rustfmt | Не фиксировался | 1.10.0-stable |
| clippy | Не фиксировался | 0.1.99 |

`rustup update stable` сообщил: latest update 2026-10-15, version 1.99.1. После установки в новом login shell с рабочей директорией `/private/tmp` проверены версии компонентов и `rustup show`:

```text
stable-aarch64-apple-darwin (active, default)
active because: it's the default toolchain
installed targets: aarch64-apple-darwin
```

Пути прокси остались `/Users/a_abramenko/.cargo/bin/{rustup,rustc,cargo}`. Глобальная установка находится в `/Users/a_abramenko/.rustup`; пользовательский default — `stable`, а не локальный override проекта.

Дополнительно во временной директории создан проект без зависимостей, edition 2024, печатающий `6 * 7`. Проверки:

| Команда | Результат |
| --- | --- |
| `cargo run --offline --quiet` | Код 0, stdout `42` |
| `cargo fmt -- --check` | Код 0 |
| `cargo clippy --offline -- -D warnings` | Код 0, предупреждений нет |

Для smoke-проекта `CARGO_HOME` и `CARGO_TARGET_DIR` были направлены внутрь его временной директории; после проверки она удалена. Использовались глобальные compiler/cargo/rustfmt/clippy через rustup. Это проверка инструментов: продуктовые тесты конвертера выполняются на этапах A–C.

Порядок дальнейшей работы: сначала реализация и все предусмотренные проверки macOS/Windows. Установка мобильных SDK и мобильный этап отложены по указанию пользователя.
