# Tickets: снижение задержки P/CP

Задачи реализуют поэтапное снижение задержки P/CP до целевых
`p50 <= 300 ms` и `p95 <= 600 ms` без ослабления проверок корректности.
Источник требований и единственный актуальный статус: `docs/pcp_latency_optimization_spec_ru.md`.

## Текущий frontier

T01-T03 реализованы. Перед запуском любой runtime-оптимизации T04-T13 нужно
собрать labeled snapshot для конкретной release-сборки и target surface.
Неполные либо `unlabeled` данные не являются основанием менять timeout, retry
или порядок adapter methods.

Работать следует по **frontier**: можно начинать только задачу, все blockers
которой уже завершены. Из-за чувствительности adapter layer рекомендуется
выполнять доступные задачи по одной и после каждой сохранять зеленые contracts
и manual smoke затронутых surfaces.

## T01. Единая performance-телеметрия OperationRunner

**Status:** implemented for ordinary OperationRunner terminal outcomes on
2026-08-09. Bridge/control-plane events remain in T02.

**What to build:** пользователь и разработчик получают единообразное измерение
полного времени обычной P/CP-операции с достаточным контекстом, чтобы отличить
задержку конкретной surface и ветки алгоритма без сохранения пользовательского
текста.

**Blocked by:** None - can start immediately.

- [x] Terminal event содержит build version и обезличенный environment label.
- [x] Event содержит `SurfaceKind`, confidence, context method и replacement method.
- [x] Event содержит profile, algorithm branch, P/CP, selection state, cold/warm и retry count.
- [x] Одинаковый набор полей записывается для completed, no-change, unsupported и failed outcomes.
- [x] Сохраняются phase timings от hotkey до полного восстановления служебного состояния.
- [x] Пользовательский текст и clipboard payload не добавляются в performance fields.
- [x] Существующие resolver, probe, policy и replacement contracts остаются зелеными.
- [x] Выбор method, timeout и runtime replacement behavior не изменяются.

Performance event пишется отдельной строкой `performance_operation_v1` в тот же
JSONL. Для сравнительного baseline обязательны `STEPLER_PERF_ENV=home-win11`
или `STEPLER_PERF_ENV=work-win11`; `unlabeled` считается диагностическим
режимом и исключается из snapshot.

## T02. Performance-телеметрия bridge paths

**Status:** implemented for PSReadLine, Xterm and SSH forwarding paths on
2026-08-09. Deferred PSReadLine layout repair is written as a separate event
with the same operation id and a dedicated branch. SSH forwarding remains
fail-closed and writes its own `SshRemote` / `ssh-remote-forwarded` event, so it
is kept separate from Qwen and PowerShell aggregation.

**What to build:** PSReadLine, Xterm и другие специальные control-plane paths
пишут те же измерения, что обычный OperationRunner, поэтому их задержку можно
сравнивать по одинаковым фазам.

**Blocked by:** T01. Единая performance-телеметрия OperationRunner.

- [x] PSReadLine path записывает correction plan, replacement, primary layout switch и delayed layout repair раздельно.
- [x] Xterm path записывает capture, apply, verify, retry и clipboard restore раздельно.
- [x] Bridge events используют те же outcome и context fields, что обычные операции.
- [x] Delayed layout repair не маскируется внутри неизвестной общей длительности.
- [x] SSH forwarding, Qwen terminal и PowerShell получают отдельные branch/surface labels и не смешиваются в performance aggregation.
- [x] Existing bridge/control-plane contracts остаются зелеными.

Bridge timing names are stored in `timings_ms[].phase`. The deferred repair
event uses branch `psreadline-delayed-layout-repair`; its measured duration is
not included in the primary PSReadLine operation duration.

## T03. Воспроизводимый baseline-отчет

**Status:** implemented on 2026-08-09. `stepler-cli performance-snapshot` builds
one deterministic snapshot from the current `performance_operation_v1` JSONL
schema and rejects mixed build versions.

**What to build:** разработчик получает фиксированный performance snapshot для
конкретной сборки и окружения, пригодный для сравнения до и после каждой
оптимизации.

**Blocked by:** T01. Единая performance-телеметрия OperationRunner; T02. Performance-телеметрия bridge paths.

