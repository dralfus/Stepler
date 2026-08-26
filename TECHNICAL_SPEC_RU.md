# Stepler: спецификация текущей реализации

Статус документа: as-built specification, то есть описание фактически
реализованного поведения, архитектуры и ограничений.

Дата сверки: 2026-08-26.

Эта спецификация описывает текущий Stepler по исходному коду, контрактным тестам и пользовательским
сценариям, которые закреплены в проекте. Требование, которого нет в этой
спецификации и нет в коде или тестах, не следует считать реализованным.

## Problem Statement

Пользователь набирает текст в Windows-приложениях, иногда находясь в
неправильной раскладке клавиатуры. У разных приложений разные модели текста:
обычный Win32 edit control, Word object model, UI Automation, браузерный
document surface, терминальный буфер или PSReadLine. Одна универсальная
операция выделения и вставки приводит к потере фокуса, повреждению буфера
обмена, удалению переносов строк, записи в соседний контрол или зависанию
Office.

Stepler должен:

- перехватывать P/CP и выбирать способ работы по фактам активной поверхности;
- исправлять текст, набранный в неверной раскладке;
- сохранять корректный текст, форматирование строки и положение каретки,
  насколько это позволяет конкретный адаптер;
- переключать раскладку независимо от текстовой операции;
- отказываться от небезопасной операции, если поверхность не распознана или
  предварительная проверка не подтверждает ожидаемый текст;
- показывать раннюю индикацию нажатия и понятный результат операции;
- не использовать Ctrl+C/Ctrl+Shift+C в терминальных TUI, где эти клавиши
  могут завершить приложение;
- оставаться диагностируемым при задержках, потере фокуса, несовместимом
  контроле и частично поддержанном приложении.

## Solution

Stepler состоит из фонового Windows tray-приложения, hotkey runner, общего
Rust-ядра коррекции, platform/resolver слоя, набора method adapters и
отдельных PowerShell/Qwen/SSH мостов.

Вход операции проходит через общий транзакционный pipeline:

```text
HotkeyReceived
  -> ContextCaptured
  -> PlanBuilt
  -> PreflightChecked
  -> ReplacementApplied
  -> Verified
  -> Completed
```

При ошибке операция завершается без подтвержденной замены. В логах сохраняются
причина отказа и фактическая стадия. Текстовая операция и переключение
раскладки являются связанными действиями одной операции, но транспорт текста
и policy выбора адаптера остаются surface-specific.

### Пользовательские режимы

- `P` — `Pause`: прямое исправление выделения или слова/фрагмента перед
  кареткой. Это режим с минимальным языковым анализом.
- `CP` — `Ctrl+Pause`: `CorrectionMode::ScrollLock` во внутреннем API.
  Внутреннее имя режима — `ScrollLock`; пользовательская клавиша — `Ctrl+Pause`.
  При явном выделении преобразуется ровно выделение. Без выделения анализируется
  текущая логическая строка от последнего `CR/LF` до каретки. Текст справа от
  каретки и предыдущие строки не входят в диапазон.
- `Left Ctrl` — явное переключение активного окна на русскую раскладку.
- `Right Ctrl` — явное переключение активного окна на английскую раскладку.
- `Menu` или разрешенный `Caps Lock` — переход на следующую раскладку.
- `Insert` может быть переназначен в режим Backspace.

### Принцип выбора адаптера

Классификатор возвращает `SurfaceKind`, confidence и evidence. Resolver по
этому результату применяет `SurfacePolicy`, `ProbePlan` и контракт метода.
Адаптер не определяет, является ли окно Rocket.Chat, JIRA или Confluence:
адаптер предоставляет одну технику чтения/замены текста, а policy определяет,
где техника разрешена.

Для risky и clipboard-based методов используется явный allowlist поверхностей.
Неизвестная поверхность обрабатывается консервативно: безопасные UIA методы
могут быть проверены, а risky fallback не включается по умолчанию.

## User Stories

