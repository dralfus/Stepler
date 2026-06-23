# План архитектурного hardening изоляции адаптеров

Дата: 2026-06-22

Актуальная итоговая ревизия после фаз 1-9:
`docs/adapter_architecture_stability_review_ru.md`.

Цель: сделать так, чтобы правка адаптера или оптимизация под одно приложение
минимально могла сломать другое приложение. Этот документ написан как
пошаговое ТЗ для исполнителя. Следовать по порядку, не перепрыгивать фазы.

## Главные правила для исполнителя

1. Делать по одной фазе за раз.
2. После каждой фазы запускать указанные тесты.
3. Не менять пользовательское поведение, если фаза явно не требует этого.
4. Не менять `MethodId` без отдельной причины.
5. Не добавлять новые fallback-методы в приложения "на всякий случай".
6. Не переносить app-specific проверки обратно в адаптеры.
7. Если тесты начали падать, не чинить все подряд. Сначала понять, какой
   контракт поменялся и должен ли он был поменяться.

Обязательные команды после каждой фазы:

```powershell
cargo fmt
cargo test -p stepler-core -p stepler-platform -p stepler-platform-windows -p stepler-cli
```

Если менялся только `stepler-platform`, можно сначала запускать:

```powershell
cargo test -p stepler-platform
```

Но перед завершением всей работы обязательно выполнить полный набор выше.

## Текущее состояние

Уже есть:

- `crates/stepler-platform/src/surface.rs`
  - `SurfaceKind`
  - `SurfacePolicy`
  - `ProbePolicy`
  - `ProbePlan`
  - classifier
  - web keyboard profiles
- `crates/stepler-platform/tests/fixtures/probe_contracts.tsv`
  - matrix ожидаемых probe-методов для проверенных поверхностей
- `crates/stepler-platform/tests/probe_matrix.rs`
  - проверяет `probe_plan_for`, `suppressed_methods`, `fast_probe` и
    `min_confidence`
- `crates/stepler-platform/src/resolver.rs`
  - `MethodResolver`
  - `ResolveDecision`
  - `ResolveTrace`
- `crates/stepler-platform/tests/fixtures/resolver_contracts.tsv`
  - matrix проверенных поверхностей
- `crates/stepler-core/tests/fixtures/replacement_behavior.tsv`
  - behavior-контракты range/caret/selection
- `diagnose-focus --surface`
  - выводит policy, `probe_plan`, runtime probes и resolver trace

Фазы 1-2 уже закрыли главный риск на уровне probe collection:

- `windows_method_probes` больше не содержит собственный app-specific порядок;
- runtime probes берутся из `probe_plan_for(target)`;
- fast web surfaces теперь явно получают fallback
  `web_keyboard_selection -> uia_editable_text`;
- `diagnose-focus --surface` показывает план и runtime-список.

Фаза 3 убрала прямой `SurfaceKind`/`classify_surface` gating из adapter
`probe` для clipboard/send_input/generic UIA/web/xterm методов.

Фаза 4 разделила web keyboard logic:

- `web_keyboard_profile.rs` содержит profile/timing/control-prefix helpers;
- `web_keyboard_support.rs` содержит technical target predicates без
  `SurfaceKind`/`classify_surface`;
- `web_keyboard.rs` использует готовый timing profile и больше не держит
  inline profile timeout branching;
- `classify_surface` для web keyboard остался только в test-only helpers,
  которые проверяют соответствие title -> profile.

Главные оставшиеся риски теперь ниже по стеку:

- fail logs пока не всегда дают достаточно короткую причину, какой surface,
  probe и resolver path привели к ошибке;
- resolver/unit contracts еще частично дублируют fixture contracts.
- `web_keyboard_support.rs` намеренно содержит технический allow-list target
  признаков для browser/Telegram/Notepad/Sticky Notes. Это не policy, но
  выглядит похоже на app-specific routing, поэтому любые новые признаки там
  должны идти вместе с focused Windows test и, если surface проверенная, с
  `probe_contracts.tsv`.

## Фаза 1. Ввести явный ProbePolicy / ProbePlan - выполнена

### Цель

Сделать так, чтобы список методов, которые вообще можно пробовать на surface,
жил рядом с `SurfacePolicy`, а не был скрыт внутри `windows_method_probes`.

### Что уже сделано

Добавлено:

- `crates/stepler-platform/src/surface.rs`
  - `ProbePolicy`
  - `ProbePlan`
  - `default_probe_policies()`
  - `probe_plan_for(&ForegroundTarget)`
  - `probe_policy_for(SurfaceKind)`
- `crates/stepler-platform/src/lib.rs`
- `crates/stepler-platform/tests/fixtures/probe_contracts.tsv`
- `crates/stepler-platform/tests/probe_matrix.rs`

Проверяется:

- `probe_plan_for(target).surface.kind`;
- `probe_methods` равны ожидаемым;
- `suppressed_methods` содержат явно запрещенные методы;
- `surface.confidence >= min_confidence`.

### Зафиксированная проверка

- `cargo test -p stepler-platform`
- `cargo fmt`
- `cargo test -p stepler-core -p stepler-platform -p stepler-platform-windows -p stepler-cli`
- `cargo build -p stepler-cli --release --target-dir target-codex-release`

Windows-слой переведен на `ProbePlan` в фазе 2.

## Фаза 2. Подключить ProbePlan в Windows probe collection - выполнена

### Цель

Убрать скрытую логику выбора probes из `windows_method_probes`, но сделать это
без случайного изменения поведения fast web surfaces.

### Что уже сделано

- `windows_method_probes` теперь строит probes через `probe_plan_for(target)`.
- Добавлен helper `probe_method_by_id(MethodId, target)`.
- Старый app-specific fast web early return удален.
- Выполнена 2B: `runtime_probe_methods == probe_plan.probe_methods`, поэтому
  fast web surfaces получают явный fallback `uia_editable_text` после
  `web_keyboard_selection`.
- `diagnose-focus --surface` выводит `probe_plan: methods=[...] runtime=[...]
  suppressed=[...] fast=...`.
- Добавлены Windows tests для Codex/FastBrowserEditor, Rocket.Chat search,
  Sticky Notes и QwenTerminal.
- Проверено:
  - `cargo fmt`
  - `cargo test -p stepler-platform -p stepler-platform-windows`
  - `cargo test -p stepler-core -p stepler-platform -p stepler-platform-windows -p stepler-cli`
  - `cargo build -p stepler-cli --release --target-dir target-codex-release`

### Фактически измененные файлы

- `crates/stepler-platform-windows/src/lib.rs`
- `crates/stepler-platform-windows/src/diagnostics.rs`
- `crates/stepler-platform-windows/src/tests.rs`
- `crates/stepler-cli/src/main.rs`

### Фаза 2A. Подключение без изменения поведения

Исторический шаг. Оставлен как контекст, если понадобится откатывать 2B
отдельно.

В `windows_method_probes(target)`:

1. Получить:

```rust
let probe_plan = probe_plan_for(target);
```

2. Вместо ручного порядка и app-specific ранних return сделать helper:

```rust
fn probe_method_by_id(method: MethodId, target: &ForegroundTarget) -> Option<MethodProbe>
```

3. Получить runtime-список методов из `ProbePlan`.

```rust
let runtime_probe_methods = if probe_plan.fast_probe {
    probe_plan.probe_methods.iter().take(1).copied().collect::<Vec<_>>()
} else {
    probe_plan.probe_methods.clone()
};
```

На этом шаге `fast_probe=true` намеренно сохраняет старое поведение:
`FastBrowserEditor` и `RocketChatEditor` реально пробуют только первый метод
(`web_keyboard_selection`). Остальные методы остаются в `probe_plan_methods`
для диагностики, но не вызываются в runtime.

4. Удалить или обнулить special-case:

```rust
if is_fast_web_keyboard_primary_target(target) {
    ...
    return probes;
}
```

Fast behavior должен приходить из `probe_plan.fast_probe` и порядка
`probe_methods`, а не из раннего return.