- [x] Отчет группирует операции по build, environment, surface, method, profile, branch, P/CP и selection state.
- [x] Для каждой группы выводятся N, p50, p90, p95, max, failure rate и retry rate.
- [x] Cold и warm результаты не объединяются.
- [x] Snapshot отклоняет вход с несколькими `build_version`, поэтому разные сборки нельзя смешать.
- [x] Для каждого набора method/surface/branch/trigger/selection на environment явно выводится `sufficient`, `insufficient_sample` или `blocked_by_destructive_outcomes` с фактическими total warm, completed warm и cold N.
- [x] Отчет включает phase contribution и выделяет фазу-bottleneck.
- [x] Рабочий и домашний ПК представлены разными обезличенными environment labels.
- [x] Snapshot сохраняется отдельно от накопительного runtime log через обязательный `--output`.

Snapshot принимает только актуальный формат `timings_ms[].phase`; старые строки
с `timings_ms[].state` требуют нового запуска runner и не используются молча.
`failure_rate` считает только `RolledBackOrFailed`, а `retry_rate` - операции с
`retry_count > 0`; общий `N` включает все четыре terminal outcomes, но
`sufficient` требует 30 `Completed` warm и 5 cold операций. Остальные исходы
доступны в `outcome_counts`; `RolledBackOrFailed` дополнительно переводит
assessment в `blocked_by_destructive_outcomes`.

## T04. Ускорение WebKeyboard для FastBrowserEditor

**Status:** profiles/contracts частично реализованы, но performance acceptance
не подтвержден: нужен baseline текущей release-сборки.

**What to build:** P/CP в Codex, JIRA и Confluence выполняются быстро на
разрешенных FastBrowserEditor surfaces, при этом сохраняются clipboard, caret,
focus, переносы строк, таблицы и точный replacement range.

**Blocked by:** labeled baseline текущей release-сборки для целевой
FastBrowserEditor branch.

- [ ] Из baseline выбрана конкретная FastBrowserEditor branch с подтвержденным bottleneck.
- [ ] Retry запускается после наблюдаемого отрицательного результата, а не только после фиксированной паузы.
- [ ] Раннее завершение verify допускается только при положительном доказательстве примененной замены.
- [ ] Preflight, clipboard restore и focus/caret restore не удалены.
- [ ] Codex Windows app проходит P и CP с selection и без selection.
- [ ] JIRA проходит P и CP без удаления строки и вставки clipboard.
- [ ] Confluence проходит P и CP внутри обычного текста и таблицы без изменения соседних блоков.
- [ ] Clipboard с изображением сохраняется.
- [ ] На каждой surface собрано не меньше 30 warm операций.
- [ ] Итоговые FastBrowserEditor показатели соответствуют p50/p95 budget либо документирован безопасный platform floor.

## T05. Ускорение WebKeyboard для Telegram и standard browser surfaces

**What to build:** проверенный быстрый WebKeyboard path работает на Telegram,
WhatsApp и обычных browser-like surfaces без переноса FastBrowser-specific
допущений на неподходящие приложения.

**Blocked by:** T04. Ускорение WebKeyboard для FastBrowserEditor.

- [ ] Standard и Telegram profiles имеют отдельные allowlist и branch metrics.
- [ ] Telegram Desktop сохраняет текущую строку, focus, caret и clipboard.
- [ ] WhatsApp/browser-like surface сохраняет корректное выделение и replacement range.
- [ ] Быстрый FastBrowser branch не включается только по общему классу Chromium/Qt окна.
- [ ] Negative contracts запрещают случайное применение branch к неподдерживаемой surface.
- [ ] На каждой затронутой surface собрано не меньше 30 warm операций.
- [ ] Итоговые показатели соответствуют p50/p95 budget либо документирован безопасный platform floor.

## T06. Изолированный UIA worker: read-only capture parity

**What to build:** UI Automation context читается через долгоживущий
изолированный STA worker без запуска нового PowerShell process на каждую
операцию, а результат совпадает с существующим проверенным capture path.

**Blocked by:** labeled baseline текущей release-сборки для целевой UIA
surface.