1. Как пользователь Windows, я хочу нажать P на выделенном тексте, чтобы преобразовать ровно выделенную область.
2. Как пользователь Windows, я хочу нажать P без выделения, чтобы исправить слово или layout-aware фрагмент непосредственно перед кареткой.
3. Как пользователь Windows, я хочу нажать CP на выделении, чтобы получить преобразование именно выделения без расширения диапазона.
4. Как пользователь Windows, я хочу нажать CP без выделения, чтобы Stepler рассмотрел текущую логическую строку слева от каретки и не изменил текст правее каретки.
5. Как пользователь, я хочу преобразовывать несколько ошибочных слов одной строкой, сохраняя корректный русский, английский, технический и числовой текст между ними.
6. Как пользователь, я хочу, чтобы CP не удалял `CR`, `LF`, `CRLF`, пробелы и границу строки при замене текста.
7. Как пользователь, я хочу, чтобы текст до и после заменяемого диапазона оставался на своем месте.
8. Как пользователь, я хочу, чтобы каретка после операции возвращалась в логически соответствующее место, если используемый адаптер умеет надежно восстановить ее.
9. Как пользователь, я хочу, чтобы выделение после операции сохранялось или снималось предсказуемо согласно поведению конкретной поверхности.
10. Как пользователь, я хочу сохранить буфер обмена, включая нетекстовые форматы, после clipboard-based операции.
11. Как пользователь, я хочу получить отказ без изменения текста, если preflight не подтвердил ожидаемую строку, контрол потерял фокус или поверхность изменилась.
12. Как пользователь, я хочу видеть индикацию нажатия P/CP независимо от того, поддерживает ли активное приложение текстовую замену.
13. Как пользователь, я хочу видеть успешную замену, отказ, причину и время операции.
14. Как пользователь Notepad, я хочу быстро исправлять P/CP через Win32 edit messages.
15. Как пользователь Word, я хочу работать с выделением и диапазоном перед кареткой через Word object model.
16. Как пользователь Outlook, я хочу исправлять текст в письме и поле поиска через разные безопасные пути, не подвешивая Outlook.
17. Как пользователь Outlook, я хочу, чтобы layout switching не запускал опасный для Word/Zimbra сценарий до подтверждения контекста.
18. Как пользователь PowerShell, я хочу исправлять текущий PSReadLine buffer без clipboard fallback и разрушительных терминальных команд.
19. Как пользователь PowerShell с SSH, я хочу работать через установленный на удаленном Linux host `stepler-remote`.
20. Как пользователь Qwen CLI, я хочу, чтобы P/CP не посылали Ctrl+C или Ctrl+Shift+C и не закрывали Qwen.
21. Как пользователь Qwen, я хочу безопасно отправить готовый текст через side-channel input file.
22. Как пользователь Qwen Input, я хочу исправить текст, сохранить фокус и отправить результат в запущенный Qwen.
23. Как пользователь Qwen Workspace, я хочу иметь terminal/Qwen и Stepler input в одном окне, сохраняя terminal session при перезапуске Stepler.
24. Как пользователь браузерного редактора, я хочу работать в Confluence, JIRA, ChatGPT/Codex, Rocket.Chat, Telegram и похожих полях через policy, учитывающую реальный control и caret.
25. Как пользователь Sticky Notes, я хочу исправлять текст в Note Editor и сохранять переносы строк.
26. Как пользователь неизвестного UIA поля, я хочу получить только безопасный пробный путь или понятный отказ вместо случайной вставки буфера.
27. Как пользователь Windows Terminal, я хочу, чтобы PowerShell, cmd, Qwen и SSH различались как разные поверхности, даже если у них одинаковый hosting window class.
28. Как пользователь, я хочу включать и выключать P, CP, layout controls, Caps Lock, Insert-as-Backspace, risky fallbacks и автозапуск из tray.
29. Как пользователь, я хочу переключать светлую и темную тему tray и Qwen Input и настраивать длительность timing overlay.
30. Как разработчик, я хочу видеть structured JSONL с методом, surface, confidence, стадиями, retry и временем.
31. Как разработчик, я хочу иметь contract tests для классификаторов, policy, resolver и проверенных поверхностей.
32. Как разработчик, я хочу измерять P50/P95 только для одной сборки, окружения, surface, режима и ветки алгоритма.
33. Как сопровождающий, я хочу собрать release win-x64 с tray, CLI, scripts, Qwen-компонентами и remote helper отдельно.
34. Как сопровождающий, я хочу запускать ровно один tray и один hotkey runner от той же distribution build.