Если удалить функцию сразу неудобно, допустимо временно оставить тонкую
обертку, которая использует `probe_plan_for(target).fast_probe`, но в ней не
должно быть app-specific логики.

### Фаза 2B. Явное включение fallback для fast web surfaces

Выполнено после зеленой 2A.

Чтобы Codex/Jira/Confluence/Rocket.Chat могли fallback'нуться с
`web_keyboard_selection` на `uia_editable_text`, runtime-список сделан полным:

```rust
let runtime_probe_methods = probe_plan.probe_methods.clone();
```

Это было поведенческое изменение. Если после ручного smoke появится регресс,
откатывать только 2B: временно вернуть runtime-список fast surfaces к первому
методу из `probe_plan.probe_methods`, не откатывая `probe_method_by_id` и
диагностику из 2A.

Перед любым повторным изменением fast fallback обязательно:

- проверить, что `uia_editable_text` не перехватывает рабочие сценарии быстрее
  `web_keyboard_selection`;
- добавить/обновить Windows tests на порядок probes;
- вручную проверить Codex Windows app, Jira, Confluence и Rocket.Chat search;
- при регрессе откатить только 2B, не откатывая 2A.

### Диагностика

В `WindowsMethodDiagnostics` добавлено:

- `probe_plan_methods`
- `runtime_probe_methods`
- `probe_plan_suppressed_methods`
- `probe_plan_fast`

`diagnose-focus --surface` выводит:

```text
probe_plan: methods=[...] runtime=[...] suppressed=[...] fast=true/false
```

### Тесты

В `stepler-platform-windows/src/tests.rs` добавлены/обновлены тесты:

- fast browser target больше не hardcodes early return и получает runtime
  список из `ProbePlan`;
- fast browser runtime после 2B совпадает с `probe_plan_methods`;
- Rocket.Chat search получает `web_keyboard_selection,uia_editable_text`;
- Sticky Notes не получает terminal probes;
- QwenTerminal получает только `xterm_keyboard_selection` и не получает
  `terminal_clipboard_shortcut`.

### Достигнутый критерий готовности

- `windows_method_probes` больше не содержит app-specific ранних return.
- `is_fast_web_keyboard_primary_target` удален.
- 2B явно отмечена как поведенческое изменение и покрыта тестами.
- `cargo test -p stepler-platform-windows -p stepler-platform` зеленый.

Важная граница: `MethodId::PsReadLine` присутствует в `ProbePlan` и
диагностике для Windows Terminal PowerShell, но не является обычным
`capture_by_method` адаптером. Реальное поведение PowerShell обслуживается
PSReadLine/passthrough-слоем. Не пытаться чинить это в фазе 3 переносом
PowerShell в generic Windows capture/apply path.

## Фаза 3. Убрать surface-gating из adapter probe, где это безопасно - выполнена

### Цель

Адаптеры должны отвечать на вопрос "могу ли я технически работать с этим
control", а не "разрешено ли мне работать в этом приложении".

### Что уже сделано

- `ClipboardSelectionMethod`, `SendInputMethod` и generic
  `UiAutomationTextMethod` больше не проверяют `SurfaceKind::Unknown` внутри
  `probe`.
- `UiAutomationTextMethod` сохранил технические блокировки для Win32 Edit,
  terminal, Word, shell/listview surfaces.
- `WebKeyboardSelectionMethod` больше не использует `classify_surface` в
  `probe`; вместо этого проверяет технический target predicate
  `is_web_keyboard_technical_target`.
- `XtermKeyboardSelectionMethod` больше не проверяет `SurfaceKind`; вместо
  этого использует xterm textarea, terminal class и `stepler-terminal-app`
  marker.
- Тесты, которые раньше ожидали запрет прямо в adapter probe для browser-like
  surfaces, переведены на runtime contract через `windows_method_probes`.
- Проверено:
  - `cargo fmt`
  - `cargo test -p stepler-platform-windows -p stepler-platform`
  - `cargo test -p stepler-core -p stepler-platform -p stepler-platform-windows -p stepler-cli`

### Оценка качества реализации

Хорошо:

- adapter `probe` больше не принимает policy-решение через `SurfaceKind`;
- запреты для browser/Telegram/Yandex теперь проверяются через runtime probe
  stack, а не прямым `probe().is_none()`;
- технические safety-блокировки для shell/listview/edit/terminal/Word
  сохранены;
- `QwenTerminal` и Sticky Notes защищены focused Windows tests.

Осторожно:

- web/xterm technical predicates пока живут в общем `lib.rs` рядом с window
  helpers и частично повторяют признаки classifier'а;
- это не регресс фазы 3, но именно этот слой нужно аккуратно вынести/покрыть в
  фазе 4, чтобы он не стал вторым скрытым classifier'ом.

### Файлы

- `crates/stepler-platform-windows/src/clipboard_selection.rs`
- `crates/stepler-platform-windows/src/send_input.rs`
- `crates/stepler-platform-windows/src/uia_text.rs`
- `crates/stepler-platform-windows/src/web_keyboard.rs`
- `crates/stepler-platform-windows/src/tests.rs`

### Что убрать

Постепенно убрать из `probe` проверки вида:

```rust
if classify_surface(target).kind != SurfaceKind::Unknown { return None; }
```

и checks, где adapter сам решает surface policy.

### Что оставить

Оставить технические проверки:

- класс окна поддерживается технически;
- control похож на edit/document;
- `focused_is_xterm_textarea()`;
- terminal marker есть/нет;
- env flag для экспериментального технического пути.

Не убирать preflight из `capture` / `apply`.

### Важное ограничение

Эту фазу делать по одному адаптеру:

1. `ClipboardSelectionMethod`
2. `SendInputMethod`
3. `UiAutomationTextMethod`
4. `WebKeyboardSelectionMethod`
5. `XtermKeyboardSelectionMethod`

После каждого адаптера запускать:

```powershell
cargo test -p stepler-platform-windows -p stepler-platform
```

### Тесты

После каждого адаптера проверить, что запрещение теперь приходит из
`ProbePlan` / `SurfacePolicy`, а не из самого adapter probe.

Добавить или обновить tests:

- forbidden method присутствует в `probe_plan.suppressed_methods`;
- если метод не в `probe_plan.probe_methods`, `windows_runtime_probe_methods`
  его не возвращает и Windows-слой его не вызывает;
- resolver trace объясняет запрет.

Перед изменением каждого адаптера добавить или проверить строку в
`probe_contracts.tsv` для surface, которую этот адаптер может затронуть.
Если surface нет в fixture, сначала добавить contract row, затем менять probe.
Так будет видно, что правка адаптера не расширила probes для чужих приложений.

После фазы 2 не нужно менять `windows_method_probes` ради каждого адаптера.
Если кажется, что нужно, сначала проверить `ProbePolicy` в `surface.rs` и
`probe_contracts.tsv`. Исключение - только добавление нового `MethodId` или
нового adapter helper.

### Критерий готовности

- В adapter probe не осталось app-policy checks.
- Все app-policy checks живут в `surface.rs`.
- Windows tests зеленые.

## Фаза 4. Разделить profile-specific части WebKeyboardSelectionMethod - выполнена

### Цель

Снизить риск, что оптимизация Rocket.Chat ломает Codex/Jira/Confluence или
наоборот.

### Что уже сделано

- Добавлен `crates/stepler-platform-windows/src/web_keyboard_profile.rs`.
- Вынесены:
  - `WebKeyboardTimingProfile`;
  - `web_keyboard_timing_profile`;
  - `web_keyboard_control_prefix`;
  - `web_keyboard_profile_is_fast`;
  - `web_keyboard_profile_is_rocket`;
  - test-only profile title helpers.
- Добавлен `crates/stepler-platform-windows/src/web_keyboard_support.rs`.
- Вынесены technical predicates:
  - `is_web_keyboard_technical_target`;
  - `is_browser_like_target`;
  - `is_telegram_target`;
  - `has_terminal_app_marker`.