- [ ] Worker имеет request id, bounded deadline, health check и restart после crash/timeout.
- [ ] Worker не выполняется на tray UI thread.
- [ ] Первый tracer bullet реализует только read-only capture.
- [ ] Parity tests сравнивают text, caret, selection, runtime identity и writable capability старого и нового paths.
- [ ] Stale response не применяется к новой операции.
- [ ] Timeout завершается fail-closed без keyboard/clipboard fallback.
- [ ] Второй request после worker restart выполняется успешно.
- [ ] Resolver и surface policy не меняются.
- [ ] Измерено отдельно cold и warm capture time.

## T07. UIA worker: EditableText/Text replacement

**What to build:** Rocket.Chat search и другие разрешенные editable UIA
surfaces выполняют capture и replacement через изолированный worker, быстро и
без потери focus/caret.

**Blocked by:** T06. Изолированный UIA worker: read-only capture parity.

- [ ] EditableText и Text apply используют request identity из соответствующего capture.
- [ ] Перед записью проверяются target identity и expected text.
- [ ] Rocket.Chat search работает с selection и без selection.
- [ ] Focus остается в исходном поле после P/CP.
- [ ] Worker timeout не включает запрещенный WebKeyboard/SendInput fallback.
- [ ] Existing Rocket.Chat resolver contracts остаются зелеными.
- [ ] Собрано не меньше 30 warm операций на затронутую surface.
- [ ] Итоговые показатели соответствуют p50/p95 budget либо документирован безопасный platform floor.

## T08. UIA worker: DocumentText replacement

**What to build:** Sticky Notes и другие разрешенные document UIA surfaces
используют быстрый изолированный capture/apply path с сохранением структуры
документа и позиции пользователя.

**Blocked by:** T07. UIA worker: EditableText/Text replacement.

- [ ] DocumentText capture/apply переведен без расширения surface allowlist.
- [ ] Sticky Notes работает с selection и без selection.
- [ ] Сохраняются переносы строк, соседние абзацы и текст вне replacement range.
- [ ] Caret возвращается в ожидаемое исходное логическое положение.
- [ ] Clipboard с текстом и изображением не изменяется.
- [ ] Timeout/crash завершается fail-closed и worker восстанавливается.
- [ ] Собрано не меньше 30 warm операций на Sticky Notes.
- [ ] Итоговые показатели соответствуют p50/p95 budget либо документирован безопасный platform floor.

## T09. Ускорение PSReadLine bridge

**Status:** telemetry готова; runtime-оптимизация не начата и ожидает
отдельные labeled baseline для standalone и embedded PowerShell.

**What to build:** локальный PowerShell и PowerShell внутри Codex быстро
исправляют строку и переключают раскладку, не блокируя ввод повторными
control-plane process calls.

**Blocked by:** labeled baseline текущей release-сборки для standalone и
embedded PowerShell.

- [ ] Correction plan, PSReadLine replacement и primary layout switch измеряются раздельно.
- [ ] Повторные синхронные CLI-запуски удалены из critical path либо объединены в один persistent request.
- [ ] Delayed layout repair выполняется после текстовой операции и логируется отдельно.
- [ ] Локальное отдельное окно PowerShell проходит P и CP.
- [ ] Embedded PowerShell в Codex проходит P и CP без появления `^C`.
- [ ] PowerShell с SSH не получает generic local path без remote helper marker.
- [ ] Layout после успешной конвертации соответствует replacement language.
- [ ] Собрано не меньше 30 warm операций для отдельного и embedded PowerShell.
- [ ] Итоговые показатели соответствуют p50/p95 budget либо документирован безопасный platform floor.

## T10. Изолированный WordCom worker: capture parity

**What to build:** Word и Outlook WordEditor читаются через отдельный
долгоживущий STA COM worker, не подвергая tray process риску зависания Office.

**Blocked by:** T06. Изолированный UIA worker: read-only capture parity.

- [ ] WordCom использует отдельный process от tray и UIA worker state.
- [ ] Worker protocol переиспользует только проверенный transport/deadline contract.
- [ ] Capture parity подтверждена отдельно для Word document и Outlook Inspector WordEditor.
- [ ] Document/Inspector identity проверяется на каждом request.
- [ ] COM object не используется после закрытия или смены документа.
- [ ] Hang/timeout приводит к остановке worker, а не Word/Outlook или tray.
- [ ] После restart следующий capture выполняется успешно.
- [ ] Измерено отдельно cold и warm capture time.