## Implementation Decisions

### 1. Границы компонентов

- `stepler-core` не знает о Windows controls и приложениях. Он содержит
  `CorrectionMode`, `TextContext`, `ReplacementPlan`, layout conversion,
  языковую оценку, транзакционные типы и pure logic.
- `stepler-platform` содержит общие идентификаторы методов, capabilities,
  `SurfaceKind`, классификацию, policy, probe plan и resolver contracts.
- `stepler-platform-windows` содержит Windows-specific capture/replace,
  UI Automation, Win32, Word COM, keyboard input, clipboard, console,
  terminal, layout и window helpers.
- `stepler-app` оркестрирует операцию: gate, foreground checks, preflight,
  clipboard guard, apply, verify, telemetry и результат.
- `stepler-cli` является runtime/diagnostic entry point: hotkey loop,
  PowerShell bridge, CLI-команды, диагностика focus и performance snapshot.
- `Stepler.Tray` является .NET tray host и settings UI. Он не реализует
  текстовую коррекцию и не выбирает адаптер.
- `Stepler.Shared` содержит общую логику Qwen Input/Workspace UI.
- `Stepler.QwenWorkspace` объединяет terminal session и Stepler input, но не
  заменяет общий resolver для обычных приложений.
- `stepler-remote` является малым Linux helper для Bash/readline по SSH, а не
  полной Linux desktop-версией Stepler.

### 2. Core-контракт текста

`TextContext` содержит snapshot текста, caret range, optional selection,
capabilities и telemetry. `ReplacementPlan` содержит exact range, replacement
text, reason/confidence и `expected_before_text`.

Перед записью адаптер обязан подтвердить, что target и expected text все еще
совпадают. План не является разрешением на запись в любое окно: он применяется
только через выбранный method adapter и его contract.

Ошибки `NoTextToReplace`, `ReplacementUnavailable`, `PreflightFailed`,
`ClipboardUnavailable`, `UnsupportedControl` и аналогичные являются обычными
результатами операции. Они не должны приводить к частичной замене.

### 3. Семантика P и CP

Для P приоритет имеет explicit selection. Без selection выбирается ближайший
поддерживаемый layout-aware token перед caret; для некоторых filename/path-like
сценариев разрешен sparse fallback только при наличии безопасного источника.
P не выполняет широкую языковую реконструкцию всей строки. Для смешанного
токена присваивания вида `ASCII_NAME=русский_хвост` core сужает диапазон P до
русского хвоста, если перед ним есть ASCII-префикс с `=`. Это сохраняет
префикс переменной и исправляет только значение, например
`export NEXUS_FQDN=туч` превращается в `export NEXUS_FQDN=nex`.

Для CP:

- selection имеет приоритет и заменяется целиком;
- без selection вычисляется граница текущей логической строки по `CR`/`LF`;
- правой границей является caret, а не конец всего snapshot;
- анализируются кандидаты внутри `[line_start, caret]`;
- корректный префикс сохраняется;
- преобразуются только подозрительные слова/диапазоны;
- если safe capture не может подтвердить range, операция отклоняется;
- trailing `CRLF` не входит в replacement range;
- общая семантика принадлежит core/resolver pipeline и не дублируется в отдельных приложениях.

Языковая оценка использует layout conversion, словарные и n-gram/score
сигналы, confidence и исключения CP. Словарь исключений является только
дополнительным сигналом для неоднозначных коротких слов; P не обязан
использовать CP-словарь.

### 4. Surface classification и policy

`SurfaceKind` представляет тип поверхности, а не название продукта. В текущем
наборе есть Win32 edit, Notepad-like, classic console, Windows Terminal cmd и
PowerShell, Qwen terminal, browser/editor, fast browser/editor, Rocket.Chat,
Yandex browser editor, Telegram, Sticky Notes, Outlook search/editor/shell,
Word editor, Excel cell editor и Unknown.

