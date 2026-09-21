# Спецификация: изолированное ускорение browser surfaces

## Problem Statement

P/CP в ChatGPT/Codex, Jira и Confluence работают через WebKeyboard, но их
задержки и bottleneck различаются. В текущем накопительном отчёте Firefox
показывается как один `FastBrowserEditor`, поэтому Jira и Confluence могут быть
смешаны в одной серии. Из-за этого нельзя безопасно менять capture, apply или
verify для одной web-поверхности: изменение может ухудшить другую, включая
таблицы Confluence и редактор Jira.

Наблюдения на 2026-09-21 для build `1.0.20260921.t1309`:

- ChatGPT/Codex P без selection: `n=15`, `p50=839 ms`, `p95=1092 ms`;
  dominant phase `ContextCaptured` (42%).
- Firefox FastBrowser P без selection: `n=26`, `p50=1153 ms`, `p95=1321 ms`;
  dominant phase `ContextCaptured` (40%).
- Firefox FastBrowser CP без selection: `n=9`, `p50=2004 ms`, `p95=2215 ms`;
  dominant phase `ReplacementApplied` (45%).

Последние две серии не доказывают, какая из Jira или Confluence является
источником задержки. Они являются основанием для измерительного разделения, но
не для общей Firefox-оптимизации.

## Solution

Каждое performance event получает стабильный безопасный
`performance_surface_id`, вычисленный из уже подтверждённых surface facts.
`performance-report` включает этот идентификатор в ключ группировки. Для
целевых web-поверхностей используются отдельные значения `chatgpt_codex`,
`jira`, `confluence` и `firefox_generic`.

После накопления независимых series Stepler оптимизирует только dominant phase
конкретной `performance_surface_id` и algorithm branch. ChatGPT/Codex P без
selection может получить узкое ускорение capture. Jira и Confluence получают
разные изменения только после собственных baseline; generic Firefox не
получает Jira/Confluence-specific допущений.

## User Stories

1. Как пользователь ChatGPT/Codex, я хочу видеть latency именно этого
   редактора, чтобы его оптимизация не зависела от других Chromium surfaces.
2. Как пользователь Jira в Firefox или Chromium, я хочу, чтобы измерения Jira
   не смешивались с Confluence, чтобы изменения сохраняли точный range и
   форматирование задачи.
3. Как пользователь Confluence, я хочу, чтобы оптимизация не удаляла таблицы,
   соседние блоки или переносы строк ради меньшей задержки.
4. Как пользователь Firefox на другой web-странице, я хочу, чтобы Jira и
   Confluence-specific быстрые пути не применялись ко мне случайно.
5. Как пользователь, я хочу, чтобы telemetry не сохраняла заголовок вкладки,
   URL, текст, clipboard или имя документа, чтобы измерение не раскрывало
   содержимое работы.
6. Как разработчик, я хочу видеть P/CP, selection state, effective profile и
   algorithm branch в отдельной группе поверхности, чтобы выбирать реальный
   bottleneck вместо общего timeout.
7. Как пользователь, я хочу, чтобы оптимизация ChatGPT/Codex касалась только
   подтверждённого fast P no-selection route и не меняла Jira, Confluence,
   Firefox или Rocket.Chat.
8. Как пользователь, я хочу, чтобы раннее завершение capture/apply/verify
   происходило только после положительного доказательства результата, чтобы
   P/CP не вставлял буфер и не терял caret/focus.
9. Как пользователь, я хочу продолжать пользоваться приложениями в обычном
   режиме вместо принудительных серий ручных повторов, чтобы статистика
   отражала реальное использование.
10. Как разработчик, я хочу, чтобы при недостаточной series новая оптимизация
    не выполнялась, а статус оставался `waiting-for-telemetry`.

## Implementation Decisions

- Использовать один новый telemetry seam: `performance_surface_id`. Он
  определяется из существующих classification/surface facts и не принимает
  пользовательский title или текст как отдельный telemetry payload.
- Идентификатор входит в runtime event, parser и ключ `performance-report`.
  `performance-snapshot` сохраняет свой строгий baseline contract и меняется
  только если для согласованности ему также нужен этот ключ.
- Новые значения ограничены известным allowlist. Неизвестная browser surface
  получает `firefox_generic` только при подтверждённом Firefox browser route;
  иначе сохраняется существующая консервативная surface classification.
- Механизм P/CP, resolver, policy, clipboard guard, capture и replacement не
  меняются в измерительной задаче.
- ChatGPT/Codex ускоряется только в fast P no-selection branch и только после
  достаточной series текущей release-сборки. Первым кандидатом является
  `ContextCaptured`.
- Jira и Confluence оптимизируются отдельными vertical slices. Выбор между
  capture и replacement/apply делается по dominant phase их собственной
  series, а не по общему Firefox результату.
- Целевой ориентир остаётся `p50 <= 300 ms` и `p95 <= 600 ms`. При
  невозможности безопасно достичь его фиксируется measured platform floor.

## Testing Decisions

- Добавить unit-тесты event/report boundary: одинаковые build, method,
  trigger, selection, profile и branch с разными `performance_surface_id`
  образуют разные report groups.
- Добавить tests classification boundary: Jira, Confluence, ChatGPT/Codex и
  generic Firefox получают ожидаемые идентификаторы; unknown surface не
  получает browser-specific идентификатор.
- Проверить privacy contract сериализации: event и report не содержат title,
  URL, пользовательский текст, clipboard или document path.
- Использовать существующие resolver/probe/policy контракты и replacement
  behavior tests как regression boundary для capture/apply изменений.
- Для каждого runtime-оптимизационного slice сначала написать RED-тест на
  диапазон, caret, focus, clipboard и fail-closed result, затем минимально
  изменить подтверждённую branch.
- После каждого slice выполнить полный Rust test suite и targeted manual
  smoke соответствующей поверхности.

## Out of Scope

- Глобальное уменьшение timeout или retry для всех WebKeyboard surfaces.
- Перенос Jira/Confluence fast path на generic Firefox, Telegram, Rocket.Chat
  или другие browser-like приложения.
- Изменение P/CP семантики, classifier/resolver policy или surface allowlist
  ради telemetry.
- Сбор title, URL, пользовательского текста, clipboard или document path.
- UIA worker, WordCom worker, Xterm и PSReadLine оптимизации.

## Further Notes

Накопительный `performance-report` служит для выбора bottleneck; строгий
`performance-snapshot` остаётся release acceptance инструментом. До появления
не менее 30 warm и 5 cold completed операций по конкретной current-build
series runtime-изменение не начинается. Исключение — измерительный
`performance_surface_id`, поскольку он не изменяет P/CP behavior.
