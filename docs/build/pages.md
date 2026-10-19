# GitHub Pages

Публичная страница продукта: https://darkbearru.github.io/converter/.

Текст и структура находятся в `website/index.html`, стили — в `website/style.css`, происхождение снимка — в `website/ORIGIN.md`. Приложение и его версия при изменении сайта не пересобираются.

Workflow `pages.yml` запускается при изменении файлов сайта в `develop`, публикации/редактировании стабильного релиза и вручную. Исходники сайта всегда берутся из `develop`; информация о версии — из GitHub API `releases/latest`. Скрипт `scripts/pages/build.mjs` проверяет наличие единственного опубликованного DMG Apple Silicon и EXE Windows x64 нужной версии и их прямые URL в этом репозитории. При ошибке новая страница не публикуется, предыдущая остаётся рабочей. Черновики и предварительные релизы не публикуют сайт.

Версия, размеры и две ссылки встроены в статический HTML. Посетителю не нужны JavaScript, API-запросы к GitHub или авторизация. Публичный каталог содержит только страницу, стили, изображения и короткий `release.json`; словари и внутренние файлы проекта туда не копируются.

Проверка и локальная сборка:

```sh
node --test tests/pages/*.test.mjs
gh api repos/darkbearru/converter/releases/latest > .work/pages-release.json
node scripts/pages/build.mjs < .work/pages-release.json
```

Результат находится в `.work/pages/site/`. Для публикации GitHub Pages использует режим GitHub Actions, окружение `github-pages` и официальные Actions с закреплёнными SHA. Окружение должно разрешать `develop` и release-теги `v*`, чтобы публикация нового релиза могла обновить сайт.