Classification возвращает `kind`, `confidence`, `evidence` (факты окна,
focused control, title, process и UIA) и, при необходимости, web keyboard
profile.

`SurfacePolicy` отдельно задает предпочтения P и CP, запрещенные методы и
`allow_risky_methods`. `ProbePlan` ограничивает набор probe разрешенными для
surface методами и умеет fast probe. Resolver фильтрует unsupported, forbidden
и risky методы до выбора контекста/замены и оставляет trace выбора.

Классификатор не должен возвращать имя “Rocket.Chat adapter” или “JIRA
adapter”. Method adapter отвечает только за способ работы, а не за знание
бизнес-приложения.

### 5. Реестр method adapters

Текущий реестр методов:

- `Win32EditMessages` — `WM_GETTEXT`, selection/caret messages и безопасная
  замена обычного Win32 edit control.
- `TerminalClipboardShortcut` — risky terminal clipboard fallback; не является
  общим способом для PowerShell и запрещен для Qwen.
- `SshTerminal` — Windows-side SSH bridge к remote readline helper.
- `ConsoleBuffer` — classic console buffer; функциональность CP для classic
  cmd остается ограниченной и нестабильной.
- `PsReadLine` — чтение и изменение текущего PowerShell buffer через
  PSReadLine commands, без общего clipboard fallback.
- `WordCom` — Word object model, включая WordEditor в Outlook.
- `UiAutomationEditableText` — writable focused UIA edit/value surface.
- `UiAutomationDocumentText` — UIA document/text pattern с caret/selection
  preflight.
- `UiAutomationText` — совместимый общий UIA text/value путь.
- `XtermKeyboardSelection` — keyboard selection path для textarea/xterm-like
  terminal controls.
- `WebKeyboardSelection` — browser/editor keyboard selection с clipboard
  preflight, expected-text preflight и verify.
- `ClipboardSelection` — risky copy/paste для выделенного текста с обязательным
  восстановлением clipboard.
- `SendInput` — risky write-only Unicode input в текущее выделение.

Каждый method adapter описывает capabilities: чтение selection и caret,
range before caret, замена selection, использование clipboard и risky status.
Resolver не вызывает метод, если его capabilities не покрывают план.

Для ChatGPT-поверхности в `Chrome_WidgetWin_1` перед попыткой прочитать
clipboard-selected текст дополнительно проверяется состояние UIA
`TextPattern` у focused element. Если UIA подтверждает отсутствие явного
выделения, clipboard-selected ветка пропускается, чтобы случайное выделение,
созданное Ctrl+Insert или самим web-контролом, не превратило P в замену всей
строки. Подтвержденное явное выделение сохраняет обычный selected-path. Если
UIA не может сообщить состояние выделения, сохраняется совместимый fallback
web-пути; это ограничение диагностируется логами и не является доказательством
явного выделения.

### 6. Фактическое сопоставление поверхностей

| SurfaceKind | Предпочтительный путь | Ограничение |
| --- | --- | --- |
| Win32Edit | `Win32EditMessages` | Нужен совместимый Win32 edit control |
| NotepadLike | `Win32EditMessages` | Fallback только по явному safe contract |
| ClassicConsole | `ConsoleBuffer` | CP в classic cmd ограничен и не считается стабильной поддержкой |
| WindowsTerminalCmd | `TerminalClipboardShortcut` | Risky allowlist surface, не общий cmd contract |
| WindowsTerminalPowerShell | `PsReadLine` | Нужны PSReadLine и profile bridge |
| QwenTerminal | `XtermKeyboardSelection`/Qwen policy | Ctrl+C и Ctrl+Shift+C запрещены; live TUI prompt снаружи не читается |
| BrowserEditor/FastBrowserEditor | `WebKeyboardSelection` и разрешенный UIA путь | Зависит от strict caret/clipboard preflight |
| RocketChatEditor | UIA editable, затем web keyboard fast profile | Поиск и поле сообщений — разные UIA surfaces |
| TelegramDesktop | Web keyboard/UIA | Это surface contract, а не универсальный Telegram adapter |
| StickyNotes | UIA document, затем web/UIA fallback | Текущий CP сохраняет логические переносы строк |
| OutlookSearch | `Win32EditMessages` | Отдельная политика для поиска |
| OutlookWordEditor | `WordCom` | Office/Zimbra hang-safety важнее latency |
| OutlookShell | Win32, затем разрешенный WordCom | UIA/risky fallback запрещены |
| WordEditor | WordCom, затем разрешенный UIA | Win32/clipboard fallback запрещены |
| ExcelCellEditor | `WebKeyboardSelection` | Зависит от режима ячейки и версии Excel |
| Unknown | безопасные UIA probes | Risky fallback закрыт по умолчанию |

