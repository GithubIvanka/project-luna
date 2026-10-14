# Аудит памяти и навыков Project Luna — 2026-10-13

## Результат

Основной путь claude-mem восстановлен: автоматический наблюдатель работает через выбранный пользователем Codex/ChatGPT, создаёт наблюдения и сводки, поиск читает записи, а стартовый контекст снова содержит прежние ручные заметки. Вход Claude Desktop не использовался как доказательство авторизации отдельного SDK-процесса.

Аудит охватил 69 файлов SKILL.md на диске, из них 48 навыков были доступны в этом чате. Ещё 20 файлов относятся к кэшированным шаблонам, один — к внутреннему review-agent; наличие файла в кэше не означает доступность навыка. Проверены обязательные YAML-поля и локальные Markdown-ссылки; 64 Python-скрипта прошли синтаксический разбор. Это структурная проверка, а не доказательство выполнения каждого внешнего рабочего процесса.

Исправлены 17 входных файлов навыков, включая все девять Luna. Их суммарный размер изменился с 80 777 до 39 469 байт; это измерение размера инструкций, а не экономии оплаченных токенов. Полный прежний справочник Graphify сохранён как отдельный reference. Семь исправлений плагинных навыков и помощник ручной памяти согласованы также в кэше Claude.

## Что мешало и что изменено

| Проблема | Исправление | Доказательство |
| --- | --- | --- |
| Наблюдатель использовал неавторизованный Claude; GUI-вход не давал ему рабочего CLI/SDK-входа | С согласия пользователя выбран provider codex и абсолютный путь к уже авторизованному CLI. Модель оставлена по умолчанию Codex | health: provider codex, lastInteraction.success=true; новые контрольные observation #20 и summary #2 |
| Стартовый контекст ограничивал источники текущим хостом | Включён CLAUDE_MEM_SESSION_START_INCLUDE_ALL_SOURCES для общей памяти проекта | Конфигурация и новый стартовый контекст |
| 18 ручных заметок имели пустой concepts и выпадали из стандартного фильтра индекса | После согласованной резервной копии добавлен общий тег how-it-works. Все остальные поля проверены неизменными | manual-index-repair.json; заметки #5/#17/#18 появились в индексе |
| Новые ручные записи повторяли этот дефект | Добавлен помощник index-manual-note.py и инструкция его вызова после save_memory | Новая заметка #55; успешное индексирование, повторный запуск, отказы для чужого проекта и автоматической записи |
| AST возвращал одинаковое «не удалось разобрать» для Rust и TypeScript | Восстановлен отдельный runtime: tree-sitter-cli 0.26.9 и совместимые Rust/TypeScript/JavaScript/Python грамматики. Архив официального GitHub release проверен по SHA256 | Rust outline настоящего operations.rs; TypeScript outline, smart_search и smart_unfold |
| Graphify срабатывал на любой вопрос и требовал немедленного запроса без проверки области/свежести | Узкий маршрутизатор, проверка по исходникам, прямой текстовый поиск при промахе; подробности загружаются отдельно | Запрос ProcessCgroup вернул реальный символ, наличие определения сверено |
| learn-codebase принуждал читать все файлы при обычном знакомстве с проектом | Полный охват оставлен только при явно выбранном полном чтении | Исправленный SKILL.md и валидатор |
| make-plan требовал копировать документационные примеры вместо изменения существующей реализации | Разрешено минимальное переиспользование и миграция по действующим контрактам | Исправленный SKILL.md и валидатор |
| version-bump переносил на пользователя разрешения автора на npm-публикацию и доступ к его токену | Убраны чужие аккаунты, автоматическая замена ~/.npmrc и предполагаемое разрешение на публикацию/уведомления | Исправленные копии навыка в Codex и Claude |
| CCS Align и Agent Cost Report привязаны к инфраструктуре автора | Отключены только эти два навыка через skills.config; файлы сохранены | TOML проверен; резервная копия конфигурации сохранена |
| Навыки Luna оставались на английском и содержали посторонние правила выбора облачных/free-провайдеров | Русские инструкции, канонический workbench/два слота, разумные проверки по типу задачи; удалена посторонняя маршрутизация | Девять навыков Luna прошли валидатор |

