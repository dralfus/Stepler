# Спецификация: измерение и ускорение P/CP

**Статус:** активная спецификация производительности
**Актуально на:** 2026-09-21
**Область:** задержка от получения P/CP до безопасной замены текста,
восстановления буфера и запуска переключения раскладки.

## Цель

Оптимизировать только измеренный bottleneck конкретного метода и поверхности,
не ослабляя preflight, проверку диапазона, восстановление буфера, focus/caret
restore и fail-closed поведение.

Ориентир для интерактивного безопасного пути: `p50 <= 300 ms`, `p95 <= 600 ms`.
Если поверхность не может достичь этого без риска, фиксируется измеренный
platform floor и причина, а не включается рискованный fallback.

## Единственный измерительный путь

`performance_operation_v1` в runtime JSONL агрегируется командой
`stepler-cli performance-snapshot`. Сравнивать можно только серии с одинаковыми:

- версией и путём реально запущенной сборки;
- `STEPLER_PERF_ENV`;
- surface, method, profile и algorithm branch;
- P/CP, selection state и cold/warm состоянием.

Для решения об оптимизации нужны не менее 30 успешных warm и 5 cold операций
в каждой целевой серии без destructive outcomes. `unlabeled` события и единичные
ручные проверки полезны для диагностики, но не для p50/p95 acceptance.

Для накопления реальной пользовательской статистики предусмотрен отдельный
`stepler-cli performance-report`. Он включает `unlabeled` события, группирует
их по build, безопасному `application_id`, surface, adapter, effective profile,
algorithm branch, P/CP и selection state и не применяется как release acceptance. Новые события не содержат
заголовок окна, пользовательский текст или путь документа.

## Подтвержденные результаты

| Surface / маршрут | Историческое состояние | Текущее состояние | Статус |
|---|---|---|---|
| WebKeyboard, общий browser path | build `1.0.20260828.t1708`: P p50 около 2923 ms, CP p50 около 3051 ms; `Verified` около 2105 ms | semantic clipboard equality убрала штатный двухсекундный timeout | реализовано, нужна валидная серия |
| ChatGPT desktop, WebKeyboard без selection | не выделялся отдельный быстрый apply path | build `1.0.20260911.t2121`: fast context дошел до apply; build `1.0.20260912.t1034`: для этого маршрута guard ждёт 50 + 120 ms вместо 80 + 250 ms | пользователь вручную подтвердил улучшение, acceptance не собран |
| ChatGPT desktop, selection | порядка 536-662 ms в диагностических событиях `t2121` | сохраняет осторожный selected preflight и общий guard | не оптимизировать без отдельного baseline |
| Codex embedded PowerShell | до оптимизации несколько UIA проверок маршрута добавляли сотни миллисекунд | handoff создаётся на keydown, keyup и handler используют токен до 750 ms; relay в диагностике около 61-80 ms, PSReadLine обычно около 190-211 ms | пользователь подтвердил комфортную работу; нужна серия |
| UIA / WordCom / Qwen Xterm | единичные старые измерения | достаточных labeled серий нет | не менять до baseline |

Числа из таблицы не являются сопоставимым итоговым benchmark, пока не набраны
требуемые выборки. Они объясняют только уже сделанные узкие изменения.

## Реализованные safety-инварианты

1. Равенство буфера определяется полезным текстом и поддерживаемыми форматами,
   а не Windows `sequence_number`.
2. Снимок пользовательского буфера берется до capture, поэтому временный
   `__STEPLER_COPY_MARKER_*` не становится донором восстановления.
3. Ускоренный guard применяется только к control id
   `web-keyboard-fast-chatgpt-*`; все остальные методы используют стандартное
   окно стабилизации.
4. ChatGPT fast profile ограничен заголовком `ChatGPT`; Jira, Confluence и
   обычные browser-like surfaces не получают его по одному классу Chromium.
5. Embedded terminal получает passthrough только после подтверждения Codex
   terminal surface; SSH и Qwen сохраняют собственные fail-closed маршруты.

## Порядок дальнейшей работы

1. Для активной release-сборки задать `STEPLER_PERF_ENV=work-win11` или
   `home-win11`, собрать отдельные 30 warm / 5 cold серии ChatGPT P и CP с
   selection и без него, затем сохранить snapshot через `--output`.
2. Зафиксировать итог fast ChatGPT/Codex branch по фактическим p50/p95. Отдельно
   проверить, что telemetry записывает effective Fast profile для этого маршрута.
3. Собрать отдельные series для standalone и embedded PSReadLine; менять только
   фазу, которая остается dominant после handoff-оптимизации.
4. До любых UIA worker, WordCom worker или Qwen Xterm изменений получить
   surface-specific baseline и manual smoke. Эти задачи не являются следствием
   ускорения WebKeyboard.
5. Перед релизным performance gate проверить ChatGPT/Codex, Jira, Confluence,
   Telegram, Rocket.Chat search, Sticky Notes, Word, Outlook, PowerShell и Qwen
   по их собственным контрактам.

## Вне области этой спецификации

- изменение семантики P/CP, classifier, resolver или surface policy без
  отдельного контракта;
- удаление safety phase только ради меньшей цифры latency;
- рискованная оптимизация Outlook/Word COM до отдельного safety gate;
- перенос вывода одного адаптера на другой.

## Связанные спецификации

- `docs/browser_surface_latency_optimization_spec_ru.md` фиксирует отдельную
  telemetry и оптимизацию ChatGPT/Codex, Jira, Confluence и generic Firefox.