## T11. WordCom worker: replacement и защита от зависаний

**What to build:** Word 2016 и Outlook 2016 compose быстро и безопасно
применяют P/CP через изолированный COM worker, сохраняя caret и переключение
раскладки.

**Blocked by:** T10. Изолированный WordCom worker: capture parity.

- [ ] Apply проверяет expected range и active Document/Inspector identity.
- [ ] Caret сохраняется при замене слова слева и внутри строки.
- [ ] После успешной конвертации переключается системная раскладка.
- [ ] Word 2016 проходит повторные P/CP без зависания.
- [ ] Outlook 2016 compose проходит повторные P/CP с Zimbra Connector.
- [ ] Outlook search продолжает использовать собственный быстрый Win32 path.
- [ ] Timeout не включает SendInput или clipboard fallback.
- [ ] Закрытие документа и смена Inspector не оставляют stale COM state.
- [ ] Собрано не меньше 30 warm операций для Word и Outlook compose.
- [ ] Итоговые показатели соответствуют p50/p95 budget либо документирован безопасный platform floor.

## T12. Новый baseline Xterm/Qwen Terminal

**What to build:** Qwen Terminal получает отдельную достоверную оценку
задержки, не смешанную с Stepler Qwen Input и Qwen Workspace.

**Blocked by:** labeled baseline текущей release-сборки для Qwen Terminal.

- [ ] Qwen Terminal, Qwen Input и Qwen Workspace имеют разные surface/control-plane labels.
- [ ] Собрано не меньше 30 warm и 5 cold Xterm операций.
- [ ] Отдельно измерены P, CP, selection и no-selection.
- [ ] Подтверждено отсутствие Ctrl+C/Ctrl+Shift+C shortcut в Qwen path.
- [ ] Зафиксированы p50, p95, failure rate, retry rate и phase contribution.
- [ ] При p50 не больше 300 ms T13 закрывается как не требующая реализации.
- [ ] При p50 больше 300 ms bottleneck и разрешенная branch явно переданы в T13.

## T13. Ускорение XtermKeyboardSelection

**What to build:** если T12 подтвердил задержку выше бюджета, Qwen Terminal
получает быстрый event-driven Xterm path без interrupt shortcut и без
смешивания с другими Qwen surfaces.

**Blocked by:** T12. Новый baseline Xterm/Qwen Terminal.

- [ ] Изменяется только подтвержденная T12 branch.
- [ ] Clipboard acknowledgement или другой ранний сигнал заменяет ожидание только при доказанном результате.
- [ ] Ctrl+C и Ctrl+Shift+C не отправляются в Qwen.
- [ ] Qwen process не завершается после P/CP.
- [ ] Qwen Input и Qwen Workspace не меняют поведение.
- [ ] Собрано не меньше 30 warm операций.
- [ ] Итоговые показатели соответствуют p50/p95 budget либо документирован безопасный platform floor.

## T14. Итоговый performance и regression gate

**What to build:** перед релизом пользователь получает подтвержденное снижение
задержки на всех затронутых surfaces, а команда получает единый отчет о
скорости, корректности и известных platform floors.

**Blocked by:** T05. Ускорение WebKeyboard для Telegram и standard browser surfaces; T08. UIA worker: DocumentText replacement; T09. Ускорение PSReadLine bridge; T11. WordCom worker: replacement и защита от зависаний; T13. Ускорение XtermKeyboardSelection; T15. Outlook-only безопасное переключение раскладки; T16. Глобальный concurrent layout coordinator.

- [ ] Для каждого method/surface приведены before/after N, p50, p95, failure rate и retry rate.
- [ ] Все resolver, probe, policy, replacement и clipboard contracts зелены.
- [ ] Выполнен manual smoke Codex, JIRA, Confluence, Telegram, Rocket.Chat search, Sticky Notes, Word 2016, Outlook 2016, PowerShell и Qwen Terminal.
- [ ] Не зафиксировано удаления соседнего текста, вставки clipboard, потери изображения, focus или caret.
- [ ] `Win32EditMessages` p95 не ухудшился более чем на 10 ms.
- [ ] Не достигшие 300/600 ms methods имеют измеренный и объясненный безопасный platform floor.
- [ ] Performance snapshot привязан к release build version.
- [ ] Release checklist содержит результаты и оставшиеся ограничения.

