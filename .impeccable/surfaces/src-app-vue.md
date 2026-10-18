---
version: 1
slug: "src-app-vue"
primary_target: "src/App.vue"
related_targets: ["src/styles/app.css","src/features/composer/SourceComposer.vue","src/features/jobs/CompletedHistory.vue","src/features/jobs/ActiveTasks.vue"]
---

# Переработка рабочего окна

Источник решения — прямые указания пользователя от 18.10.2026 и image1–image5. Они заменяют прежнюю композицию с левой очередью. Кодовый путь уже выбран; новые макеты и сборки не требуются. Проверки — dev-сервер, тесты и typecheck. Push, запускающий desktop packages, отложен.

Mode: Operate. Пользователь выбирает/перетаскивает источник или скачивает URL, затем отдельно запускает конвертацию. После запуска панель снова принимает следующий файл; задачи поднимаются из-под неё. Завершённые и остановленные задания находятся сверху в Accordion с результатом, активным путём, диагностикой и действиями восстановления.

## Direction contract

THESIS: Рабочее окно по композиции Codex: история сверху, постоянная панель ввода снизу, задачи между ними.

OWN-WORLD: PrimeVue Aura, системная светлая/тёмная тема, системная типографика. Основной оранжевый #F96133 и голубой #65C8EE взяты из book.svg; нейтральные цвета — токены PrimeVue. SVG из предоставленной svg-single.

STORY: Выбрать файл → конвертировать либо ссылка → скачивание → готовый файл → конвертировать. Настройки и ресурсы доступны в Dialog. Результат открывается в ОС по проверенному ID задания.

FIRST VIEWPORT: Изменяемое окно 740×500, компактный заголовок, прокручиваемая история, нижняя панель FileUpload высотой около180px с действиями справа. Активные строки видны над панелью. Диалоги укладываются в окно.

FORM: Прямо заданная пользователем композиция image1–image5; прежний seed a332c5d1 больше не определяет layout. Подписи файлов и кнопки остаются читаемыми при длинных именах и масштабировании.

MOTION: Spatial consistency. TransitionGroup, translateY/opacity,240ms cubic-bezier(0.23,1,0.32,1); новые строки выходят из-под панели, старые сдвигаются вверх; reduced motion убирает перемещения.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance

Уточнения пользователя: конвертация пока имеет отмену и повтор; настоящая пауза нужна только скачиванию. SVG находятся в iCloud Materials/170-thin-line-ui-icons/svg-single. Книга увеличивается на30%, края не обрезаются.
