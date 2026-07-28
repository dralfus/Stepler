# Baseline задержек P/CP

Дата снимка: 2026-07-26.

Этот документ фиксирует исходные данные перед оптимизацией скорости P/CP.
Он не предлагает менять runtime-поведение и не является отчетом о выполненной
оптимизации.

## Цель измерения

Найти активные adapter methods, у которых типичная успешная операция занимает
больше 300 ms, определить фазу-задержку и подготовить отдельный безопасный план
для каждого метода.

Рабочее определение медленного метода:

- `p50 > 300 ms` на репрезентативной выборке;
- `p95` используется как контроль нестабильности;
- единичный результат больше 300 ms не делает метод медленным;
- выборка меньше 30 операций считается недостаточной для окончательного
  решения.

Время считается от `HotkeyReceived` до терминального состояния операции,
включая восстановление clipboard, caret/focus и проверку результата.

## Источник и качество данных

Источник:

`%LOCALAPPDATA%\Stepler\logs\stepler_hotkey_log.jsonl`

На момент анализа:

- распознано 3588 JSON-событий;
- найдено 1893 успешных операции `Completed`;
- 1015 успешных операций имеют `timestamp_unix_ms`;
- timestamped-период: 2026-06-16 15:47:56 - 2026-07-26 11:47:54;
- за последние 7 дней доступно 192 успешных операции;
- в текущей сборке `1.0.20260726.t1124` доступно только 4 успешных операции.

Ограничения:

1. Накопительный лог объединяет несколько версий Stepler. Вся история пригодна
   для поиска устойчиво медленных methods, но не для точного сравнения двух
   последних реализаций.
2. Поле `app` часто содержит только Win32-классы. Например,
   `Chrome_WidgetWin_1/Chrome_WidgetWin_1` не позволяет отличить Codex, JIRA,
   Confluence и другие Chromium/Electron surfaces.
3. В completed event нет `SurfaceKind`, web profile, capture/apply branch,
   признака cold/warm и исходного состояния selection.
4. История с домашнего ПК не входит в этот файл.
5. Некоторые methods имеют слишком маленькую или только старую выборку.

Поэтому текущие числа являются baseline для планирования, а не доказательством,
что конкретная surface уже достигла или не достигла целевого времени.

## Сводка по adapter methods

Основная таблица использует все timestamped successful operations.

| Method | N | p50, ms | p90, ms | p95, ms | >300 ms | Вывод |
|---|---:|---:|---:|---:|---:|---|
| `word_com` | 24 | 1792 | 2504 | 2534 | 100% | Медленный |
| `uia_editable_text` | 106 | 1311 | 1837 | 2015 | 100% | Медленный |
| `uia_document_text` | 41 | 1125 | 1286 | 1402 | 100% | Медленный |
| `embedded_terminal_psreadline` | 63 | 1005 | 2214 | 2598 | 100% | Медленный |
| `web_keyboard_selection` | 751 | 559 | 2718 | 2858 | 84.8% | Медленный и двухрежимный |
| `win32_edit_messages` | 29 | 22 | 46 | 46 | 0% | Цель уже выполнена |
| `uia_text` | 1 | 2982 | 2982 | 2982 | 100% | Данных недостаточно |

Исторические methods, которые нельзя оценивать как текущий основной путь:

| Method | N | p50, ms | Комментарий |
|---|---:|---:|---|
| `xterm_keyboard_selection` | 3 | 623 | Недостаточная старая выборка |
| `console_buffer` | 26 | 108 | Быстрый p50, но известная функциональная нестабильность cmd |
| `embedded_terminal_passthrough` | 16 | 428 | Старое имя/поведение, отсутствует в текущем `MethodId` |

## Последние 7 дней

Последние 7 дней лучше отражают текущие ветки, но выборка меньше.

| Method | N | p50, ms | p90, ms | p95, ms | Главная задержка |
|---|---:|---:|---:|---:|---|
| `web_keyboard_selection` | 125 | 2769 | 2949 | 3103 | `Verified`: p50 2091 ms |
| `word_com` | 6 | 1717 | 2452 | 2452 | capture p50 784 ms + apply p50 932 ms |
| `uia_editable_text` | 55 | 1293 | 1722 | 2009 | capture p50 663 ms + apply p50 605 ms |
| `embedded_terminal_psreadline` | 6 | 1026 | 1384 | 1384 | Детальные phase timings не записываются |

Для текущей сборки всего 4 операции: три
`web_keyboard_selection` с p50 2850 ms и одна
`embedded_terminal_psreadline` 1285 ms. Этого недостаточно для решения, но
выборка подтверждает, что задержка видима и в последней сборке.

## Разрез по приложениям и классам

Последние 7 дней:

| Method + app class | N | total p50 | capture p50 | apply p50 | verify p50 |
|---|---:|---:|---:|---:|---:|
| `web_keyboard_selection`, Chromium/Electron | 49 | 2823 | 480 | 246 | 2089 |
| `web_keyboard_selection`, Firefox | 58 | 2718 | 400 | 247 | 2093 |
| `web_keyboard_selection`, Qt 5.15.19 | 18 | 2787 | 441 | 240 | 2092 |
| `word_com`, Word/Outlook editor | 6 | 1717 | 784 | 932 | 0 |
| `uia_editable_text`, Chromium/Electron | 30 | 1356 | 730 | 603 | 0 |
| `uia_editable_text`, Chrome render host | 10 | 1223 | 586 | 642 | 0 |
| `uia_editable_text`, Firefox | 12 | 1174 | 620 | 586 | 0 |

