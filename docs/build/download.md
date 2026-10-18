# Получение словаря по ссылке

Rust CLI скачивает файлы самостоятельно; PHP, Python и внешние программы скачивания не нужны.

```sh
cargo run --locked -p converter-cli -- fetch --url 'https://example.org/dictionary.zip' --cache .downloads
cargo run --locked -p converter-cli -- fetch --source-id SOURCE_ID --cache .downloads
cargo run --locked -p converter-cli -- convert --url 'https://example.org/main.dsl.dz' --resources-url 'https://example.org/resources.zip' --cache .downloads --output results
```

`--sha256` задаёт ожидаемый SHA-256 основного источника. `--resources-url` и локальный `--resources` можно повторять. Локальный `--source` и `--url` взаимоисключающие. Весь набор удалённых источников должен быть получен до начала подготовки словаря.

Ctrl-C возвращает 130 и сохраняет подтверждённую часть в постоянном кэше. Повтор той же URL-команды использует тот же непрозрачный source ID; `fetch --source-id` продолжает без повторного ввода ссылки. Изменение ссылки или эталонного SHA создаёт другой источник. Готовая копия перед повторным использованием проверяется по сохранённым хешам, новый GET не нужен.

stderr содержит JSON с `phase: download` или `phase: conversion`. stdout содержит только итоговый JSON. При неизвестном размере передаются байты и скорость, общего процента нет. Полные URL хранятся только в приватных манифестах кэша; не публикуйте их вместе с логами.

Докачка использует сильный ETag и проверенный `206 Content-Range` того же конечного URI. `200`, `416` и смена валидатора требуют полной загрузки. Неожиданное HTTP-кодирование отклоняется. TLS-сертификаты проверяются; переход HTTPS → HTTP запрещён. Максимум три дополнительных сетевых повтора через 2/5/15 секунд; `Retry-After` учитывается до пяти минут. После большего ожидания предлагается продолжить позже.

Проверки: `cargo test --locked -p converter-download -p converter-cli`. Для ручной проверки локального обрыва: `cargo run --locked -p converter-test-support --example serve_fixture -- PATH_TO_LARGE_DSL`.
