# Спецификация: снижение задержки P/CP

Статус: готово к декомпозиции после сбора нового instrumented baseline.

Исходные данные:
`docs/pcp_latency_baseline_ru.md`.

## Статус выполнения

Статус фиксируется относительно конкретной сборки и набора измерений. Новые
runtime-данные не возвращают закрытый этап в состояние "не выполнено": для
новой сборки создается следующий snapshot и сравнивается с сохраненным
baseline.

| Работа | Статус | Условие завершения |
|---|---|---|
| Анализ существующего накопительного лога | Выполнено 2026-07-26 | Baseline сохранен в отдельном документе |
| Определение медленных method families и bottlenecks | Выполнено 2026-07-26 | Составлен план для каждого method с наблюдаемым временем выше 300 ms |
| Спецификация и acceptance budgets | Выполнено 2026-07-26, требуется подтверждение порогов | Пользователь подтверждает p50/p95 и test seam |
| Этап 0A: расширение performance telemetry | Не начат | Все перечисленные telemetry fields пишутся для terminal outcomes |
| Этап 0B: новый instrumented baseline | Заблокирован этапом 0A | Не меньше 30 warm и 5 cold операций на method/environment |
| Этап 1: `WebKeyboardSelection` | Не начат | Выполнены budget и regression criteria этапа |
| Этап 2: UI Automation family | Не начат | Выполнены budget, parity и worker recovery criteria |
| Этап 3: PSReadLine bridge | Не начат | Critical path отделен от delayed layout repair и выполнен budget |
| Этап 4: `WordCom` | Не начат | Выполнены budget и Office hang-safety smoke |
| Этап 5: `XtermKeyboardSelection` | Ожидает данных этапа 0B | Сначала N не меньше 30, затем решение о необходимости оптимизации |

Текущий baseline является завершенным снимком, но не заменяет этап 0B:
накопительный лог смешивает версии Stepler и не содержит части полей,
необходимых для однозначного сравнения surfaces и algorithm branches.

## Problem Statement

Пользователь Stepler ожидает, что после нажатия P или CP исправление раскладки
будет восприниматься как мгновенное. В части проверенных приложений операция
занимает от 1 до 3 секунд, хотя быстрый Win32 path выполняется за десятки
миллисекунд.

Накопленные логи показывают, что типичное время превышает 300 ms у
`WebKeyboardSelection`, UI Automation methods, `WordCom` и PowerShell
PSReadLine bridge. При этом текущая телеметрия недостаточно точно связывает
задержку с `SurfaceKind`, profile и algorithm branch. Преждевременное
сокращение timeout может вернуть уже исправленные дефекты: потерю clipboard,
вставку не в тот range, дублирование текста, потерю focus/caret и зависание
Word/Outlook.

## Solution

Сначала расширить performance telemetry без изменения выбора methods и
replacement behavior. Затем снять воспроизводимый baseline для каждой
проверенной surface и оптимизировать по одному method family за раз.

Для каждого этапа:

1. Зафиксировать текущие p50/p95 и распределение фаз.
2. Изменить только один bottleneck.
3. Прогнать автоматические contracts и ручную surface matrix.
4. Сравнить p50/p95 с baseline.
5. Откатить этап, если нарушена корректность или сохранность состояния.

Целевой бюджет для локальных non-network операций:

- p50 не больше 300 ms;
- p95 не больше 600 ms;
- ни одна проверка корректности не удаляется только ради скорости;
- если platform limitation не позволяет достичь бюджета безопасно, этап
  документирует измеренный floor и не маскирует его искусственным ранним
  индикатором завершения.

## User Stories

1. Как пользователь Stepler, я хочу получать исправленный текст не позднее чем
   через 300 ms в типичном случае, чтобы P/CP ощущались как обычная клавиша.
2. Как пользователь Stepler, я хочу видеть честное полное время операции,
   включая verify и восстановление состояния, чтобы индикатор не скрывал
   задержку.
3. Как пользователь Codex Windows app, я хочу быстрые P/CP с выделением и без
   выделения, чтобы не ждать keyboard-selection verify несколько секунд.
4. Как пользователь JIRA и Confluence, я хочу ускорение без удаления соседних
   строк, таблиц и переносов.
5. Как пользователь Telegram и browser-like editors, я хочу ускорение без
   порчи изображения или текста в clipboard.
6. Как пользователь Rocket.Chat search, я хочу быстрый UIA path без потери
   focus.
7. Как пользователь Sticky Notes, я хочу быстрый document-text path с
   сохранением переносов, caret и выделения.
8. Как пользователь Word 2016, я хочу ускорение без зависания Word.
9. Как пользователь Outlook 2016, я хочу ускорение compose editor без
   зависания Outlook/Zimbra Connector и без регрессии поиска писем.
10. Как пользователь PowerShell, я хочу, чтобы P/CP исправляли строку и меняли
    раскладку без нескольких заметных запусков helper-процессов.