Этот список является текущим поведением, а не обещанием поддержки всех версий
указанных продуктов. Поверхности с ограничениями должны оставаться
fail-closed.

### 7. Capture, preflight, apply и verify

Операция проверяет стабильность foreground/focused target и не выполняется,
если окно или control сменились. Перед apply проверяется expected text и
диапазон. После apply выполняется verify, если его поддерживает метод.

`OperationGate` запрещает конфликтующие параллельные операции для одного
контрола. Clipboard guard сохраняет clipboard snapshot для методов, которые
могут временно использовать clipboard, отслеживает marker/stabilization и
восстанавливает текстовые и нетекстовые форматы. Невозможность безопасно
восстановить или подтвердить clipboard превращает операцию в отказ, а не в
частичную запись.

Focus/caret restore является частью adapter contract. Нельзя считать операцию
успешной только потому, что текст визуально изменился: target, expected text и
итоговое состояние должны соответствовать контракту.

В web keyboard captured-left ветке перед вставкой повторно выделяется ровно
захваченный левый диапазон, replacement вводится через временный clipboard paste,
а исходный clipboard snapshot восстанавливается после операции. Это не меняет
семантику диапазона CP и не разрешает включать текст справа от caret.

### 8. Раскладка клавиатуры

Layout control отделен от text method selection. Глобальный механизм принимает
левый/правый Ctrl и Menu/Caps policy; text replacement не должен подменять
этот механизм app-specific shortcut-ом.

После успешной текстовой операции layout action может выполняться в том же
operation pipeline и не должен задерживать подтвержденную замену сверх
необходимого. Для Outlook/Zimbra предусмотрены narrow safety restrictions:
если контекст может привести к зависанию Word/Outlook, опасный post-replacement
layout path подавляется. Это не отключает текстовую коррекцию в Outlook.

Английская раскладка определяется по culture prefix `en-*`, а не только по
одному фиксированному HKL или `en-US`; это поддерживает `en-GB` и другие
английские варианты. Фактический набор раскладок и право Windows на их
переключение остаются внешними условиями машины.

### 9. Hotkey runtime и PowerShell

CLI hotkey runner обслуживает глобальные сообщения, keyboard hook, callbacks
индикации, text operation и layout control. Tray запускает runner с текущими
settings. Для уже работающих PowerShell-сессий загрузка profile bridge требует
перезапуска сессии или явного dot-source profile.

PSReadLine adapter использует `GetBufferState`, строит план через общий core и
применяет его через `RevertLine`/`Insert`. Общий terminal clipboard fallback
для PowerShell не является штатным контрактом.

В SSH-сценарии Windows Stepler распознает отмеченную SSH terminal surface и
форвардит протокол только если remote helper доступен. Если helper не найден,
операция должна завершиться безопасным отказом; удаленный host не должен
получать произвольные команды из clipboard fallback.

### 10. Qwen Input и Qwen Workspace

Qwen CLI запускается через wrapper/marker и использует `--input-file` для
безопасной отправки подготовленного текста. Нельзя получать текущий TUI
prompt через Ctrl+Shift+C: Qwen может трактовать это как interrupt.

Qwen Input является отдельным .NET окном с общим P/CP поведением, ранней
индикацией, timing overlay, восстановлением фокуса/caret, отправкой и
переключением языка по результату.