- `web_keyboard_support.rs` не вызывает `classify_surface` и не использует
  `SurfaceKind`.
- `web_keyboard.rs` теперь получает тайминги через
  `web_keyboard_timing_profile(profile)`, а не через inline if/else.
- Добавлены focused Windows tests:
  - `web_keyboard_timing_profiles_are_profile_specific`;
  - `web_keyboard_control_prefixes_follow_profile_only`;
  - `web_keyboard_technical_target_does_not_expand_unknown_runtime_stack`.
- Проверено:
  - `cargo fmt`;
  - `cargo test -p stepler-platform-windows -p stepler-platform`;
  - `cargo test -p stepler-core -p stepler-platform -p stepler-platform-windows -p stepler-cli`.

### Оценка качества реализации

Хорошо:

- runtime `classify_surface` не вернулся в `WebKeyboardSelectionMethod.probe`;
- profile/timing/control-prefix логика собрана в одном маленьком модуле;
- technical target predicates отделены от profile/timing helpers;
- `web_keyboard_support.rs` не принимает решение "разрешен ли метод на
  surface", а только отвечает "похож ли target на технически поддерживаемый";
- фаза покрыта focused tests и не потребовала менять `MethodId` или contracts.

Проблемы/риски, которые важно не потерять:

- `web_keyboard_support.rs` все равно содержит app-похожие признаки
  (`browser`, `Telegram`, `Notepad`, `Sticky Notes`). Это осознанный
  технический allow-list, но будущие правки легко могут превратить его во
  второй classifier. Поэтому новые признаки добавлять только с тестом на
  положительный case и тестом, что чужая surface не расширила runtime probes.
- `web_keyboard_profile_is_fast` и `web_keyboard_profile_is_rocket` зависят от
  env flag `STEPLER_ENABLE_WEB_FAST_PROFILE`. Текущие тесты проверяют default
  behavior. Если будущие тесты будут менять этот env flag, они должны
  изолировать и восстанавливать окружение.
- `capture/apply` в `web_keyboard.rs` все еще большой. Это не исправлялось в
  фазе 4 намеренно: caret/focus restore связан с preflight/apply path и уже
  был источником регрессов. Выносить его только отдельной маленькой фазой под
  конкретный bugfix или тест.

Решение после ревизии: дополнительных кодовых правок в фазе 4 не требуется.
Оставшиеся риски закрывать через фазы 5, 6 и 9, а не расширением
`web_keyboard_support.rs`.

### Файлы

- `crates/stepler-platform-windows/src/web_keyboard.rs`
- `crates/stepler-platform-windows/src/web_keyboard_profile.rs`
- `crates/stepler-platform-windows/src/web_keyboard_support.rs`
- `crates/stepler-platform-windows/src/tests.rs`

### Что сделать

Не менять `MethodId`.

Не менять соответствие `SurfaceKind -> WebKeyboardProfile` в `surface.rs` в
рамках этой фазы. Если профиль для surface все же нужно поменять, сначала
обновить `probe_contracts.tsv` и resolver contracts, затем сделать отдельный
маленький коммит.

После фазы 3 в scope фазы 4 также входит аккуратно отделить technical target
predicates от profile/timing logic:

- `is_web_keyboard_technical_target`;
- `is_browser_like_target`;
- `is_telegram_target`;
- `is_notepad_like_target`;
- `is_sticky_notes_target`;
- `has_terminal_app_marker`.

Эти helpers не должны вызывать `classify_surface` и не должны решать, разрешен
ли метод на surface. Они отвечают только на вопрос "похож ли control технически
на target, с которым keyboard-selection adapter умеет работать". Разрешение
метода остается в `ProbePolicy`.

Profile-specific настройки вынесены в структуру:

```rust
struct WebKeyboardTimingProfile {
    selected_timeout: Duration,
    short_context_timeout: Duration,
    line_context_timeout: Duration,
    clipboard_timeout: Duration,
    retry_pause: Duration,
    attempt_pause: Duration,
}
```

Функция:

```rust
fn web_keyboard_timing_profile(profile: WebKeyboardProfile) -> WebKeyboardTimingProfile
```

Caret/focus restore helpers пока не выносились: они сильнее связаны с apply
path и preflight. Выносить их отдельно только если появится следующая
локальная правка в `web_keyboard.rs`, чтобы не делать пустую декомпозицию.
Trait hierarchy не вводился.

### Тесты

Добавлены/проверены unit tests:

- `Standard` profile тайминги не равны `Fast`;
- `RocketSearch` использует fast-параметры;
- `web_keyboard_control_prefix` не ломает существующие control ids;
- technical target predicates не используют classifier/policy и не расширяют
  `windows_runtime_probe_methods` для чужих surfaces;
- QwenTerminal по-прежнему получает только `xterm_keyboard_selection`, а не
  web keyboard/terminal clipboard fallback.

### Критерий готовности

- Основной `capture` в `web_keyboard.rs` стал короче и читабельнее за счет
  вынесения profile/timing веток.
- Profile-specific ветки находятся рядом и покрыты тестами.
- Technical target predicates отделены от profile/timing helpers и остаются
  `SurfaceKind`-free.
- Реальное поведение не менялось.

## Фаза 5. Добавить runtime trace summary в hotkey fail logs - выполнена

### Цель

Если пользователь присылает только `stepler_hotkey_log.jsonl`, должно быть
понятно:

- какая surface была распознана;
- какой метод был выбран;
- какие методы были запрещены или suppressed;
- на каком capture/apply произошел fail.

### Файлы

- `crates/stepler-cli/src/main.rs`
- `crates/stepler-platform-windows/src/diagnostics.rs`
- `crates/stepler-platform-windows/src/lib.rs`
- `crates/stepler-core/src/log_event.rs`
- `crates/stepler-core/src/lib.rs`
- `crates/stepler-platform-windows/src/tests.rs`

### Что уже сделано

- В `OperationLogEvent` добавлено необязательное поле `resolver_trace`.
- `HotkeyReceived`, `Completed` и embedded-terminal success events пишут
  `resolver_trace: None`, поэтому успешные операции и индикатор нажатия не
  раздуваются.
- В `stepler-platform-windows` добавлен
  `hotkey_failure_trace_summary(mode, final_error)`.
- Formatter собирает только diagnostic snapshot:
  - foreground target;
  - `surface`;
  - `confidence`;
  - `probe_plan`;
  - `runtime`;
  - фактически созданные `probes`;
  - `probe_none`;
  - `suppressed`;
  - selected resolver method;
  - `policy_skipped`;
  - финальную ошибку как `operation_failed`.
- Formatter не вызывает `text_context()` и не запускает замену текста повторно.
- Formatter не добавляет новый classifier для web keyboard: использует
  существующие `probe_plan_for`, `windows_runtime_probe_methods`,
  `windows_method_probes` и resolver trace.
- При ошибке самого diagnostic path основной fail log остается старого вида,
  только без `resolver_trace`.
- Добавлены tests:
  - `operation_log_event_formats_jsonl` проверяет JSON field
    `resolver_trace`;
  - `hotkey_failure_trace_summary_includes_probe_and_resolver_boundaries`
    проверяет наличие surface/mode/probe/runtime/probe_none/suppressed/
    policy_skipped/final.
- Проверено:
  - `cargo test -p stepler-core -p stepler-platform-windows`.
  - `cargo test -p stepler-core -p stepler-platform -p stepler-platform-windows -p stepler-cli`.

### Оценка качества реализации

Хорошо:

- fail trace вынесен в отдельное поле `resolver_trace`, старое поле
  `expected_before_text` осталось совместимым;
- success logs и `HotkeyReceived` не получили тяжелый trace;
- formatter не вызывает `text_context()` и не запускает adapter capture/apply
  повторно;
- formatter использует существующие `ProbePlan`, runtime probes и resolver
  trace, а не строит новый classifier;
- если diagnostic path сам упадет, основной fail log все равно пишется.

Проблемы/риски, которые важно не потерять:

- текущий `OperationError` не несет точную stage-информацию, поэтому
  `resolver_trace` пишет `final=operation_failed:...`, а не пытается угадать
  `capture_failed` или `apply_failed`;
- `policy_skipped` показывает только resolver-visible rejections, а методы,
  отфильтрованные раньше, нужно смотреть в `suppressed=[...]`;
- formatter собирает foreground snapshot в момент логирования fail. Если фокус
  уже успел смениться после ошибки, trace может описывать новое foreground
  окно. Это приемлемый компромисс для легкого fail log, но не замена
  `diagnose-focus --surface`.

Решение после ревизии: кодовая правка нужна была только одна - заменить
неточную метку `capture_failed` на `operation_failed`. Более точные
`context_capture_failed` / `plan_failed` / `apply_failed` выносить в отдельную
маленькую фазу только если это реально понадобится по логам.

### Что сделать

При `RolledBackOrFailed` добавлена короткая диагностическая строка в отдельном
поле `resolver_trace`.

Не писать полный огромный trace в каждую успешную операцию. Только fail.

Trace должен использовать уже существующие данные policy/probe:

- `surface`;
- `confidence`;
- `probe_plan_methods`;
- `runtime_probe_methods`;
- `suppressed_methods`;
- выбранный resolver method, если он был;
- финальную ошибку capture/apply.

После фазы 2 эти данные уже есть в `WindowsMethodDiagnostics` и CLI
`diagnose-focus --surface`. В фазе 5 не нужно заново строить classifier/probe
trace. Нужно вынести маленький formatter/helper, который переиспользует
существующие поля или их компактный аналог в runtime fail path.

После фазы 3 formatter должен различать:

- метод не попал в `runtime_probe_methods` из-за `ProbePolicy`;
- метод был в runtime list, но adapter `probe` технически вернул `None`;
- resolver выбрал метод, но `capture/apply` упал.

Это важно, потому что adapter `probe` теперь чаще отвечает "технически могу",
а запрет policy находится выше.

После фазы 4 для web keyboard fail summary не нужно заново вычислять profile
или target type внутри formatter. Использовать уже выбранный method/control id
и surface/probe trace. Если нужен compact label для web keyboard context,
брать его из `web_keyboard_profile.rs`/`web_keyboard_support.rs`, но не
добавлять новый classifier в логирование.

После ревизии фазы 4 добавлено в trace summary явное различение:

- `policy_skipped`: метод был запрещен `ProbePolicy`/`SurfacePolicy`;
- `probe_none`: метод был в runtime list, но adapter technical `probe` вернул
  `None`;
- `operation_failed`: операция упала после выбранного diagnostic path.

Для web keyboard это особенно важно: `probe_none` означает технический отказ
из `web_keyboard_support.rs`, а не policy-запрет surface.

Точную стадию `capture/apply/plan` этот trace пока не обещает. Ее можно
добавить позже только через явное расширение `OperationError`/runner stage, а
не через угадывание по тексту ошибки.

Нюанс: `policy_skipped` показывает только policy/risky/replacement rejections
среди методов, которые дошли до resolver trace. Методы, отфильтрованные еще на
уровне `ProbePolicy`, фиксируются в `suppressed=[...]`.

Добавлено отдельное поле:

```json
"resolver_trace":"surface=... selected=... rejected=..."
```

`expected_before_text` оставлен для старого текста ошибки, чтобы не ломать
существующую привычку смотреть fail reason в этом поле.

### Тесты

Добавлен unit test на formatter:

- trace summary содержит surface;
- confidence;
- probe/runtime/suppressed methods;
- selected method;
- suppressed/policy boundary;
- final error.

`HotkeyReceived` события не получают `resolver_trace`, индикатор нажатия
остается легким.

### Критерий готовности

- При fail лог стал полезнее.
- Успешные операции не раздулись.
- Существующие log tests зеленые.

## Фаза 6. Убрать дубли больших resolver contracts из unit-тестов - выполнена

### Цель

Один источник правды для проверенных приложений.

### Файлы

- `crates/stepler-platform/src/lib.rs`
- `crates/stepler-platform/tests/fixtures/resolver_contracts.tsv`
- `crates/stepler-platform/tests/resolver_matrix.rs`

### Что уже сделано

- Удален большой дублирующий unit-test
  `resolver_contracts_for_verified_applications`.
- Focused resolver tests в `crates/stepler-platform/src/lib.rs` оставлены:
  risky blocked, forbidden by policy, mode-specific preferences, trace
  acceptance/rejection, Outlook boundaries, fallback boundaries.
- Перед удалением сверены app cases из unit-test:
  Notepad, classic console, Windows Terminal PowerShell/cmd, QwenTerminal,
  Codex, Jira, Confluence, Rocket.Chat, Telegram, Word, Outlook editor/search,
  Sticky Notes.
- Единственный полезный case, которого не было в TSV, перенесен в fixture:
  `rocket-search-ru-pause` с title `Нет непрочитанных сообщений`.
- `resolver_contracts.tsv` остался источником правды для проверенных
  приложений.
- Проверено:
  - `cargo test -p stepler-platform`.

### Что сделать

В `lib.rs` оставить только focused unit tests:

- risky blocked;
- forbidden by policy;
- different methods for `P` and `CP`;
- trace explains acceptance/rejection.

Большой тест `resolver_contracts_for_verified_applications` удален после
сверки с TSV.

Перед удалением сравнить cases:

- Notepad
- classic console
- Windows Terminal PowerShell
- Windows Terminal cmd
- QwenTerminal
- Codex app
- Jira
- Confluence
- Rocket.Chat
- Telegram
- Word
- Outlook editor
- Outlook search
- Sticky Notes

Если case есть только в unit test, перенести его в TSV.

После фаз 1-2 в unit tests могут оставаться focused checks для
`windows_runtime_probe_methods` и `probe_method_by_id`. Их не переносить в
resolver TSV: это Windows implementation-level tests, а не resolver contracts.

После фазы 3 также оставить focused tests, которые проверяют technical
predicates web/xterm adapter'ов. Их не переносить в resolver TSV, потому что
они проверяют Windows technical routing, а не resolver choice.

После фазы 4 focused tests для `web_keyboard_profile.rs` и
`web_keyboard_support.rs` тоже оставить в Windows unit tests. Они фиксируют
локальную декомпозицию adapter implementation, а не resolver contract.

Не пытаться переносить tests `web_keyboard_timing_profiles_are_profile_specific`,
`web_keyboard_control_prefixes_follow_profile_only` и
`web_keyboard_technical_target_does_not_expand_unknown_runtime_stack` в TSV.
Это не app contract, а protection от повторного смешивания profile/support
logic.

После фазы 5 tests для `OperationLogEvent.resolver_trace` и
`hotkey_failure_trace_summary_*` тоже не переносить в resolver TSV. Это
logging/diagnostics contract, а не contract выбора метода. В TSV должны
оставаться surface/probe/resolver expectations, а не формат JSONL.

Не удалять и не объединять `probe_contracts.tsv` с `resolver_contracts.tsv`.
Это разные уровни контракта:

- `probe_contracts.tsv` проверяет, какие методы вообще разрешено спрашивать у
  Windows/UIA/keyboard слоя;
- `resolver_contracts.tsv` проверяет, какой метод resolver выберет из уже
  найденных candidates.

Допустимо вынести общий parser helper для TSV, если это маленькая механическая
правка без изменения fixture формата.

### Критерий готовности

- Большие app lists живут только в TSV fixtures.
- Focused unit tests остались.
- `cargo test -p stepler-platform` зеленый.

## Фаза 7. Использовать confidence как контракт - выполнена

### Цель

Classifier confidence должен защищать от слишком грубых совпадений, особенно в
Chromium/Electron окнах.

### Файлы

- `crates/stepler-platform/src/surface.rs`
- `crates/stepler-platform/tests/fixtures/probe_contracts.tsv`
  или `resolver_contracts.tsv`
- соответствующий matrix test

### Что уже сделано