11. Как пользователь Qwen Terminal, я хочу отдельную оценку terminal TUI path,
    чтобы его не смешивали с Qwen Input/Workspace.
12. Как разработчик, я хочу видеть в логе `SurfaceKind`, method, profile и
    branch, чтобы задержка конкретного приложения не терялась внутри общего
    Win32-класса.
13. Как разработчик, я хочу различать cold и warm операции, чтобы запуск
    helper-процесса не искажал steady-state baseline.
14. Как разработчик, я хочу видеть phase timings для всех bridge paths, чтобы
    не оптимизировать неподтвержденную часть цепочки.
15. Как разработчик, я хочу сравнивать P и CP, selection и no-selection, чтобы
    быстрый сценарий не скрывал медленный.
16. Как разработчик, я хочу иметь minimum sample rule, чтобы не принимать
    решение по одному удачному запуску.
17. Как разработчик, я хочу оптимизировать один method family за этап, чтобы
    источник регрессии был однозначным.
18. Как разработчик, я хочу сохранить resolver и surface contracts, чтобы
    ускорение одного приложения не меняло adapter другого.
19. Как разработчик, я хочу, чтобы Win32EditMessages оставался контрольной
    группой и не замедлялся из-за общей инфраструктуры.
20. Как разработчик, я хочу fail-closed поведение при timeout helper-а, чтобы
    ускорение не приводило к слепой вставке текста.
21. Как разработчик, я хочу отдельные метрики успешных, no-change,
    unsupported и failed операций, чтобы пользовательская задержка отказа тоже
    была видна.
22. Как разработчик, я хочу измерять рабочий и домашний ПК как разные
    environments, чтобы аппаратная разница не считалась регрессией adapter-а.
23. Как релиз-инженер, я хочу воспроизводимый отчет до и после каждого этапа,
    чтобы решение о включении оптимизации было основано на данных.
24. Как релиз-инженер, я хочу сохранить ручной smoke для реальных Office,
    browser, Electron, Qt и XAML surfaces, потому что synthetic tests не
    доказывают корректность focus/clipboard/caret.

## Implementation Decisions

### Общие решения

- Порог включения method family в работу: p50 больше 300 ms при N не меньше 30.
- Methods с N меньше 30 сначала получают измерения, а не runtime-изменение.
- Performance event содержит build, environment label, surface, confidence,
  context/replacement methods, profile, branch, trigger, selection state,
  cold/warm, retry count, outcome и phase timings.
- Performance event не хранит пользовательский текст; разрешены длины, ranges
  и категориальные признаки.
- Существующие `SurfaceKind`, probe policy, resolver policy и forbidden/risky
  contracts остаются источником истины. Performance optimization не получает
  права переклассифицировать приложение.
- Порядок этапов определяется вкладом в пользовательскую задержку и риском:
  WebKeyboardSelection, UIA family, PSReadLine, WordCom, затем Xterm после
  набора данных.

### Этап 0. Instrumented baseline

- Добавить недостающие поля ко всем terminal operation events, включая
  no-change, unsupported и failure.
- Для special/bridge paths записывать те же фазы, что и для OperationRunner.
- Добавить branch markers для keyboard selection, UIA и terminal paths.
- Снять не меньше 30 warm операций на каждый активный slow method и не меньше
  5 cold операций на method/environment.
- Хранить snapshot отчета отдельно от накопительного runtime log.
- Не менять timeout, retry, resolver order и replacement behavior на этом
  этапе.

### Этап 1. WebKeyboardSelection

- Сначала разделить показатели по web profile и branch.
- Подтвержденный bottleneck `Verified` исследовать отдельно от capture/apply.
- Заменять полный verify timeout только там, где есть более раннее
  положительное доказательство результата: clipboard sequence, ожидаемое
  selection state или surface-specific acknowledgement.
- Не убирать preflight и clipboard restore.
- Не использовать один fast path для Codex, JIRA, Confluence, Telegram и
  Sticky Notes без отдельного surface contract.
- Retry должен выполняться только после наблюдаемого отрицательного результата,
  а не после фиксированной паузы.
- Каждый новый fast branch получает allowlist surfaces и negative contracts.

### Этап 2. UI Automation family

- `UiAutomationEditableText`, `UiAutomationText` и
  `UiAutomationDocumentText` измерять раздельно, но оптимизировать общей
  инфраструктурой.
- Устранить запуск нового PowerShell process на каждую capture/apply операцию.
- Предпочтительный seam - долгоживущий изолированный STA helper с простым
  request/response protocol, request id, deadline и автоматическим restart.
- Не переносить UIA/COM worker на tray UI thread.
- Сначала реализовать read-only capture request и сравнить результат с текущим
  script path; apply подключать отдельным этапом после parity tests.
- При crash/timeout worker-а операция завершается unsupported/failed без
  fallback к запрещенному keyboard method.