Qwen Workspace содержит terminal/Qwen session и Stepler input в одном рабочем
окне. Рабочий каталог задается настройкой/окружением, поддерживается запуск с
`--continue`, а перезапуск Stepler не должен сам по себе завершать внешнюю
PowerShell/Qwen session. Реальное взаимодействие с Qwen terminal остается
ограниченным его TUI и отдельной policy.

### 11. Tray, настройки и индикация

Tray-only UI предоставляет статус, запуск/перезапуск runner, выход, открытие
Qwen Input/Workspace, логи и настройки. Настройки сохраняются в пользовательском
профиле Windows и включают как минимум:

- `PauseEnabled` и `ScrollLockEnabled`;
- `CtrlLayoutSwitchEnabled` и `MenuCapsSwitchEnabled`;
- `DisableCapsLock`;
- `InsertAsBackspaceEnabled`;
- `RiskyFallbacksEnabled`;
- `DarkTheme`;
- `ShowTimingOverlay` и `TimingOverlayDurationMs`;
- `QwenWorkspaceDirectory`.

Индикация P/CP создается на уровне hotkey runtime как можно раньше и не
зависит от того, найден ли подходящий адаптер. Затем она обновляется результатом
операции: success, fail, no text, unsupported и elapsed time. Отсутствие
поддержанного метода не должно выглядеть как потерянное нажатие.

### 12. Логи, диагностика и производительность

Основные диагностические каналы: tray log, structured JSONL hotkey log, hook
signal log и performance events с surface kind/confidence, method,
profile/branch, cold/warm, retry, длинами и фазами.

CLI предоставляет диагностику focus/methods, PSReadLine self-test/plan,
performance snapshot, Qwen submit, layout commands и UIA fixture.

P50/P95 сравниваются только внутри одной build, environment, surface kind,
mode, selection/no-selection и algorithm branch. Целевой ориентир для локальных
операций — P50 до 300 ms и P95 до 600 ms, но это не оправдывает отключение
preflight, clipboard restore, expected-text check, verify или focus restore.
Office, SSH, cold start и network latency рассматриваются отдельно.

### 13. Сборка и распространение

Release собирается для `win-x64` и включает tray, CLI, runtime scripts,
Qwen-компоненты и соответствующие ресурсы. Linux remote helper собирается
отдельно, обычно через WSL, и копируется на Linux host вручную или скриптом.

Версия сборки должна присутствовать в имени/метаданных release и отображаться
в tray. После rebuild запускать Stepler следует из distribution directory вне
sandbox; приемка требует проверить, что остаются живы ровно один tray process
и один hotkey runner от той же distribution build.

## Testing Decisions

### Что считается хорошим тестом

Тесты проверяют внешнее поведение и контракты: выбранный surface/method,
диапазон, replacement text, отсутствие правой части строки, сохранение
переносов, clipboard/focus safety, failure reason и состояние транзакции.
Тест не должен закреплять внутреннюю последовательность вызовов, если она не
является частью safety contract.

### Unit и pure-core tests

Обязательно проверяются P с selection и без selection, trailing space,
punctuation и filename-like input; CP с mixed line, несколькими подозрительными
токенами, корректным prefix, caret внутри слова, selection и no-selection;
границы `CR`, `LF`, `CRLF`, пустой строки и предыдущей строки; отсутствие
изменений справа от caret; layout conversion и английские `en-*` варианты;
CP dictionary exceptions; смешанный ASCII assignment prefix; transaction state
machine и operation gate.

### Resolver и policy contract tests

Для каждой проверенной поверхности контракт закрепляет ожидаемый `SurfaceKind`,
минимальный confidence/evidence, первый разрешенный method для P и CP,
запрещенные методы, допустимый fallback, risky allowlist и поведение при
unsupported control/failed preflight.

Отдельно проверяются invariants: forbidden method никогда не выбирается,
risky method не появляется на Unknown, Qwen никогда не получает terminal
clipboard shortcut, Outlook policy не получает generic risky fallback,
classification и resolver используют один target snapshot.

