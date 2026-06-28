# План стабилизации Stepler

Дата ревизии: 2026-06-26

Статус: архитектурная стабилизация после S1/S3/S5/S6 закрыта. Новые фазы не
планируются без конкретной повторяющейся регрессии или failing contract test.

## Короткий вывод

Архитектура адаптеров уже не хрупкая в старом смысле. Основные runtime-границы
теперь разделены:

- `TargetFacts` описывает признаки активного окна;
- `SurfaceKind` классифицирует поверхность;
- `ProbePolicy` решает, какие методы можно пробовать;
- `SurfacePolicy` решает, какие методы resolver может выбрать для `P`/`CP`;
- Windows adapters отвечают за техническую возможность capture/apply;
- `TextContext.capabilities.method_binding` запрещает replacement-слою
  угадывать adapter;
- `Unknown` работает conservative и не получает generic/risky fallback без
  явной diagnostic-настройки.

Старые документы `adapter_architecture_stability_review_ru.md` и
`adapter_isolation_hardening_plan_ru.md` удалены: фазы 10-13 выполнены, а
хранить подробный исторический план рядом с актуальными правилами уже вредно.

## Что сейчас считается источником правды

- `crates/stepler-platform/src/surface.rs` - `SurfaceKind`, `ProbePolicy`,
  `SurfacePolicy`, classifier и profile routing.
- `crates/stepler-platform/src/target_facts.rs` - признаки target.
- `crates/stepler-platform/src/resolver.rs` - выбор context/replacement method.
- `crates/stepler-platform/tests/fixtures/probe_contracts.tsv` - contract
  probe/classifier stack для проверенных поверхностей.
- `crates/stepler-platform/tests/fixtures/resolver_contracts.tsv` - contract
  resolver choices и forbidden methods.
- `crates/stepler-platform/tests/policy_invariants.rs` - общие policy
  инварианты.
- `crates/stepler-platform-windows/src/tests.rs` - Windows runtime boundaries,
  keyboard control и adapter-specific protection.
- `crates/stepler-core/tests/replacement_behavior.rs` - range/caret/selection
  поведение.
- `docs/development_commands_ru.md` - команды и правила разработки.
- Этот файл - итоговый статус стабилизации и правила сопровождения.

## Что больше не хранить в проекте

- Завершенные phase-by-phase планы, если они уже перенесены в код, tests и этот
  документ.
- Временные orchestration/model-routing документы, не связанные с runtime или
  developer workflow Stepler.
- Tool-specific правила локального агента, если они не являются частью
  поддерживаемого процесса проекта.
- Дублирующие architecture review файлы, которые описывают уже закрытое
  состояние.

## Итог после S1/S3/S5/S6

Активных архитектурных фаз больше нет. Цель "не хрупко, но без
over-engineering" достигнута за счет контрактов и явного routing:

- S1 закрыл рассинхрон `ProbePolicy` / `SurfacePolicy` инвариантами;
- S3 закрепил bridge/control-plane methods явным allowlist;
- S5 зафиксировал маршрут добавления нового приложения или surface;
- S6 отделил adapter contracts от продуктового smoke.

Дальнейшие изменения должны идти не через новые слои архитектуры, а через
поддерживающие правила: конкретный баг -> точечный contract/negative case;
новое приложение -> чеклист из `docs/development_commands_ru.md`; release или
tray/Qwen/installer изменение -> smoke checklist.

## Завершенные и условные пункты

### S1. Усилить policy invariants - выполнено

Цель: сделать так, чтобы расхождение `ProbePolicy` и `SurfacePolicy` ломало
тесты сразу, а не проявлялось в Jira/Rocket/Outlook через неделю.

Сделано:

- В `crates/stepler-platform/tests/policy_invariants.rs` проверка forbidden
  methods расширена с replacement preferences на context + replacement
  preferences.
- Добавлена проверка, что `default_probe_policies()` не содержит дублей
  `SurfaceKind`.
- Добавлена проверка, что `default_surface_policies()` не содержит дублей
  `SurfaceKind`.
- Matching policy test теперь проверяет обе стороны:
  - каждый `ProbePolicy` имеет `SurfacePolicy`;
  - каждый `SurfacePolicy` имеет `ProbePolicy`.