Для Sticky Notes в полном timestamped-периоде:

- `uia_document_text`: N=12, p50=781 ms;
- `web_keyboard_selection`: N=18, p50=1518 ms.

Эти строки нельзя автоматически объединять с текущей политикой Sticky Notes:
лог не содержит `SurfaceKind` и версии policy.

## Диагноз по methods

### WebKeyboardSelection

Подтвержденный bottleneck последних 7 дней - `Verified`, а не сама вставка:

- capture p50: 436 ms;
- apply p50: 247 ms;
- verify p50: 2091 ms.

Распределение двухрежимное: в истории есть успешные операции около 100-200 ms,
но текущие полные verify-path часто занимают около 2.8 s. Значит, перед
изменением timeout нельзя усреднять все branches. Нужны отдельные метрики по:

- surface;
- web profile;
- selected/no-selection;
- capture branch;
- apply branch;
- verify branch;
- успешной первой попытке и retry.

### UIAutomationEditableText и UIAutomationText

Capture и apply каждый запускают отдельный `powershell.exe` с UI Automation
script. Наблюдаемые p50 фаз близки к стоимости запуска процесса и выполнения
скрипта:

- capture: 663 ms;
- apply: 605 ms.

`uia_text` имеет только одну успешную запись и должен анализироваться вместе с
семейством UIA, но не объявляться отдельно оптимизированным без новой выборки.

### UIAutomationDocumentText

Текущий timestamped p50 равен 1125 ms. Этот method также использует отдельные
PowerShell UIA scripts. Последних семидневных данных недостаточно, поэтому
нужно сначала получить новую выборку по Sticky Notes и другим surfaces, где он
является первым разрешенным method.

### WordCom

Capture и apply запускают отдельные PowerShell/COM scripts:

- capture p50: 784 ms;
- apply p50: 932 ms.

Это объясняет почти всю задержку. При оптимизации обязательно сохранить
изоляцию от Word/Outlook: ранее эти приложения зависали на COM-path, поэтому
перенос COM в tray process недопустим без отдельного доказательства
безопасности.

### EmbeddedTerminalPsReadLine

Типичный результат около 1 s. Подробные `timings_ms` для этого special path не
пишутся. Скрипт синхронно запускает `stepler-cli` для построения плана и
дополнительные команды для переключения раскладки, а также создает delayed
PowerShell process для повторной синхронизации layout.

До оптимизации нужно разделить:

- расчет и замену строки;
- первичное переключение layout;
- отложенную проверку/повтор layout;
- cold start `stepler-cli`.

### XtermKeyboardSelection

Три старые записи с p50 623 ms не образуют baseline. Qwen Terminal и Qwen
Workspace нужно измерить отдельно: они используют разные control-plane paths и
не должны объединяться только по слову Qwen.

### Win32EditMessages

p50 22 ms, p95 46 ms. Общая оптимизация ему не нужна. Он используется как
контрольная группа: изменение общей инфраструктуры не должно ухудшить его
p95 более чем на 10 ms.

## Недостающая телеметрия

До первой runtime-оптимизации completed/no-change/failure event должен позволять
сгруппировать операцию по следующим полям:

- build version;
- `SurfaceKind`;
- classification confidence;
- context method и replacement method;
- profile;
- capture/apply/verify branch;
- `Pause`/`ScrollLock`;
- selection present/absent;
- cold/warm;
- retry count;
- terminal outcome;
- phase timings для special/bridge paths.

Текст пользователя в performance dataset не нужен. Достаточны длины, ranges,
ветки и результаты.

## Матрица нового baseline

Для каждого активного медленного method требуется минимум 30 успешных warm
операций и 5 cold операций. Внутри выборки должны присутствовать:

- `P` и `CP`;
- selection и no-selection;
- латиница -> кириллица и кириллица -> латиница;
- короткое слово и строка с несколькими языками;
- caret в конце и caret внутри строки;
- clipboard с текстом и clipboard с изображением, если method использует
  clipboard.

Обязательные surfaces:

| Surface | Ожидаемый первый method |
|---|---|
| Notepad | `win32_edit_messages` |
| Codex Windows app | `web_keyboard_selection` |
| JIRA web | `web_keyboard_selection` |
| Confluence Firefox/web | `web_keyboard_selection` |
| Telegram Desktop | `web_keyboard_selection` |
| Rocket.Chat search | `uia_editable_text` |
| Sticky Notes | `uia_document_text` |
| Word 2016 | `word_com` |
| Outlook 2016 compose | `word_com` |
| PowerShell/Windows Terminal | `psreadline` bridge |
| Qwen Terminal | `xterm_keyboard_selection` |

Домашний и рабочий ПК нужно хранить как разные environment labels, не выводя
имя пользователя в отчет.

## Результат baseline

К оптимизации допускаются:

1. `web_keyboard_selection`;
2. `uia_editable_text` / `uia_text`;
3. `uia_document_text`;
4. `word_com`;
5. `embedded_terminal_psreadline` / `PsReadLine`;
6. `xterm_keyboard_selection` только после новой выборки.

Не допускаются к общей оптимизации:

- `win32_edit_messages` - уже быстрее цели;
- `console_buffer` - отдельная функционально нестабильная и сейчас не
  приоритетная cmd-задача;
- risky fallback methods без отдельной surface allowlist и regression case.