Процесс worker не перезапускался: PID 6241 сохранён. Изменение провайдера и сброс кэша выполнены штатным локальным API. Параметр concurrency=1 сохранён для следующего создания пула; действующий пул, созданный до аудита, не переинициализировался.

## Проверенные рабочие пути

- SQLite quick_check: ok; FTS содержал все исходные 17 наблюдений.
- Chroma: реальный глубокий semantic-search round-trip healthy.
- save_memory -> mem_search: ручная запись #18 и затем итоговая #55.
- work_state_write -> work_state_read -> стартовый контекст: список аудита виден.
- sessions/init -> observations -> новая запись #20 -> summarize -> сводка #2 -> поиск: отдельный контролируемый проект project-luna-tools-audit-probe.
- Стартовый контекст: без ложного сообщения «памяти нет» и без действующего предупреждения signed out.
- smart_outline: настоящий Rust-файл Luna и контрольный TypeScript; search/unfold вернули требуемую функцию.
- 17 изменённых навыков: штатный quick_validate, 17/17 PASS.
- Ручной помощник: четыре проверки PASS; текст/поля записи и автоматическое наблюдение сохранены.

В финальном снимке памяти было 61 наблюдение и 3 сводки. Это число записей работающих сессий на момент проверки; оно не является количеством исправлений Luna.

## Оставшиеся условия и ограничения

Прямые prime_corpus/query_corpus используют Claude SDK независимо от основного provider. Контрольный prime действительно вернул отказ входа. Перевод наблюдателя на Codex не исправляет эти API. Навык knowledge-agent теперь предлагает рабочий тематический анализ текущим Codex по результатам mem_search; build_corpus остаётся доступным. Перенос самого vendor API на Codex в этот аудит не входил.

Функционально проверены Rust и TypeScript AST. JavaScript/Python грамматики установлены, но полный набор языков не проверялся. Отсутствующую грамматику нельзя выдавать за отсутствие символа.

Внешние действия — публикация npm, GitHub review/CI, cloud sync, Pets, подключение Excel/Google Docs, публикация Sites — не запускались ради проверки. В таблице они отмечены условными. CLI gh сейчас не авторизован; это отдельная авторизация, а не доказательство состояния доступного GitHub-коннектора.

Отключения skills.config применяются при следующем запуске Codex. Автопамять уже работает в текущем процессе. Локальные правки кэша навыков и ссылки зависимостей могут быть заменены обновлением плагина: сохранены оригиналы, подготовленные исправления и контрольные суммы для повторной проверки. Версия 13.35.0 закреплена только как проверенная текущая установка, а не запрет обновления.

Публичные архитектурные контракты и исполняемый код ОС не менялись этим аудитом. Сборка/QEMU ОС для правки инструментов не выполнялась и не заявляется доказательством результата.

## Матрица доступных навыков

«Проверены инструкции» означает проверку маршрутизации, условий и файлов; это не утверждение, что конкретный PR, документ или внешняя интеграция успешно обработаны.