- Добавлена проверка, что каждый `probe_method` surface либо входит в context
  preferences этой же surface, либо находится в явном documented exception.
  Сейчас exceptions нет: `documented_probe_context_exception(...) == false`.

Проверка:

```powershell
cargo fmt
cargo test -p stepler-platform
```

Результат текущей проверки: `cargo test -p stepler-platform` зеленый.
После S3 общий набор `policy_invariants.rs` выполняет 10 тестов.

Решение после S1: риск случайного расхождения `ProbePolicy` и `SurfacePolicy`
теперь закрыт тестами. S2 больше не является срочной стабилизацией; выполнять
его стоит только если policy-таблицы реально начнут мешать сопровождению или
будут массово меняться.

### S2. Свести policy-таблицы к одному источнику - не выполнять без нового сигнала

Цель: уменьшить дублирование policy-таблиц, если оно начнет мешать
сопровождению. После S1 это не обязательная safety-фаза: рассинхрон уже должен
ломать `policy_invariants.rs`.

Перед началом S2 обязательно заново оценить, нужна ли она. Если изменения
policy редкие и инварианты остаются зелеными, лучше оставить S2 отложенной и
не трогать policy-структуру.

Решение после S1/S3: S2 сейчас не делать. Сведение policy к одному источнику
может быть полезным позже, но сейчас это больше риск лишнего рефакторинга, чем
необходимая стабилизация. Вернуться к S2 только если:

- policy-таблицы начнут часто меняться;
- изменения начнут дублироваться в `ProbePolicy` и `SurfacePolicy`;
- новые инварианты начнут регулярно ловить ошибки сопровождения.

Если S2 когда-нибудь понадобится, ограничить scope так:

1. Рассмотреть маленький `SurfaceRuntimePolicy`, который хранит для surface:
   `probe_methods`, `pause_methods`, `scrolllock_methods`, `forbidden_methods`,
   `fast_probe`, `allow_risky_methods`.
2. Генерировать из него `ProbePolicy` и `SurfacePolicy`.
3. Не менять порядок methods и runtime behavior в этой фазе.
4. Оставить fixture tests как внешнюю проверку поведения.
5. Если реализация требует большого переписывания `surface.rs`, остановиться и
   пометить S2 как отложенную: выигрыш не должен быть меньше риска.

Проверка:

```powershell
cargo fmt
cargo test -p stepler-platform
cargo test -p stepler-platform-windows
```

### S3. Закрепить bridge/control-plane methods - выполнено

Цель: не смешивать обычные text adapters с bridge methods вроде `PsReadLine`,
`SshTerminal`, `TerminalClipboardShortcut`, `XtermKeyboardSelection`.

Сделано:

- В `stepler-platform` добавлены:
  - `BRIDGE_METHOD_IDS`;
  - `method_is_bridge_method(MethodId)`.
- Bridge/control-plane methods сейчас:
  - `TerminalClipboardShortcut`;
  - `SshTerminal`;
  - `PsReadLine`;
  - `XtermKeyboardSelection`.
- В `policy_invariants.rs` добавлен invariant:
  bridge method не может появиться в `ProbePolicy` или context/replacement
  preferences без явного surface allowance.
- Текущий allowlist:
  - `WindowsTerminalCmd` -> `TerminalClipboardShortcut`;
  - `WindowsTerminalPowerShell` -> `PsReadLine`;
  - `QwenTerminal` -> `XtermKeyboardSelection`.
- `SshTerminal` не разрешен ни одной runtime surface. Это намеренное
  fail-closed состояние: SSH helper работает через отдельный terminal/title
  marker path, а не через generic surface fallback.

Проверка:

```powershell
cargo fmt
cargo test -p stepler-platform
cargo test -p stepler-platform-windows
```

Результат текущей проверки: оба тестовых набора зеленые.

### S4. Расширить negative contracts - делать только по конкретной регрессии

Цель: ловить не только "X работает как X", но и "похожее окно не становится X".

После S1/S3 часть риска уже закрыта:

- `target_facts.rs` уже проверяет, что fast browser title сам по себе не
  превращает unknown Chromium-like shell в browser surface;
- Telegram Qt window без chat title не становится technical Telegram target;
- Yandex остается отдельной surface, но технически browser-like;
- bridge methods не могут попасть в surface без явного allowlist.

Не нужно раздувать negative matrix "на всякий случай". Добавлять только cases,
которые соответствуют реальным похожим поверхностям или уже бывшим регрессиям.

