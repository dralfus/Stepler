# Итоговая ревизия устойчивости архитектуры адаптеров

Дата: 2026-06-23

## Короткий вывод

После фаз 1-9 архитектура Stepler стала заметно менее хрупкой. Главный старый
риск - "починили одно приложение, сломали другое" - теперь ограничен слоями:

- `SurfaceKind` и classifier описывают активную поверхность;
- `ProbePolicy` решает, какие методы вообще можно пробовать;
- `SurfacePolicy` решает, какие методы resolver может выбрать для `P` и `CP`;
- adapters в основном отвечают за техническую возможность метода, а не за
  app-policy;
- probe/resolver/behavior fixtures фиксируют проверенные поверхности;
- diagnose/runtime logs показывают surface, probe plan, resolver path и причину
  fail.

Текущее состояние можно считать рабоче-устойчивым для дальнейшей разработки.
Но полностью "нехрупкой" архитектура станет после еще нескольких небольших
hardening-шагов. Большой рефакторинг сейчас не нужен; нужны точечные защитные
инварианты.

## Что стало устойчивым

### 1. Policy отделена от adapters

`ProbePolicy` и `SurfacePolicy` теперь являются основными местами, где живет
решение "какой метод разрешен для какого приложения". Это правильно:

- изменение Rocket.Chat search больше не должно требовать правки generic
  browser adapter-а;
- Outlook/Zimbra защищен отдельной policy boundary;
- Qwen/terminal TUI не получают generic terminal clipboard fallback;
- risky methods не включаются для неизвестных приложений без явной policy.

### 2. Probe stage стал тестируемым

`probe_contracts.tsv` фиксирует:

- ожидаемый `SurfaceKind`;
- runtime probe methods;
- suppressed methods;
- minimum classifier confidence;
- fast probe flag.

Это закрывает прежний риск, когда Windows probe collection скрыто выкидывала
fallback до resolver-а.

### 3. Resolver stage стал тестируемым

`resolver_contracts.tsv` фиксирует выбранные context/replacement methods и
запрещенные методы для проверенных поверхностей.

Это важно, потому что resolver теперь можно менять без угадывания: если
поведение проверенного приложения изменилось, matrix test должен показать где.

### 4. Web keyboard уже не один большой условный блок

`web_keyboard_profile.rs` и `web_keyboard_support.rs` отделили:

- timing/profile;
- technical target predicate;
- собственно capture/apply logic.

Это снизило риск, что ускорение Codex/Jira/Confluence заденет Rocket.Chat или
Sticky Notes.

### 5. Core behavior защищен отдельно

`replacement_behavior.tsv` фиксирует range/caret/selection поведение на уровне
`stepler-core`. Это правильная нижняя страховка: даже если меняется adapter,
выбор текста для замены не должен внезапно переехать.

## Оставшиеся риски

### Риск 1. `Unknown` surface слишком широкая

Сейчас `Unknown` в `ProbePolicy` пробует все методы, а default surface policy
может включать широкий список методов. Это удобно для диагностики и старых
fallback-сценариев, но архитектурно это самый вероятный будущий источник
регрессий.

Почему это риск:

- новое приложение может случайно попасть в `Unknown`;
- technical probe какого-то метода может стать шире;
- resolver получит метод, который не был явно разрешен для этой surface.

Рекомендация:

- сделать `Unknown` fail-closed или почти fail-closed;
- оставить широкий unknown-probe только в явном diagnostic/risky режиме;
- для нового приложения сначала добавлять `SurfaceKind` + probe/resolver
  contracts, и только потом разрешать метод.

Приоритет: высокий.

### Риск 2. `ProbePolicy` и `SurfacePolicy` могут разойтись

Обе policy живут рядом, но их согласованность пока держится в основном через
fixtures проверенных приложений. Нужен отдельный invariant test на все
`SurfaceKind`.

Что тест должен проверять:

- каждый `probe_method` либо присутствует в pause/scrolllock preferences, либо
  явно перечислен как synthetic/bridge exception;
- forbidden method не присутствует в runtime probe plan без осознанной причины;
- risky method разрешен только если `allow_risky_methods=true` и surface
  явно допускает этот risky method;
- `SurfaceKind::Unknown` не получает широкий runtime stack в обычном режиме
  после исправления риска 1.

Приоритет: высокий.

### Риск 3. Technical predicates частично дублируют classifier