- `probe_contracts.tsv` уже содержит `min_confidence` для проверенных
  probe-поверхностей, это было заложено в фазе 1.
- `probe_contract_matrix_matches_verified_surfaces` уже парсит
  `min_confidence` и проверяет:

```rust
assert!(plan.surface.confidence >= row.min_confidence)
```

- Confidence пока остается тестовым/диагностическим контрактом. Runtime behavior не
  менялся: низкая confidence не включает молчаливый fallback и не переводит
  surface в `Unknown`.
- Колонка confidence не добавлялась в `resolver_contracts.tsv`, потому что
  текущий gap покрыт probe matrix и дублировать контракт не нужно.
- Добавлен probe-contract для Rocket.Chat с русским title
  `Нет непрочитанных сообщений`, чтобы вариант из resolver fixture был покрыт
  и на уровне probe/confidence.
- Сообщение падения confidence-теста теперь печатает evidence classifier'а.
- Проверено:
  - `cargo test -p stepler-platform`.

### Оценка качества реализации

Хорошо:

- контракт реализован в одном источнике правды - `probe_contracts.tsv`;
- matrix одновременно проверяет `expected_surface`, `probe_methods`,
  `suppressed_methods`, `fast_probe` и минимальную confidence;
- ошибка теста печатает evidence classifier'а, поэтому будет понятно, какое
  правило сработало;
- нет скрытого изменения поведения для Codex/Jira/Confluence/Rocket.Chat.

Осторожно:

- confidence защищает classifier, но не заменяет `ProbePolicy` и
  `SurfacePolicy`;
- technical predicates из `web_keyboard_support.rs` все еще должны
  проверяться отдельными Windows tests, потому что они могут расширить runtime
  stack даже при корректной classification;
- при добавлении новой Chromium/Electron-like surface сначала добавлять row в
  `probe_contracts.tsv` с осмысленным `min_confidence`, а уже потом менять
  classifier/support predicates.

### Зафиксированные границы после выполнения

`min_confidence` остается обязательным контрактом в `probe_contracts.tsv`.
Если появляются новые проверенные surfaces, их нужно добавлять туда с
осмысленным минимальным confidence.

Тест проверяет:

```rust
assert!(plan.surface.confidence >= row.min_confidence)
```

Runtime behavior в фазе 7 не менялся.

Confidence в `resolver_contracts.tsv` не добавлялся. Добавлять его туда только
если возникнет реальный gap, который не покрывает `probe_contracts.tsv`. Не
дублировать колонку ради симметрии.

Runtime confidence-gating остается возможной отдельной будущей задачей:

- если confidence ниже порога для risky/fast surface, использовать более
  консервативный policy или `Unknown`;
- но это делать отдельным коммитом/фазой и только если есть покрытие.

После 2B особенно не включать runtime confidence-gating для fast web surfaces
без ручного smoke Codex/Jira/Confluence/Rocket.Chat: низкий confidence должен
сначала падать в matrix test, а не молча менять runtime policy.

После фазы 4 помнить, что classifier confidence не покрывает все риски
`web_keyboard_support.rs`: support predicates могут технически признать target,
но итоговый runtime stack все равно должен ограничиваться `ProbePolicy`.
Если добавляется новая browser/electron-like surface, сначала добавить
`probe_contracts.tsv` row с `min_confidence`, затем Windows test, что technical
predicate не расширяет чужую surface.

### Критерий готовности

- Matrix защищает ожидаемую уверенность classifier’а.
- Runtime behavior не изменился.

## Фаза 8. Outlook/Zimbra safety boundary - выполнена

### Цель

Сохранить Outlook как high-risk surface и не дать будущим fallback-ам случайно
попасть в Outlook.

### Файлы

- `crates/stepler-platform/src/surface.rs`
- `crates/stepler-platform/tests/fixtures/resolver_contracts.tsv`
- `crates/stepler-platform/tests/fixtures/probe_contracts.tsv`
- `outlookhaging.md`

### Что уже сделано

- `OutlookWordEditor` сужен на уровне `SurfacePolicy` до `word_com`.
- Generic UIA (`uia_editable_text`, `uia_document_text`, `uia_text`),
  `send_input`, `clipboard_selection`, `terminal_clipboard_shortcut` и
  `win32_edit_messages` запрещены resolver policy для `OutlookWordEditor`.
- `probe_contracts.tsv` содержит явные rows для:
  - `outlook-editor`
  - `outlook-search`
  - `outlook-shell`
- `resolver_contracts.tsv` содержит явные rows для:
  - `outlook-editor-cp`
  - `outlook-search-pause`
  - `outlook-shell-pause`
- Добавлен Windows unit test
  `outlook_runtime_stacks_do_not_include_generic_fallbacks`, который проверяет
  runtime probe stack для Outlook editor/search/shell.
- `outlookhaging.md` обновлен: теперь явно различает runtime `ProbePolicy` и
  resolver `SurfacePolicy`, а также ссылается на fixture contracts.
- Проверено:
  - `cargo test -p stepler-platform`
  - `cargo test -p stepler-platform-windows outlook_runtime_stacks_do_not_include_generic_fallbacks`
  - `cargo test -j 1 -p stepler-core -p stepler-platform -p stepler-platform-windows -p stepler-cli`

### Оценка качества реализации

Хорошо:

- Outlook high-risk boundary теперь зафиксирован на двух уровнях:
  `ProbePolicy` не дает runtime generic fallback, а `SurfacePolicy` не дает
  resolver принять generic UIA/clipboard/send_input для Outlook editor.
- Outlook shell/explorer перестал быть неявным случаем: он есть и в probe, и в
  resolver fixture.
- Windows runtime stack покрыт focused test, а не только platform matrix.

Осторожно:

- Outlook search все еще разрешает `Win32EditMessages`, потому что это нужно
  для рабочего поиска писем. Риск Zimbra hang описан в `outlookhaging.md`;
  не расширять search fallback без отдельного smoke.
- `WordEditor` не тронут: generic UIA fallback для обычного Word остается
  прежним и не должен смешиваться с Outlook policy.

### Исторический план фазы

Проверить и зафиксировать contract rows:

- Outlook compose/editor:
  - surface: `OutlookWordEditor`
  - runtime probe method: `word_com`
  - forbidden at runtime/probe level: clipboard, send_input, terminal
    shortcuts, generic UIA fallback
  - отдельно проверить resolver policy: сейчас safety boundary в runtime
    держится через `ProbePolicy`, а не через полный запрет UIA в
    `SurfacePolicy`; если цель фазы 8 - запретить UIA даже для искусственно
    переданных probes, это отдельное поведенческое решение и его нужно покрыть
    resolver contract
- Outlook search:
  - surface: `OutlookSearch`
  - method: текущий разрешенный метод
  - forbidden: все risky fallback
- Outlook shell/explorer:
  - surface: `OutlookShell`
  - no risky fallback

В `probe_contracts.tsv` после фаз 1-7 уже есть rows для Outlook compose/editor
и Outlook search. В фазе 8 нужно убедиться, что они не расширились при
подключении Windows `ProbePlan`, и отдельно добавить/проверить
`OutlookShell`/Explorer row, если shell/explorer surface должна быть явным
контрактом.
После фазы 2 это означает: `windows_runtime_probe_methods` для
`OutlookWordEditor`, `OutlookSearch` и `OutlookShell` должен совпадать с
ожидаемым probe contract и не включать generic UIA/clipboard/send_input.
Если `resolver_contracts.tsv` допускает метод, который runtime probe contract
не допускает, в фазе 8 нужно явно решить, это осознанный synthetic fallback
для unit-тестов или нарушение high-risk boundary.

После фазы 7 не использовать confidence как единственную защиту Outlook.
Граница Outlook должна оставаться явной через `ProbePolicy`, `SurfacePolicy`,
`probe_contracts.tsv` и `resolver_contracts.tsv`.