### Этап 3. PSReadLine bridge

- Разделить время correction plan, PSReadLine replacement, primary layout
  switch и delayed layout repair.
- Убрать синхронные повторные запуски CLI из critical path операции.
- Объединить control-plane команды одной request/response операцией к уже
  работающему Stepler process либо другим существующим persistent seam.
- Delayed layout verification выполнять после завершения текстовой операции и
  не включать в блокировку ввода, но продолжать логировать отдельно.
- Сохранить поведение локального PowerShell, embedded Codex terminal и запрет
  generic PSReadLine path внутри SSH без remote helper.

### Этап 4. WordCom

- Не размещать Word/Outlook COM automation внутри tray process.
- Использовать отдельный долгоживущий STA helper с жестким deadline,
  health-check и принудительным restart после зависшего COM call.
- Capture и apply переводить на helper по одному, проверяя Word и Outlook
  отдельно.
- Не кешировать Document/Inspector/WordEditor object между сменами окон без
  повторной проверки identity.
- При timeout не выполнять SendInput/clipboard fallback.
- Обязательный smoke включает Word 2016, Outlook 2016 compose, Outlook search,
  закрытие документа и смену Inspector.

### Этап 5. XtermKeyboardSelection

- До изменения получить N не меньше 30 отдельно для Qwen Terminal.
- Не объединять Qwen Terminal с Stepler Qwen Input и Qwen Workspace.
- Если p50 остается выше 300 ms, использовать тот же принцип event-driven
  clipboard acknowledgement, что и для web keyboard, но с отдельными terminal
  shortcuts и contracts.
- Любой shortcut, который Qwen трактует как Ctrl+C, остается запрещенным.

### Методы вне текущей оптимизации

- `Win32EditMessages`: только regression guard, без рефакторинга.
- `ConsoleBuffer`: функциональная стабильность cmd является отдельной задачей.
- `TerminalClipboardShortcut`, `ClipboardSelection`, `SendInput`: не
  включаются ради скорости без явной surface allowlist.
- SSH network latency не включается в локальный 300 ms budget.

## Testing Decisions

- Высший автоматический seam - operation outcome и его telemetry, а не
  внутренние sleep/timeout значения.
- Unit tests не должны утверждать wall-clock timing Windows scheduler.
- Resolver/probe/policy contracts должны подтверждать неизменный выбор method
  до и после каждого performance этапа.
- Replacement behavior tests должны покрывать ranges, mixed-language text,
  caret restore, selection restore и layout switch.
- Clipboard tests должны покрывать Unicode text, изображение и отсутствие
  clipboard mutation после операции.
- Helper protocol tests должны покрывать normal response, stale request id,
  timeout, crash, restart и второй успешный request после restart.
- Для UIA helper сначала нужен parity test: одинаковый target дает одинаковый
  context в старом script path и новом helper path.
- Для WordCom нужен external behavior smoke; fake COM unit test не считается
  доказательством отсутствия зависания Office.
- Для WebKeyboardSelection обязательны реальные smoke cases: Codex, JIRA,
  Confluence, Telegram и browser ChatGPT; результаты не объединяются до
  surface-level отчета.
- Для каждого этапа сравниваются p50, p95, failure rate и retry rate.
- Этап принимается только при N не меньше 30 warm успешных операций на
  затронутый method/surface и отсутствии новых destructive outcomes.
- `Win32EditMessages` p95 не должен ухудшиться более чем на 10 ms.

## Out of Scope

- Исправление оставшихся функциональных багов, не связанных с внесенной
  performance-правкой.
- Включение P/CP в unsupported cmd surfaces.
- Полноценная Linux desktop версия.
- Изменение алгоритма определения языка и CP dictionaries.
- Изменение hotkey mapping, tray UI или индикатора.
- Замена surface classifier и resolver architecture.
- Ускорение SSH network round-trip до локального бюджета.
- Одновременная переработка нескольких method families.

## Further Notes

- Текущий baseline показывает, что простое уменьшение одного timeout не решит
  все методы: web path тратит время на verify, UIA и Word - на два запуска
  PowerShell/COM scripts, PSReadLine - на несколько control-plane process
  calls.
- Самый большой потенциальный быстрый выигрыш находится в
  WebKeyboardSelection verify path, но он же имеет высокий риск возврата
  destructive web regressions.
- Самый предсказуемый архитектурный выигрыш находится в изолированном
  persistent helper для UIA/COM. Его следует вводить по read-only parity-first
  схеме.
- Текущая сборка имеет слишком мало наблюдений. Runtime-оптимизация не должна
  начинаться до instrumented baseline.
- Перед созданием implementation tickets нужно подтвердить, что p50 является
  порогом пользователя для понятия "method медленнее 300 ms". Если требуется
  оценивать каждую отдельную операцию, acceptance budget должен быть p100, что
  существенно дороже и чувствительнее к Windows scheduler.
