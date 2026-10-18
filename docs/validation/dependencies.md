# Зависимости desktop

2026-10-18. GitHub Dependabot отметил `glib 0.18.5` из общего `Cargo.lock`: [RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html), ошибка `VariantStrIter` в версиях 0.15–0.19.

Проверен граф для обоих выпускаемых target:

```sh
cargo tree --locked --target aarch64-apple-darwin --invert glib
cargo tree --locked --target x86_64-pc-windows-msvc --invert glib
```

Оба вызова вернули `nothing to print`: `glib` не входит в macOS/Windows граф. В manifest Tauri 2.12.4 и wry 0.57.0 GTK/WebKitGTK с glib 0.18 подключаются только для Linux/BSD. В Linux CI проверяются Rust-библиотеки с `--exclude converter-desktop`; нативного Linux-пакета в этом выпуске нет.

Уведомление не скрывалось и зависимости не подменялись несовместимой версией. Перед добавлением Linux-приложения необходима отдельная переоценка GTK/glib и устранение advisory. Это ограничение общего lock-файла не считается проверкой безопасности всех прочих зависимостей.
