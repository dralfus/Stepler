# Скорость P/CP: статус и план оптимизации

Дата ревизии: 2026-08-22.

Это единственный рабочий документ по задержкам P/CP. Он заменяет прежний
`pcp_latency_baseline_ru.md`: исторические значения сохранены ниже только как
точка сравнения, а новый baseline всегда строится из telemetry одной release
сборки и одного окружения.

## Короткий статус

- Телеметрия обычного pipeline и bridge paths реализована: T01 и T02 закрыты.
- `stepler-cli performance-snapshot` реализован и не позволяет смешать builds:
  T03 закрыт.
- Для release `1.0.20260821.t2216` нет валидного набора измерений: текущие
  события имеют `environment_label=unlabeled`, а одной или нескольких
  операций недостаточно для P50/P95.
- Поэтому нельзя честно утверждать, что какая-либо последняя оптимизация уже
  улучшила или ухудшила конкретный adapter. Ручное ощущение скорости полезно
  как сигнал, но не заменяет snapshot.

Цель для локальных операций: `p50 <= 300 ms`, `p95 <= 600 ms`. Исключение
возможно только при зафиксированном безопасном platform floor.

## Реестр adapters

`Исторический baseline` - снимок 2026-07-26 из смешанного лога. Он годится
только для порядка приоритетов. `Последнее наблюдение` - необязательный
ориентир из новой telemetry; оно не является текущим результатом, пока не
набраны минимум 30 успешных warm и 5 cold операций для одной
build/environment/surface/branch группы.

| Adapter (проверенные приложения) | Исторический baseline P50/P95, ms | Последнее наблюдение P50/P95, ms | Что уже сделано | Улучшение относительно baseline | Следующий шаг / статус |
|---|---:|---:|---|---|---|
| `win32_edit_messages` (Notepad, Outlook search, Excel edit control) | 22 / 46, N=29 | 7 / 30, N=6, `t1721` | Контрольная быстрая ветка; новых speed-изменений не требуется | Наблюдаемо -15 / -16 ms, но выборка мала | **Контрольная группа.** Собрать 30/5 в `t2216`; не допустить роста P95 более чем на 10 ms |
| `web_keyboard_selection` Fast/standard (Codex Windows app, ChatGPT web, JIRA, Confluence, Telegram, WhatsApp) | 559 / 2858, N=751, смешанные branches | `t1146`: 2242-2848 / 2262-2882, N=6 на branch | Добавлены раздельные Fast/Standard/Rocket timing profiles и surface contracts; безопасный verify сохранен | Не подтверждено: текущая `t2216` выборка отсутствует | **Первый приоритет.** Собрать 30/5 по каждой реальной surface и branch; затем менять только доказанный bottleneck, обычно `Verified` |
| `uia_editable_text` / `uia_text` (Rocket.Chat search, Windows Settings, WPF UIA fixture) | 1311 / 2015, N=106 | 1471 / 1471, N=2, `t1130` | Только telemetry и contracts; persistent UIA worker не реализован | Не подтверждено | Собрать labeled baseline. При подтверждении задержки начать read-only STA worker parity, не apply |
| `uia_document_text` (Sticky Notes) | 1125 / 1402, N=41 | Нет новой telemetry | Исправлены contracts caret/range и переносов; speed-работа не начата | Не подтверждено | Собрать 30/5 Sticky Notes; затем отдельный UIA DocumentText worker, только после parity EditableText |
| `word_com` (Word 2016, Outlook 2016 compose) | 1792 / 2534, N=24 | 1535 / 1535, N=2, `t1721` | Текстовый path сохранен; добавлены Outlook/Zimbra hang-safety ограничения для layout switching | Наблюдаемо -257 / -999 ms, но сравнение недействительно при N=2 | Собрать 30/5 отдельно для Word и Outlook. До этого не переносить COM в tray и не сокращать timeout |
| `psreadline` bridge (локальный PowerShell, PowerShell в Codex) | 1005 / 2598, N=63 | 496 / 496, N=1 cold, `t2216` | Телеметрия primary и delayed layout repair разнесена; bridge contracts закреплены | Не подтверждено | Собрать 30/5 для standalone и embedded PowerShell. Если P50 >300, убрать оставшиеся синхронные control-plane calls из critical path |
| `xterm_keyboard_selection` (Qwen Terminal; не Qwen Input) | 623 / н/д, N=3 | Нет новой telemetry | Qwen имеет отдельные labels и запрет interrupt shortcuts | Не подтверждено | Сначала baseline 30/5. При P50 <=300 закрыть без кода; иначе оптимизировать только подтвержденную branch |
| `console_buffer` (classic `cmd.exe`) | 108 / н/д, N=26 | Нет новой telemetry | Производительность приемлема, функциональная стабильность CP нет | Не применяется | **Вне latency-плана.** Отдельная cmd-задача, не ускорять рискованными fallback |
| Risky fallback (`terminal_clipboard_shortcut`, `clipboard_selection`, `send_input`) | н/д | н/д | Ограничены policy/allowlist | Не применяется | Не оптимизировать: сначала безопасность и явный surface contract |