## T15. Outlook-only безопасное переключение раскладки

**Status:** реализация готова к ручному Outlook smoke; автоматические контракты
должны оставаться зелеными до закрытия задачи.

**What to build:** Outlook 2016 с Zimbra Connector сохраняет left/right `Ctrl`
как единый пользовательский механизм Stepler, но не получает прямой
`WM_INPUTLANGCHANGEREQUEST`. Для `P/CP` системный layout dispatch начинается
перед replacement, а verification идет параллельно с текстовой операцией.

**Source:** `outlookhaging.md`, раздел «Актуальная спецификация: Outlook-only
переключение раскладки» и dump
`F:\distr\system\outlook_diag\OUTLOOK_hang_19712_20260824_105915.dmp`.

- [x] Outlook классифицируется в отдельный system-hotkey transport.
- [x] Outlook transport не имеет fallback к прямому window message.
- [x] Non-Outlook applications сохраняют прежний layout transport.
- [x] `P/CP` вызывает layout dispatch между preflight и replacement apply.
- [x] Foreground, focus и Outlook process identity проверяются до dispatch и verification.
- [x] Layout failure не отменяет успешную текстовую замену и имеет отдельный overlay result.
- [ ] Перезапущен зависший Outlook после сохранения dump.
- [ ] Outlook compose (`_WwG`): 30 повторов P, CP, left Ctrl и right Ctrl без hang.
- [ ] Outlook search (`RICHEDIT60W`): 30 повторов P, CP, left Ctrl и right Ctrl без hang.
- [ ] После P/CP target layout подтверждается без заметной задержки.
- [ ] В `hotkey_signal.log` нет Outlook `layout_post ... WM_INPUTLANGCHANGEREQUEST` path.
- [ ] При искусственной смене focus текст сохраняется, layout branch завершается fail-closed.

## T16. Глобальный concurrent layout coordinator

**Status:** реализация и автоматические контракты готовы; требуется ручной
cross-surface smoke.

**What to build:** после P/CP смена раскладки не выполняется отдельной
последовательной операцией. Layout dispatch начинается после preflight, а
verification идет параллельно replacement для всех поддерживаемых surfaces.

- [x] Outlook сохраняет `system_hotkey`, остальные surfaces - `window_message`.
- [x] После dispatch foreground повторно проверяется до replacement apply.
- [x] Layout worker проверяет исходные foreground/focus/PID перед verify и retry.
- [x] Layout failure не откатывает успешную текстовую замену и виден в overlay.
- [x] Старый hotkey path `control action -> sleep -> window -> foreground` не вызывается после replacement.
- [ ] Smoke: Notepad, Codex/ChatGPT, JIRA/Confluence, Sticky Notes, Word/Outlook.
- [ ] Для каждой поверхности layout подтверждается не позже завершения операции либо возвращается явный partial result.

## T17. Поддержка вариантов английской раскладки `en-*`

**Status:** ready-for-agent. Спецификация: `docs/english_layout_variants_spec_ru.md`.

**What to build:** Stepler находит установленную английскую раскладку по
семейству `en-*`, включая `en-US` и `en-GB`, и одинаково использует её для
обычного переключения раскладки и terminal clipboard fallback. При отсутствии
английской раскладки операция завершается безопасным `Unsupported`.

**Blocked by:** None — can start immediately.

- [ ] `en-US` (`0x0409`) продолжает распознаваться.
- [ ] `en-GB` (`0x0809`) распознаётся как английская целевая раскладка.
- [ ] Другие варианты `en-*` распознаются по primary language bits Windows
  LANGID.
- [ ] Русская раскладка (`0x0419`) не выбирается как английская.
- [ ] При нескольких английских раскладках выбор детерминирован и следует
  порядку Windows.
- [ ] Обычный layout switcher и terminal clipboard fallback используют общий
  resolver.
- [ ] При отсутствии `en-*` сохраняется `Unsupported` без выбора другой
  раскладки.
- [ ] Добавлены unit-тесты и выполнен ручной smoke с установленной `en-GB`.