| Навык | Статус в этом аудите | Когда полезен для Luna |
| --- | --- | --- |
| [imagegen](/home/sibitti/.codex/skills/.system/imagegen/SKILL.md) | Инструмент доступен; генерация в аудите не запускалась | Арты и обои Luna. Для рунических колец с точной геометрией предпочтителен собственный renderer. |
| [openai-docs](/home/sibitti/.codex/skills/.system/openai-docs/SKILL.md) | Применён; официальные страницы получены | Настройки Codex, навыки, модели и API OpenAI. |
| [skill-creator](/home/sibitti/.codex/skills/.system/skill-creator/SKILL.md) | Применён; 17 изменённых навыков прошли валидатор | Содержательные исправления навыков и проверка их структуры. |
| [skill-installer](/home/sibitti/.codex/skills/.system/skill-installer/SKILL.md) | Применён; получен каталог 39 навыков | Выбор дополнительных навыков; отметки installed требуют сверки с системными и плагинными навыками. |
| [graphify](/home/sibitti/.codex/skills/graphify/SKILL.md) | Запрос выполнен; символ сверён с исходником | Межкомпонентные связи. Граф — снимок от 12 октября; проверять по текущим исходникам. |
| [agent-cost-report](/home/sibitti/.codex/plugins/cache/claude-mem-local/claude-mem/13.35.0/skills/agent-cost-report/SKILL.md) | Отключён в конфигурации Codex | Привязан к учётным данным, Mac и хранилищам автора; не даёт достоверный отчёт этого окружения без адаптации. |
| [babysit](/home/sibitti/.codex/plugins/cache/claude-mem-local/claude-mem/13.35.0/skills/babysit/SKILL.md) | Условия проверены; PR-цикл не запускался | Наблюдение за конкретным PR. Для долгого ожидания использовать доступные автоматизации; нужен доступ к GitHub. |
| [ccs-align](/home/sibitti/.codex/plugins/cache/claude-mem-local/claude-mem/13.35.0/skills/ccs-align/SKILL.md) | Отключён в конфигурации Codex | Специальное рабочее место CCS/Worker Watch автора; соответствующее окружение отсутствует. |
| [cloud-sync](/home/sibitti/.codex/plugins/cache/claude-mem-local/claude-mem/13.35.0/skills/cloud-sync/SKILL.md) | Условный; подключение не выполнялось | Отдельный запрос резервирования в cmem.ai; локальной памяти этот сервис не требуется. |
| [design-is](/home/sibitti/.codex/plugins/cache/claude-mem-local/claude-mem/13.35.0/skills/design-is/SKILL.md) | Инструкции проверены; дизайн здесь не оценивался | Аудит конкретного дизайна по Рамсу. Сложная многоагентная процедура для отдельной задачи. |
| [do](/home/sibitti/.codex/plugins/cache/claude-mem-local/claude-mem/13.35.0/skills/do/SKILL.md) | Инструкции проверены; план здесь не исполнялся | Выполнение явно выбранного поэтапного плана. Не требует применения к каждой обычной правке. |
| [handoff](/home/sibitti/.codex/plugins/cache/claude-mem-local/claude-mem/13.35.0/skills/handoff/SKILL.md) | Инструкции проверены | Передача большой незавершённой работы; дополняет, но не заменяет work_state и доказательства. |
| [how-it-works](/home/sibitti/.codex/plugins/cache/claude-mem-local/claude-mem/13.35.0/skills/how-it-works/SKILL.md) | Исправлен по фактическим результатам | Объясняет независимые пути ручной записи, наблюдателя, поиска и начального контекста. |
| [knowledge-agent](/home/sibitti/.codex/plugins/cache/claude-mem-local/claude-mem/13.35.0/skills/knowledge-agent/SKILL.md) | Корпус создан; prime получил отказ авторизации | Исправлен рабочий путь анализа через текущий Codex и mem_search. Прямые prime/query остаются Claude-only. |
| [learn-codebase](/home/sibitti/.codex/plugins/cache/claude-mem-local/claude-mem/13.35.0/skills/learn-codebase/SKILL.md) | Исправлены границы охвата | Обзор репозитория по задаче; чтение всех исходников только при явно выбранном полном охвате. |
| [make-plan](/home/sibitti/.codex/plugins/cache/claude-mem-local/claude-mem/13.35.0/skills/make-plan/SKILL.md) | Исправлено требование копировать вместо переиспользования | Планирование по действующим контрактам и существующим компонентам. |
| [mem-search](/home/sibitti/.codex/plugins/cache/claude-mem-local/claude-mem/13.35.0/skills/mem-search/SKILL.md) | Запись и прогрессивный поиск выполнены | Основной поиск прошлого контекста; добавлен проверенный помощник индексирования ручной заметки. |
| [mode-creator](/home/sibitti/.codex/plugins/cache/claude-mem-local/claude-mem/13.35.0/skills/mode-creator/SKILL.md) | Скрипты синтаксически проверены; настройка режима не запускалась | Пользовательская схема памяти. Telegram, новые провайдеры и изменение режима — только для соответствующего запроса. |
| [oh-my-issues](/home/sibitti/.codex/plugins/cache/claude-mem-local/claude-mem/13.35.0/skills/oh-my-issues/SKILL.md) | Условный; внешние изменения не выполнялись | Большой backlog с общими причинами; нужен выбранный репозиторий и доступ к GitHub. |
| [pathfinder](/home/sibitti/.codex/plugins/cache/claude-mem-local/claude-mem/13.35.0/skills/pathfinder/SKILL.md) | Инструкции проверены | Отдельное исследование дублирования; не заменяет принятые архитектурные границы Luna. |
| [smart-explore](/home/sibitti/.codex/plugins/cache/claude-mem-local/claude-mem/13.35.0/skills/smart-explore/SKILL.md) | Восстановлен; outline/search/unfold выполнены | Локальный AST для Rust/TypeScript/JavaScript/Python. При неподдерживаемом языке — rg и Graphify. |
| [standup](/home/sibitti/.codex/plugins/cache/claude-mem-local/claude-mem/13.35.0/skills/standup/SKILL.md) | Инструкции проверены; рабочие деревья не объединялись | Сравнение нескольких веток или PR; в обычной задаче не требуется. |
| [timeline-report](/home/sibitti/.codex/plugins/cache/claude-mem-local/claude-mem/13.35.0/skills/timeline-report/SKILL.md) | Условия проверены; отчёт истории не создавался | История проекта по имеющимся данным памяти; не выдумывать отсутствующие периоды и экономию токенов. |
| [version-bump](/home/sibitti/.codex/plugins/cache/claude-mem-local/claude-mem/13.35.0/skills/version-bump/SKILL.md) | Исправлены границы публикации | Выпуск именно указанного плагина. Убрано предполагаемое разрешение автора на npm-токен, публикацию и уведомления. |
| [weekly-digests](/home/sibitti/.codex/plugins/cache/claude-mem-local/claude-mem/13.35.0/skills/weekly-digests/SKILL.md) | Условия проверены; главы не создавались | Длинная последовательная история по неделям; для краткого статуса достаточно поиска памяти. |
| [what-the](/home/sibitti/.codex/plugins/cache/claude-mem-local/claude-mem/13.35.0/skills/what-the/SKILL.md) | Инструкции проверены | Краткое объяснение технической темы; специальной внешней службы не требует. |
| [wowerpoint](/home/sibitti/.codex/plugins/cache/claude-mem-local/claude-mem/13.35.0/skills/wowerpoint/SKILL.md) | Не проверен сквозным запуском; не основной путь | Специализированная стилистика и окружение. Для презентаций Luna есть штатный presentations. |
| [visualize](/home/sibitti/.codex/plugins/cache/openai-bundled/visualize/1.0.49/skills/visualize/SKILL.md) | Инструменты/скрипты доступны; макет здесь не создавался | Интерактивные объяснения и быстрые прототипы; статические экспортируемые фигуры — отдельные артефакты. |
| [plugin-management](/home/sibitti/.codex/plugins/cache/openai-curated-remote/plugin-management/0.1.0/skills/plugin-management/SKILL.md) | Применены правила выбора возможностей | Проверка и выбор интеграции, когда штатных средств недостаточно. |
| [sites](/home/sibitti/.codex/plugins/cache/openai-curated-remote/sites/1.0.0/skills/sites/SKILL.md) | MCP и файлы доступны; сайт не создавался | Сайт проекта, демонстрация или портал. Не подменяет нативный рабочий стол ОС. |
| [create-pet](/home/sibitti/.codex/plugins/cache/openai-curated-remote/work-pets/0.1.6/skills/create-pet/SKILL.md) | Условный; операции Pets не запускались | Только анимированный питомец ChatGPT Work; не относится к реализации ОС Luna. |
| [pets](/home/sibitti/.codex/plugins/cache/openai-curated-remote/work-pets/0.1.6/skills/pets/SKILL.md) | Условный; операции Pets не запускались | Управление питомцами ChatGPT Work по отдельному запросу. |
| [update-pet](/home/sibitti/.codex/plugins/cache/openai-curated-remote/work-pets/0.1.6/skills/update-pet/SKILL.md) | Условный; операции Pets не запускались | Изменение питомца ChatGPT Work по отдельному запросу. |
| [documents](/home/sibitti/.codex/plugins/cache/openai-primary-runtime/documents/26.1011.11113/skills/documents/SKILL.md) | Python-библиотеки импортируются; экспорт не проверялся | DOCX/документы для обмена. Доступ Google Docs/Drive проверяется отдельно при таком запросе. |
| [pdf](/home/sibitti/.codex/plugins/cache/openai-primary-runtime/pdf/26.1011.11113/skills/pdf/SKILL.md) | Poppler и библиотеки доступны; PDF здесь не создавался | Экспортируемые спецификации и отчёты; нужен render-and-inspect для конкретного документа. |
| [presentations](/home/sibitti/.codex/plugins/cache/openai-primary-runtime/presentations/26.1011.11113/skills/presentations/SKILL.md) | pptxgenjs/artifact-tool доступны; deck не создавался | Презентация Alpha или архитектуры; не требует специализированного wowerpoint. |
| [excel-live-control](/home/sibitti/.codex/plugins/cache/openai-primary-runtime/spreadsheets/26.1011.11113/skills/excel-live-control/SKILL.md) | Условный; живая Excel-сессия не проверялась | Только подключённая живая книга Excel. Для обычных файлов применяется spreadsheets. |
| [spreadsheets](/home/sibitti/.codex/plugins/cache/openai-primary-runtime/spreadsheets/26.1011.11113/skills/spreadsheets/SKILL.md) | openpyxl/pandas/artifact-tool доступны; книга не создавалась | Таблицы оборудования, проверок и бюджетов. Отсутствие exceljs не является отказом применяемого artifact-tool. |
| [template-creator](/home/sibitti/.codex/plugins/cache/openai-primary-runtime/template-creator/26.1011.11113/skills/template-creator/SKILL.md) | Скрипт доступен; шаблон не создавался | Повторяемые артефакты по явному запросу создать шаблон; одноразовый документ не требует нового навыка. |
| [luna-agent-workflow](/home/sibitti/source/project-luna/.agents/skills/luna-agent-workflow/SKILL.md) | Исправлен и проверен | Русский инженерный цикл, разумный объём проверок и сохранение посторонней работы; удалены чужие правила маршрутизации free/Ollama. |
| [luna-architecture](/home/sibitti/source/project-luna/.agents/skills/luna-architecture/SKILL.md) | Исправлен и проверен | Принятые контракты и границы компонентов; архитектура проекта не менялась. |
| [luna-boot](/home/sibitti/source/project-luna/.agents/skills/luna-boot/SKILL.md) | Исправлен и проверен | UEFI, handoff и постоянный PID 1; сборка загрузчика в этом аудите не запускалась. |
| [luna-build-qemu](/home/sibitti/source/project-luna/.agents/skills/luna-build-qemu/SKILL.md) | Исправлен и проверен | Канонический workbench, два слота и доказательства QEMU для изменений образа. |
| [luna-documentation](/home/sibitti/source/project-luna/.agents/skills/luna-documentation/SKILL.md) | Исправлен и проверен | Русская документация, точные технические имена и различение планов/результатов. |
| [luna-filesystem](/home/sibitti/source/project-luna/.agents/skills/luna-filesystem/SKILL.md) | Исправлен и проверен | LUNA-SYS/ext4, LUNA-DATA/Btrfs, логический корень и ресурсы DATA. |
| [luna-init](/home/sibitti/source/project-luna/.agents/skills/luna-init/SKILL.md) | Исправлен и проверен | Постоянный PID 1, handoff, дочерний системный runtime и корректный жизненный цикл. |
| [luna-kernel](/home/sibitti/source/project-luna/.agents/skills/luna-kernel/SKILL.md) | Исправлен и проверен | Минимальная интеграция Linux и приоритетное оборудование Alpha. |
| [luna-rust](/home/sibitti/source/project-luna/.agents/skills/luna-rust/SKILL.md) | Исправлен и проверен | musl, границы unsafe, русские SAFETY и проверки значимого поведения. |