В `outlookhaging.md` добавить ссылку на `ProbePolicy`, `probe_contracts.tsv` и
resolver contracts, если такой ссылки еще нет. Заодно сверить формулировку
"UIA запрещен" с реальным состоянием `SurfacePolicy`/`ProbePolicy`: для
Outlook важнее не оставлять двусмысленность между runtime probe запретом и
resolver-level запретом.

### Критерий готовности

- Outlook не получает generic fallback.
- Outlook contracts явно проверяют probes и resolver.

## Фаза 9. Documentation and developer workflow - выполнена

### Цель

Чтобы следующий разработчик не вернул старую проблему.

### Анализ реализации

Фаза 9 закрыта документационно, без изменения runtime-логики:

- `README.md` уже описывает архитектуру method adapters, risky/fallback
  ограничения и проверенные приложения, а также ссылается на
  `docs/development_commands_ru.md` как на developer workflow;
- `docs/development_commands_ru.md` содержит короткое правило, какие
  контракты и policy нужно менять при правке adapter-а;
- команда `diagnose-focus --delay 3 --methods --surface` вынесена в
  developer workflow как основной способ увидеть classifier, probe policy и
  resolver trace;
- `docs/adapter_architecture_stability_review_ru.md` содержит актуальную
  итоговую ревизию устойчивости после фаз 1-9;
- старый `docs/adapter_isolation_review_ru.md` удален как устаревший
  промежуточный документ.

Качество реализации достаточное: фаза не трогает runtime-код, не меняет
поведение приложений и фиксирует главное правило - не возвращать app-specific
ветвления в adapter probe.

### Файлы

- `README.md`
- `docs/development_commands_ru.md`
- `docs/adapter_architecture_stability_review_ru.md`
- этот файл

### Что зафиксировано в документации

Короткое правило:

При добавлении или изменении адаптера нужно обновить:

1. `AdapterContract`, если меняются возможности метода;
2. `ProbePolicy`, если метод должен или не должен пробоваться на surface;
3. `SurfacePolicy`, если метод должен или не должен выбираться resolver’ом;
4. resolver/probe fixture, если меняется проверенная surface;
5. behavior fixture, если меняется range/caret/selection behavior.

Если меняется classifier или `SurfaceKind`, сначала обновить
`probe_contracts.tsv`, затем `resolver_contracts.tsv`. Если один из контрактов
падает, не править адаптер до понимания, какой surface contract реально
изменился.

После фазы 7 дополнительное правило: для каждой новой или измененной
проверенной surface в `probe_contracts.tsv` должен быть осмысленный
`min_confidence`. Не занижать его "чтобы тест прошел": если confidence упала,
сначала понять, это classifier стал грубее или fixture описывает другой
surface. Runtime confidence-gating добавлять только отдельной фазой/коммитом,
после manual smoke соответствующих приложений.

Если surface уже есть в `resolver_contracts.tsv`, но получает новый title,
process или language-specific вариант, проверить, нужен ли такой же row в
`probe_contracts.tsv`, чтобы classifier/probe confidence не остались
непокрытыми.

Если меняется Windows probe collection, сначала проверить, что это нельзя
выразить через `ProbePolicy`. После фазы 2 нормальный путь такой:

1. изменить `ProbePolicy`/classifier;
2. обновить `probe_contracts.tsv`;
3. только затем трогать adapter probe, если не хватает технической проверки.

После фазы 3 дополнительное правило: если хочется добавить проверку приложения
в `WebKeyboardSelectionMethod.probe` или `XtermKeyboardSelectionMethod.probe`,
сначала решить, это policy или technical predicate.

- policy: менять `surface.rs` + `probe_contracts.tsv`;
- technical: менять `web_keyboard` support helper + focused Windows test;
- смешанный случай: разделить на два маленьких коммита.

После фазы 4 дополнительное правило для web keyboard:

- profile/timing/control-prefix менять в `web_keyboard_profile.rs`;
- техническую пригодность target менять в `web_keyboard_support.rs`;
- разрешение или запрет метода на приложении менять только в
  `surface.rs`/`ProbePolicy`;
- не добавлять `classify_surface` обратно в `web_keyboard.rs` или
  `web_keyboard_support.rs`.
- при добавлении нового technical predicate в `web_keyboard_support.rs`
  обязательно добавить:
  - один positive Windows unit test на сам predicate/probe;
  - один negative test, что неизвестная или соседняя surface не получила новый
    runtime method;
  - `probe_contracts.tsv` row, если surface уже считается проверенной.
- если нужна новая скорость/тайминги для web keyboard, сначала добавить новый
  `WebKeyboardProfile` в `surface.rs`, затем contract rows, и только потом
  менять `web_keyboard_profile.rs`. Не кодировать тайминги по title/process в
  `web_keyboard.rs`.

После фазы 5 дополнительное правило для логов:

- новые diagnostic поля добавлять как optional fields;
- не менять смысл `expected_before_text` ради диагностики, потому что
  пользователь уже смотрит это поле в tail-логах;
- не добавлять resolver/probe trace в successful `Completed` и
  `HotkeyReceived` events без отдельной причины;
- если нужна точная стадия fail (`context_capture`, `plan`, `apply`), сначала
  расширить `OperationError`/runner stage и покрыть это тестом. Не угадывать
  stage по тексту ошибки.

Команда диагностики:

```powershell
F:\distr\system\Stepler\dist\Stepler\stepler-cli.exe diagnose-focus --delay 3 --methods --surface
```

### Критерий готовности - выполнен

- Документация объясняет, куда вносить изменения.
- Нет инструкции "просто добавить if по приложению в адаптер".
- README/review/development docs не противоречат друг другу по роли policy,
  probe и adapter probe.

## Финальная проверка всей работы

После всех фаз выполнить:

```powershell
cargo fmt
cargo test -p stepler-core -p stepler-platform -p stepler-platform-windows -p stepler-cli
cargo build -p stepler-cli --release --target-dir target-codex-release
```

Если есть доступ к .NET SDK и `dist` не заблокирован:

```powershell
$env:PATH = (Join-Path $PWD '.dotnet-sdk') + ';' + $env:PATH
$env:CARGO_TARGET_DIR = (Join-Path $PWD 'target-codex-release')
.\scripts\build-release.ps1
```

После release-сборки запустить Stepler из `dist` вне sandbox.

## Manual smoke после архитектурных фаз

Проверить минимум:

- Notepad: `P`, `CP`, no selection, selected text, caret after `CP`.
- PowerShell обычный: `P`, `CP`.
- PowerShell внутри Codex app: `P`, `CP`.
- Codex Windows app input: `P`, `CP`, selected/no selection.
- Confluence/Jira web: comment/edit field, selected/no selection.
- Rocket.Chat search: selected/no selection.
- Sticky Notes: selected/no selection.
- Outlook 2016 search: аккуратно, короткий smoke.
- Outlook 2016 compose: аккуратно, короткий smoke.
- Qwen input/workspace: `P`, `CP`, submit.

Если Outlook начинает подвисать, не расширять fallback. Сначала собрать логи и
свериться с `outlookhaging.md`.

## Что нельзя делать в рамках этого плана

- Нельзя заменять resolver простым if/else по приложениям.
- Нельзя добавлять app-specific проверки в `WebKeyboardSelectionMethod` ради
  быстрого фикса.
- Нельзя включать `TerminalClipboardShortcut` для Qwen.
- Нельзя включать generic `ClipboardSelection` / `SendInput` для browser/editor
  surfaces без отдельного explicit contract.
- Нельзя удалять preflight/verification из keyboard-selection методов ради
  скорости.
- Нельзя менять core алгоритм n-грамм/вероятности в этом плане.

## Итог после фаз 1-9

После выполнения всех фаз:

- список probes станет явным и тестируемым;
- adapter probe перестанет быть местом app policy;
- fast web оптимизации уже не скрывают fallback после фазы 2;
- runtime fail logs станут достаточно информативными;
- resolver contracts будут жить в fixtures;
- confidence classifier’а будет зафиксирован тестами;
- Outlook останется отдельной high-risk поверхностью с явными границами.