Добавлять negative cases только для:

- Rocket.Chat vs обычный browser editor, если появится новый Rocket.Chat-like
  title/process;
- Codex app vs Windows Terminal PowerShell внутри Codex, если снова появится
  конфликт terminal/browser classification;
- Sticky Notes vs Windows Terminal XAML/InputSite, если меняется XAML/InputSite
  classification;
- Outlook search vs Outlook Word editor vs Outlook shell, если меняются
  Outlook/Word process/class rules.

Проверка:

```powershell
cargo test -p stepler-platform
cargo test -p stepler-platform-windows
```

### S5. Формализовать процесс добавления нового приложения - выполнено

Цель: чтобы новый adapter/surface добавлялся по одному маршруту, а не через
быструю правку random probe.

После S1/S3 это самый полезный следующий шаг без over-engineering: он не меняет
runtime, но снижает шанс, что будущая правка обойдет уже добавленные
инварианты.

Сделано: в `docs/development_commands_ru.md` добавлен короткий чеклист
"Чеклист добавления нового приложения или surface".

Чеклист фиксирует маршрут:

1. Снять `diagnose-focus --delay 3 --methods --surface`.
2. Проверить/добавить `TargetFacts`.
3. Добавить или уточнить `SurfaceKind`.
4. Добавить строку в `probe_contracts.tsv`.
5. Добавить строку в `resolver_contracts.tsv`.
6. Проверить forbidden/risky methods.
7. Только после этого менять technical adapter.
8. Для UI/runtime cases добавить manual smoke note.

Проверка:

```powershell
cargo fmt
cargo test -p stepler-platform
```

Результат: runtime не менялся; проверка `cargo test -p stepler-platform`
должна оставаться зеленой.

Решение после S1/S3/S5: архитектурная стабилизация adapter layer достаточна
для текущего этапа. Дальше не нужно расширять план ради "идеальной"
архитектуры. S2 остается отложенным, S4 выполняется только под конкретную
регрессию или новую похожую surface, а единственный полезный общий шаг без
over-engineering - S6.

### S6. Отделить продуктовый smoke от adapter contracts - выполнено

Цель: не смешивать tray/Qwen workspace/installer/lifecycle с adapter policy.
Это не новая архитектурная фаза и не повод строить большой test harness.

Сделано:

- В `docs/release_smoke_checklist_ru.md` добавлена карта проверок, которая
  разделяет:
  - adapter policy/classifier/resolver contracts;
  - replacement behavior;
  - Windows runtime boundaries;
  - hotkey/layout/tray lifecycle;
  - Qwen input/workspace;
  - installer/release package.
- Добавлено явное правило: изменения adapter policy проверяются contract tests,
  а изменения tray/installer/Qwen workspace требуют manual smoke.
- Для release/runtime изменений добавлена проверка запуска Stepler из
  `dist\Stepler` вне sandbox.
- Qwen input/workspace smoke вынесен в lifecycle-раздел, чтобы не смешивать его
  с adapter policy.
- Большая новая матрица ручных проверок или orchestration framework не
  добавлялись.

Проверка: документальная; `git diff --check` для измененных docs.

После S6 этот план считается закрытым. Дальше действуют поддерживающие правила:

- новый app/surface добавлять по чеклисту из `docs/development_commands_ru.md`;
- новая регрессия должна получать точечный contract или negative case;
- S2 возвращать только при реальной боли сопровождения policy-таблиц;
- S4 расширять только для похожих поверхностей, которые уже ломались или
  реально появились;
- не добавлять новые архитектурные слои без failing test или повторяющегося
  дефекта.

## Постоянные правила

- Не расширять `Unknown` ради одного приложения.
- Не добавлять app-specific checks в adapter, если это classifier/policy
  решение.
- Не менять timing/profile web keyboard без отдельного теста профиля.
- Не добавлять bridge method в surface без явного contract row.
- Не использовать `Capabilities::default()` в production capture path без
  заполнения `method_binding`.
- Любая правка `SurfaceKind` требует `probe_contracts.tsv` и
  `resolver_contracts.tsv`.

## Быстрая проверка после архитектурных правок

```powershell
cargo fmt
cargo test -p stepler-core -p stepler-platform -p stepler-platform-windows -p stepler-cli
```