Значения с `N < 30` не используются для решения о регрессии, даже если они
выглядят лучше исторического baseline. Диапазон у WebKeyboard отражает разные
ветки, а не один усредненный метод.

## Что измерять сейчас

Сначала создать валидный snapshot для **одной** release-сборки и **одного**
окружения (`home-win11` или `work-win11`). Успех сбора:

1. В `sample_assessments` нет `unlabeled`.
2. Для target branch есть 30 `Completed warm` и 5 cold событий.
3. Нет `RolledBackOrFailed`; иначе сначала расследовать безопасность, а не
   сокращать задержки.
4. В этой таблице обновлены `Последнее наблюдение`, `Улучшение` и
   `Следующий шаг` только по готовому snapshot.

Точная команда сбора и фильтрации смешанного JSONL находится в
[`development_commands_ru.md`](development_commands_ru.md#воспроизводимый-performance-snapshot).

## Порядок следующих работ

1. **Собрать labeled baseline `t2216`.** Это текущий frontier; без него
   нельзя доказать эффект уже сделанных profile-ускорений.
2. **WebKeyboard Fast/Standard.** Выбрать одну branch с `p50 > 300 ms` и
   наибольшим phase contribution. Не сокращать verify без положительного
   подтверждения примененной замены; сохранить preflight, clipboard и
   caret/focus restore.
3. **PSReadLine.** Работать только если отдельные snapshot standalone и
   embedded PowerShell покажут задержку выше бюджета. Delayed layout repair не
   должен блокировать текстовую операцию.
4. **UIA family.** Начать с read-only persistent STA worker и parity capture;
   apply добавлять отдельным шагом. Worker crash/timeout должен fail-closed.
5. **WordCom.** Только после стабильного Office smoke и накопленного
   baseline. COM worker обязан быть отдельным процессом; tray process не
   выполняет COM call.
6. **Xterm.** Решение только после отдельного Qwen Terminal baseline.

## Неподвижные ограничения

- Нельзя ускорять за счет удаления preflight, clipboard restore, проверки
  expected text, caret/focus restore или запрета unsafe fallback.
- После preflight layout dispatch и replacement запускаются как единая
  конкурентная операция: verification идет параллельно apply, а повторная
  проверка foreground перед apply и snapshot HWND/focus/PID в layout worker
  защищают от записи в другой контрол.
- Конкурентная оркестрация едина для P/CP, но transport остается
  surface-specific; Outlook-only system hotkey запрещено переносить на другие
  приложения.
- Нельзя смешивать home/work ПК, builds, surface kinds, profiles или algorithm
  branches в одном сравнении.
- `P` и `CP`, selection и no-selection учитываются раздельно.
- Outlook/Word проверяются отдельным ручным smoke; отсутствие зависания не
  доказывается synthetic unit test.
- SSH network latency не входит в локальный бюджет 300/600 ms.

## Состояние задач

| Задача | Состояние |
|---|---|
| T01, telemetry OperationRunner | Выполнено |
| T02, telemetry bridge paths | Выполнено |
| T03, reproducible snapshot | Выполнено |
| T04-T05, WebKeyboard optimization | Частично реализованы profiles/contracts; acceptance заблокирован fresh baseline |
| T06-T08, UIA worker | Не начаты |
| T09, PSReadLine critical-path optimization | Не начата; telemetry готова |
| T10-T11, WordCom worker | Не начаты; повышенный Office risk |
| T12-T13, Xterm/Qwen Terminal | Ожидают baseline |
| T14, итоговый release gate | Не начат |