Фазы 1-9 закрыты. Runtime/contract часть закреплена кодом и fixtures,
документационный workflow теперь объясняет, куда вносить изменения и как не
возвращать старое смешение policy и technical adapter checks.

## План будущего hardening после фаз 1-9

Этот раздел - точное ТЗ для следующего исполнителя. Выполнять строго по одной
фазе за раз. Не объединять фазы в один коммит. После каждой фазы обновлять этот
файл: помечать фазу как выполненную, кратко писать что изменено, какие тесты
запущены и какие риски остались.

Подробная мотивация рисков описана в
`docs/adapter_architecture_stability_review_ru.md`. Если есть противоречие
между ревизией и этим планом, рабочим источником считать этот план.

### Фаза 10. Policy consistency invariants - выполнена

#### Цель

Добавить автоматические тесты, которые проверяют согласованность
`ProbePolicy`, `SurfacePolicy`, risky/forbidden methods и preferences для всех
`SurfaceKind`. Эта фаза не должна менять runtime behavior.

#### Что сделано

- Добавлен integration test
  `crates/stepler-platform/tests/policy_invariants.rs`.
- Тест проверяет, что для каждого `ProbePolicy`:
  - `probe_methods` и `suppressed_methods` не содержат дублей;
  - оба списка не пересекаются;
  - вместе они покрывают все `ALL_METHOD_IDS`;
  - все методы входят в известный список методов.
- Тест проверяет, что для каждого `SurfacePolicy`:
  - pause/scrolllock context/replace preferences ссылаются только на известные
    методы;
  - forbidden methods не пересекаются с preferred replacement methods;
  - risky preferred methods требуют явного surface allowance;
  - каждый probe policy имеет matching surface policy.
- Production policy values, classifier и порядок методов не менялись.

#### Проверено

```powershell
cargo fmt
cargo test -p stepler-platform
cargo test -p stepler-core -p stepler-platform -p stepler-platform-windows -p stepler-cli
```

#### Файлы

- `crates/stepler-platform/src/surface.rs`
- `crates/stepler-platform/tests/probe_matrix.rs`
- возможно новый файл `crates/stepler-platform/tests/policy_invariants.rs`
- этот файл

#### Порядок действий

1. Открыть `crates/stepler-platform/src/surface.rs`.
2. Проверить, что публично доступны или могут быть безопасно использованы в
   tests:
   - `default_surface_policies()`;
   - `default_probe_policies()`;
   - `surface_policy_for(SurfaceKind)`;
   - `probe_policy_for(SurfaceKind)`;
   - `surface_allows_risky_method(SurfaceKind, MethodId)`;
   - `ALL_METHOD_IDS`.
3. Если для тестов не хватает маленького helper-а, добавить его в
   `surface.rs`. Не менять policy values в этой фазе.
4. Добавить integration test `policy_invariants.rs`.
5. В тесте перечислить все `SurfaceKind`, которые есть в
   `default_probe_policies()`, и для каждого surface проверить:
   - каждый `probe_methods` элемент входит в `ALL_METHOD_IDS`;
   - каждый `suppressed_methods` элемент входит в `ALL_METHOD_IDS`;
   - `probe_methods` и `suppressed_methods` не пересекаются;
   - `probe_methods + suppressed_methods` покрывают все `ALL_METHOD_IDS`;
   - `forbidden_methods` не пересекаются с allowed replacement preferences
     без явного комментария/exception в тесте;
   - risky method не может быть выбран, если `allow_risky_methods=false`;
   - если `allow_risky_methods=true`, risky method должен также проходить
     `surface_allows_risky_method(surface, method)`.
6. Добавить отдельный test для preferences:
   - все `pause_methods.context_methods`;
   - все `pause_methods.replace_methods`;
   - все `scrolllock_methods.context_methods`;
   - все `scrolllock_methods.replace_methods`
   должны входить в `ALL_METHOD_IDS`.
7. Добавить explicit exceptions только если они уже существуют в архитектуре.
   Пример допустимого exception: `PsReadLine` присутствует в policy как bridge,
   но не является обычным Windows capture/apply adapter.
8. Запустить тесты.
9. Обновить этот раздел: заменить заголовок на
   `### Фаза 10. Policy consistency invariants - выполнена`, добавить
   "Что сделано" и "Проверено".

#### Что нельзя делать

- Нельзя менять порядок методов в `SurfacePolicy` или `ProbePolicy`.
- Нельзя менять `SurfaceKind` classifier.
- Нельзя чинить падающий invariant изменением policy, пока не понятно, это
  реальная ошибка или допустимое исключение.
- Нельзя удалять risky methods из `ALL_METHOD_IDS`.

#### Проверка

```powershell
cargo fmt
cargo test -p stepler-platform
```

Перед завершением всей серии фаз:

```powershell
cargo test -p stepler-core -p stepler-platform -p stepler-platform-windows -p stepler-cli
```

#### Критерий готовности

- Добавлен invariant test для согласованности probe/surface policy.
- Фаза не изменила runtime behavior.
- `cargo test -p stepler-platform` зеленый.

### Фаза 11. Conservative Unknown surface

#### Цель

Сделать обычный `SurfaceKind::Unknown` fail-closed или почти fail-closed, чтобы
новые неизвестные приложения не получали широкий набор fallback-методов.
Широкий diagnostic probing оставить только под явным флагом.

#### Файлы

- `crates/stepler-platform/src/surface.rs`
- `crates/stepler-platform/tests/fixtures/probe_contracts.tsv`
- `crates/stepler-platform/tests/fixtures/resolver_contracts.tsv`
- `crates/stepler-platform/tests/probe_matrix.rs`
- `crates/stepler-platform/tests/resolver_matrix.rs`
- возможно `crates/stepler-platform-windows/src/diagnostics.rs`
- `docs/development_commands_ru.md`
- этот файл

#### Порядок действий

1. До правок запустить:

   ```powershell
   cargo test -p stepler-platform
   ```

2. В `surface.rs` найти `default_probe_policies()` и policy для
   `SurfaceKind::Unknown`.
3. Заменить обычный unknown probe stack с `ALL_METHOD_IDS` на минимальный
   безопасный набор. Рекомендуемый первый вариант:
   - `UiAutomationEditableText`
   - `UiAutomationDocumentText`
   - `UiAutomationText`

   Если этот набор ломает существующие verified apps, значит classifier
   неверно относит их к `Unknown`; сначала добавить/исправить surface contract,
   а не расширять `Unknown`.

4. В `default_surface_policy()` также сузить preferences до того же безопасного
   набора. Не включать:
   - `TerminalClipboardShortcut`;
   - `ClipboardSelection`;
   - `SendInput`;
   - `ConsoleBuffer`;
   - `PsReadLine`;
   - `WebKeyboardSelection`
   без отдельного explicit surface contract.
5. Добавить helper в `surface.rs`, например:

   ```rust
   fn unknown_allows_diagnostic_probe() -> bool {
       std::env::var_os("STEPLER_DIAGNOSTIC_UNKNOWN_PROBES").is_some()
   }
   ```

   Имя можно выбрать другое, но оно должно быть явно diagnostic.

6. Если diagnostic flag включен, `probe_policy_for(SurfaceKind::Unknown)` может
   возвращать широкий `ALL_METHOD_IDS`. Без флага - только safe-minimal stack.
7. Обновить `probe_contracts.tsv` строку `unknown` под обычный режим:
   - `expected_probe_methods` = safe-minimal stack;
   - `expected_suppressed_methods` = все остальные методы;
   - `min_confidence` оставить `10`;
   - `fast_probe=false`.
8. Если `resolver_contracts.tsv` содержит unknown row, обновить его. Если не
   содержит - не добавлять без необходимости.
9. Добавить/обновить tests:
   - обычный unknown не содержит risky methods;
   - diagnostic unknown под env flag возвращает широкий stack;
   - checked verified surfaces не изменились.