`surface.rs` и `web_keyboard_support.rs` оба знают признаки browser-like,
Telegram, Sticky Notes, Notepad-like target. Сейчас это разделено по смыслу:

- classifier решает surface;
- support predicate решает, технически похож ли target на подходящий control.

Но признаки похожи, и со временем они могут разойтись.

Рекомендация:

- ввести маленький platform-neutral `TargetTraits` / `TargetFacts`;
- classifier и technical predicates должны читать одни и те же facts;
- policy все равно остается только в `surface.rs`.

Важно: это не должно превращаться в новый большой app-router. Это просто общий
словарь фактов о target.

Приоритет: средний.

### Риск 4. Apply fallback может обходить resolver при пустом method binding

`apply_replacement` сначала использует `context.capabilities.method_binding`.
Но если binding отсутствует, остаются legacy fallback branches по `control_id`
или Win32 default.

Это полезно для старой совместимости, но архитектурно опасно: capture method
может забыть поставить binding, а apply уйдет в другой adapter.

Рекомендация:

- добавить invariant test: все production capture paths должны возвращать
  `method_binding`;
- оставить legacy apply fallback только для test/compat contexts или удалить
  после миграции;
- в runtime fail log явно писать `missing_method_binding`, если такое случится.

Приоритет: средний.

### Риск 5. Terminal adapters остаются особым control-plane

PSReadLine, SSH remote helper и Qwen input/workspace не являются обычными
TextContext adapters в том же смысле, что Win32/UIA/WebKeyboard.

Сейчас это в целом нормально: risky terminal clipboard fallback запрещен там,
где он опасен. Но для будущей поддержки terminal TUI стоит явно сохранить
терминальные адаптеры как отдельный bridge/control-plane класс, а не пытаться
натянуть их на generic clipboard selection.

Рекомендация:

- документировать terminal bridge adapters отдельно от generic text adapters;
- не включать `TerminalClipboardShortcut` для новых terminal surfaces без
  отдельного contract и manual smoke;
- для новых TUI предпочитать side-channel/интеграционный adapter, как Qwen
  input/workspace или remote helper.

Приоритет: средний.

### Риск 6. UI/tray lifecycle не покрыт adapter contracts

Фазы 1-9 хорошо защищают adapter/resolver слой, но не защищают tray process,
runner restart, overlay indicator и settings UI. Это другой класс устойчивости.

Рекомендация:

- держать отдельный smoke checklist для tray lifecycle;
- при изменении tray/runner запускать Stepler из `dist` вне sandbox и проверять,
  что `Stepler.exe` и `stepler-cli.exe` живут через несколько секунд;
- не смешивать tray lifecycle fixes с adapter policy changes.

Приоритет: низкий для adapter isolation, средний для продукта в целом.

## Нужно ли продолжать архитектурный hardening

Да, но маленькими шагами. Не нужен большой перепил. Лучший следующий порядок:

1. **Фаза 10. Policy consistency invariants.**
   Добавить тесты согласованности `ProbePolicy`, `SurfacePolicy`,
   risky/forbidden methods и method preferences для всех `SurfaceKind`.

2. **Фаза 11. Conservative Unknown surface.**
   Сузить обычный `Unknown` surface до fail-closed/minimal-safe поведения.
   Широкий probe оставить только под явным diagnostic флагом.

3. **Фаза 12. TargetFacts для classifier/support predicates.**
   Убрать дублирование признаков target между `surface.rs` и
   `web_keyboard_support.rs`, не перенося policy обратно в adapters.

4. **Фаза 13. MethodBinding invariant.**
   Зафиксировать, что production capture всегда возвращает replacement method
   binding, а apply не выбирает adapter самовольно.

Этого достаточно, чтобы архитектура стала устойчивой, а не просто менее
хрупкой. Остальные улучшения лучше делать только при появлении реального бага.

## Что больше не актуально

Старый `docs/adapter_isolation_review_ru.md` больше не нужен как отдельный
документ: его роль заменена этим итоговым review и
`docs/adapter_isolation_hardening_plan_ru.md`.

Источники правды после этой ревизии:

- `docs/adapter_isolation_hardening_plan_ru.md` - фазы, правила и будущие
  hardening-шаги;
- `docs/adapter_architecture_stability_review_ru.md` - текущая архитектурная
  оценка после фаз 1-9;
- `docs/development_commands_ru.md` - короткий workflow для разработчика;
- `README.md` - обзор архитектуры и проверенных приложений.