### Adapter и operation tests

На уровне adapter contracts проверяются capabilities, exact range, preflight,
verify, clipboard guard, focus/caret restore и no-partial-mutation. В Windows
integration smoke используются реальные или fixture controls для Win32 edit,
UIA editable/document, classic console, Windows Terminal, Word/Outlook,
browser-like editor, Sticky Notes и Qwen input. Для ChatGPT в
`Chrome_WidgetWin_1` отдельно проверяются no-selection, explicit-selection и
UIA-unavailable сценарии: первый не должен принимать ложный selected clipboard,
второй должен сохранять selected-path, третий должен оставаться совместимым и
диагностируемым.

Для Outlook тесты включают Word editor, Outlook search и shell surfaces, а
также проверку, что text replacement остается доступным при подавлении
опасного layout path. Отсутствие зависания в synthetic test не доказывает
безопасность Office: нужен ручной smoke и лог foreground/target/phase.

### Runtime, release и ручная приемка

Перед release выполняются:

```powershell
cargo fmt --all -- --check
cargo test --workspace
```

Для Windows-specific paths дополнительно выполняются release build,
`diagnose-focus --delay 3 --methods`, PSReadLine self-test, UIA fixture,
performance snapshot и ручная матрица приложений. В snapshot нельзя смешивать
домашний и рабочий ПК, разные сборки, cold/warm и разные surface kinds.

Ручная матрица текущего состояния:

| Поверхность | Текущий статус |
| --- | --- |
| Notepad / обычный Win32 edit | поддерживается через `Win32EditMessages` |
| Word 2016 | поддерживается через `WordCom`; нужен smoke после сборки |
| Outlook 2016 search | отдельный Win32 contract; проверять отдельно |
| Outlook 2016 письмо | `WordCom`; text path поддерживается, layout safety ограничен |
| PowerShell / Windows Terminal | поддерживается через `PSReadLine` при корректном profile |
| PowerShell с SSH | поддерживается только с remote helper |
| Qwen CLI | безопасный wrapper/side-channel; live prompt ограничен |
| Qwen Input/Workspace | поддерживается отдельным UI/runtime path |
| Confluence/JIRA/ChatGPT/Codex web/editor | зависит от UIA/WebKeyboard preflight |
| Rocket.Chat | отдельные editor/search surface contracts |
| Sticky Notes | UIA document path, CP сохраняет line breaks |
| classic cmd/conhost | ограниченная поддержка; CP не считается стабильным |
| cmd внутри Windows Terminal | risky/diagnostic path, не общий safe contract |
| Excel cell editor | отдельный web keyboard contract, зависит от режима ячейки |

## Further Notes

1. `README.md` остается пользовательским руководством: команды запуска,
   profile setup и краткая таблица приложений могут быть подробнее, чем здесь.
   Этот документ является источником архитектурных и поведенческих контрактов.
2. `docs/pcp_latency_optimization_spec_ru.md` остается рабочим документом по
   измерению задержек. Он не переопределяет semantics P/CP или surface policy.
3. `docs/release_smoke_checklist_ru.md` является приемочным checklist, а не
   альтернативной спецификацией.
4. При изменении одного адаптера сначала меняется его method contract и
   resolver/policy contract tests. Поверхностные правила нельзя прятать в
   adapter implementation.
5. При добавлении новой поверхности сначала фиксируются target evidence,
   SurfaceKind, разрешенные методы, forbidden methods и fallback. Только после
   этого добавляется adapter code.
6. При изменении общего CP-контракта обязательны regression tests для
   no-selection current-line-to-caret, selection, line breaks и минимум одной
   web/UIA, одной Win32 и одной terminal-like поверхности.
7. При оптимизации запрещено удалять safety phases ради цифры latency. Сначала
   нужен labeled baseline и phase contribution, затем узкое изменение одного
   transport/worker branch с rollback при регрессии.
8. Текущая спецификация намеренно различает “реализовано”, “ограничено” и
   “требует ручного smoke”. Это предотвращает превращение единичного успешного
   запуска в обещание поддержки всей категории приложений.