10. В `docs/development_commands_ru.md` добавить короткую заметку: если новое
    приложение стало `Unknown`, правильный путь - добавить surface contract, а
    не расширять `Unknown`.
11. Запустить тесты.
12. Обновить этот раздел: статус, что сделано, проверки.

#### Что нельзя делать

- Нельзя расширять `Unknown`, чтобы быстро починить одно приложение.
- Нельзя включать clipboard/terminal/send_input fallback в обычный `Unknown`.
- Нельзя менять classifier для проверенных приложений без обновления
  `probe_contracts.tsv` и `resolver_contracts.tsv`.
- Нельзя использовать diagnostic env flag в production behavior по умолчанию.

#### Проверка

```powershell
cargo fmt
cargo test -p stepler-platform
cargo test -p stepler-platform-windows
```

Если менялся Windows diagnostics path:

```powershell
cargo test -p stepler-platform -p stepler-platform-windows
```

#### Критерий готовности

- `Unknown` без diagnostic флага не получает risky fallback.
- Verified surfaces из `probe_contracts.tsv` не изменили surface/probe stack.
- Есть test на diagnostic unknown mode.

### Фаза 12. TargetFacts для classifier/support predicates

#### Цель

Убрать дублирование признаков target между `surface.rs` и
`web_keyboard_support.rs`, не возвращая app-policy в adapters.

#### Файлы

- `crates/stepler-platform/src/surface.rs`
- возможно новый файл `crates/stepler-platform/src/target_facts.rs`
- `crates/stepler-platform/src/lib.rs`
- `crates/stepler-platform-windows/src/web_keyboard_support.rs`
- `crates/stepler-platform-windows/src/web_keyboard_profile.rs`
- `crates/stepler-platform-windows/src/tests.rs`
- `crates/stepler-platform/tests/fixtures/probe_contracts.tsv`
- этот файл

#### Порядок действий

1. До правок запустить:

   ```powershell
   cargo test -p stepler-platform -p stepler-platform-windows
   ```

2. В `stepler-platform` добавить маленькую структуру facts, например:

   ```rust
   pub struct TargetFacts {
       pub is_windows_terminal: bool,
       pub is_browser_like: bool,
       pub is_fast_browser_title: bool,
       pub is_rocket_chat: bool,
       pub is_yandex_browser: bool,
       pub is_telegram: bool,
       pub is_sticky_notes: bool,
       pub is_notepad_like: bool,
       pub is_outlook_process: bool,
       pub is_word_process: bool,
   }
   ```

   Точный список можно сократить, если часть facts не используется. Не делать
   generic map/string bag.

3. Добавить функцию:

   ```rust
   pub fn target_facts(target: &ForegroundTarget) -> TargetFacts
   ```

4. Переписать `classify_surface(target)` так, чтобы он использовал
   `target_facts(target)`, но сохранял прежний порядок classification.
5. Переписать `web_keyboard_support.rs`, чтобы technical predicates читали
   facts вместо копирования проверок title/process/class. Пример:

   ```rust
   pub(super) fn is_web_keyboard_technical_target(target: &ForegroundTarget) -> bool {
       let facts = target_facts(target);
       facts.is_browser_like || facts.is_telegram || facts.is_notepad_like || facts.is_sticky_notes
   }
   ```

6. Не переносить `ProbePolicy`, `SurfacePolicy` или `WebKeyboardProfile` в
   `web_keyboard_support.rs`.
7. Не менять строки в `probe_contracts.tsv`, если behavior реально не
   изменился. Если тест падает, сначала проверить, сохранился ли порядок
   classification.
8. Добавить focused tests:
   - facts для Rocket.Chat title/process;
   - facts для Codex/Jira/Confluence fast browser title;
   - facts для Sticky Notes;
   - facts для Telegram;
   - negative case: unknown Chromium-like соседняя surface не получает лишний
     technical predicate без policy.
9. Запустить тесты.
10. Обновить этот раздел: статус, что сделано, проверки.

#### Что нельзя делать

- Нельзя превращать `TargetFacts` в новый resolver.
- Нельзя хранить в `TargetFacts` выбранный `MethodId`.
- Нельзя переносить policy в Windows adapter слой.
- Нельзя менять web keyboard timing/profile в этой фазе.

#### Проверка

```powershell
cargo fmt
cargo test -p stepler-platform
cargo test -p stepler-platform-windows
```

#### Критерий готовности

- Признаки target считаются в одном месте.
- `classify_surface` и `web_keyboard_support` используют общий facts layer.
- Probe/resolver fixture tests не изменили ожидаемые checked surfaces.

### Фаза 13. MethodBinding invariant

#### Цель

Зафиксировать, что production capture paths всегда возвращают
`TextContext.capabilities.method_binding`, а `apply_replacement` не выбирает
adapter самовольно для новых runtime contexts.

#### Файлы

- `crates/stepler-core/src/types.rs`
- `crates/stepler-platform-windows/src/lib.rs`
- Windows adapter files:
  - `win32_edit.rs`
  - `uia_text.rs`
  - `web_keyboard.rs`
  - `word_com.rs`
  - `console_buffer.rs`
  - `terminal_clipboard.rs`
  - `clipboard_selection.rs`
  - `send_input.rs`
- `crates/stepler-platform-windows/src/tests.rs`
- этот файл

#### Порядок действий

1. До правок запустить:

   ```powershell
   cargo test -p stepler-platform-windows
   ```

2. Найти все production `capture` функции, которые создают `TextContext`.
3. Для каждой capture функции проверить, что она заполняет:

   ```rust
   capabilities.method_binding = Some(MethodBinding::new(context_method, vec![replacement_method]))
   ```

   Обычно `context_method == replacement_method`. Исключения допустимы только
   если resolver реально выбрал split binding.

4. Добавить helper в Windows tests, который вызывает capture/test context
   builder для каждого adapter, где это возможно без live UI. Для live-only
   adapters добавить unit-level test на builder/helper, который формирует
   `TextContext`.
5. Добавить test: production-like contexts must have method binding.
6. В `apply_replacement` заменить silent fallback на explicit behavior:
   - если `method_binding` есть - использовать его;
   - если `method_binding` нет, разрешить legacy fallback только для явно
     помеченных test/compat contexts.
7. Если сразу удалить legacy fallback рискованно, добавить временный guard:

   ```rust
   if context.capabilities.method_binding.is_none() {
       return Err(PlatformError::ReplacementUnavailableReason("missing_method_binding".into()));
   }
   ```

   Но перед этим проверить tests. Если старые tests создают context без
   binding, исправить tests, а не runtime.

8. В runtime fail log не менять `expected_before_text`; если нужно добавить
   диагностику, использовать существующий trace/fail summary mechanism.
9. Запустить тесты.
10. Обновить этот раздел: статус, что сделано, проверки.

#### Что нельзя делать

- Нельзя возвращать fallback `None => Win32EditMessagesMethod.apply` для новых
  production contexts.
- Нельзя угадывать replacement adapter по `control_id`, кроме временного
  compat path с явным test/comment.
- Нельзя менять replacement plan/range logic.
- Нельзя менять behavior fixtures, если behavior реально не должен измениться.

#### Проверка

```powershell
cargo fmt
cargo test -p stepler-core
cargo test -p stepler-platform-windows
cargo test -p stepler-core -p stepler-platform -p stepler-platform-windows -p stepler-cli
```

#### Критерий готовности

- Все production capture paths возвращают `method_binding`.
- `apply_replacement` не выбирает adapter через неявный default для новых
  runtime contexts.
- Missing binding дает понятную fail-причину.

## Финальная проверка после фаз 10-13

После выполнения всех фаз 10-13:

```powershell
cargo fmt
cargo test -p stepler-core -p stepler-platform -p stepler-platform-windows -p stepler-cli
cargo build -p stepler-cli --release --target-dir target-codex-release
```

Затем выполнить manual smoke из раздела "Manual smoke после архитектурных фаз".
Если менялась release-сборка, запустить Stepler из `dist` вне sandbox по
правилам из `AGENT.md`.