## Полезные дополнения

Проверен официальный каталог из 39 навыков; новые навыки не устанавливались автоматически.

1. [gh-fix-ci](https://github.com/openai/skills/tree/main/skills/.curated/gh-fix-ci) — разбор именно GitHub Actions, когда появляются реальные отказы PR. Требует рабочего gh-доступа и учитывает уже данное разрешение на исправление.
2. [gh-address-comments](https://github.com/openai/skills/tree/main/skills/.curated/gh-address-comments) — обработка замечаний по выбранному PR, полезна при переходе от локальной Alpha к ревью изменений.
3. [security-threat-model](https://github.com/openai/skills/tree/main/skills/.curated/security-threat-model) — отдельная модель угроз для handoff, посредника запуска, cgroups/namespaces и доверия бандлам; имеет смысл по явному запросу такого аудита.

Для текущей работы полезнее прежде всего уже имеющиеся Luna-навыки, mem-search, восстановленный smart-explore и Graphify. Для дизайна — design-is, imagegen и visualize; для материалов на экспорт — documents/pdf/presentations.

security-best-practices из каталога заявляет Python/JavaScript/TypeScript/Go, поэтому не выбран как общий аудит нативного Rust-стека. Figma требует соответствующей интеграции и полезна при совместной работе над макетами; Sites относится к сайту проекта. Уже доступные openai-docs и pdf не нужно устанавливать повторно: простой список установщика учитывает пользовательский каталог и не полностью отражает плагинные/системные навыки.

## Материалы и восстановление

Доказательства: `target/agent-tools-audit-20261013/`: final-verification.json, skills-inventory.json, all-skills-validation.json, skills-final-changes.json, skills-final-validation.json, observer-probe.json, manual-index-repair.json, manual-note-helper-tests.json, ast-runtime-repair.json, startup-final.txt.

Резервные копии SQLite сохранены с правами 0600. Настройки наблюдателя: `/home/sibitti/.claude-mem/backups/settings-before-codex-audit-20261013T041304Z.json`. Настройки Codex: `/home/sibitti/.codex/skill-audit-backups/20261013T042402Z/config.toml`. Прежние навыки: `target/agent-tools-audit-20261013/skill-backups/`. Не восстанавливайте старую SQLite поверх продолжающих работать сессий: резервная копия предназначена для точечного восстановления прежних полей, а не удаления новых записей.

Правила обнаружения и отключения навыков сверены с [официальной документацией](https://learn.chatgpt.com/docs/build-skills). Источник закреплённого бинарника AST — [tree-sitter v0.26.9](https://github.com/tree-sitter/tree-sitter/releases/tag/v0.26.9); SHA256 архива записан в ast-runtime-repair.json.
