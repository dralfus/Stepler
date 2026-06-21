# План разработки Stepler

Stepler - самостоятельная системная утилита для исправления текста, набранного в неверной раскладке, управления горячими клавишами и безопасной работы с активным приложением.

## 1. Цели проекта

- Стабильно исправлять текст по `Pause` и `Ctrl+Pause`.
- Не портить пользовательский буфер обмена.
- Не делать двойных вставок через 1-4 секунды после hotkey.
- Не зависеть от фиксированных задержек там, где можно дождаться реального события.
- Поддерживать разные приложения через единый контракт доступа к тексту.
- Поддержать Windows сначала, но заложить портирование на Linux.
- Обеспечить работу минимум в:
  - Notepad;
  - Microsoft Word;
  - PowerShell/Windows Terminal;
  - Codex;
  - Windsurf.

## 2. Технологический выбор

### 2.1 Язык и ядро

Основной язык: Rust.

Причины:

- строгая модель владения помогает уменьшить скрытые состояния и гонки;
- удобно отделить платформенное ядро от UI;
- хорошо подходит для системных утилит;
- можно собрать общее core-ядро для Windows и Linux.

### 2.2 UI

UI должен быть тонким слоем поверх ядра.

Допустимые варианты:

- Tauri для tray/settings/log viewer;
- Avalonia-like UI, если будет выбран .NET-hosted UI вокруг Rust core;
- другой desktop UI, если он не ломает архитектурное разделение.

UI не должен содержать бизнес-логику исправления текста.

### 2.3 Репозиторная структура

Предлагаемая структура:

```text
stepler/
  crates/
    stepler-core/
    stepler-platform/
    stepler-platform-windows/
    stepler-platform-linux/
    stepler-app/
  apps/
    Stepler.Tray/
  docs/
```

## 3. Архитектура: method adapters + policies + resolver

Главная цель архитектуры: избежать зоопарка app-specific adapters. Stepler должен поддерживать приложения через ограниченный набор method adapters, а различия между приложениями описывать маленькими policy-правилами.

Runtime flow:

```text
Foreground target
  -> capability probes
  -> app policy
  -> context method
  -> CorrectionEngine
  -> replacement method
  -> transaction/preflight/verify
```

Текущий статус реализации:

- `MethodId` и `MethodBinding` добавлены в `stepler-core`;
- `MethodProbe`, `ProbeSafety`, `AppPolicy`, `ForegroundTarget`, `MethodResolver` добавлены в `stepler-platform`;
- Windows provider уже строит `ForegroundTarget`, собирает probes и использует `MethodResolver` для выбора безопасных методов или отказа от risky fallback (`TerminalClipboardShortcut`, `ClipboardSelection`, `SendInput`) по app policy;
- PowerShell реализован как отдельный `PSReadLine` method adapter через `scripts/Stepler.PSReadLine.ps1` и `stepler-cli psreadline-plan`;
- следующий рефактор: выделить concrete `ContextMethod`/`ReplacementMethod` traits и перенести `Win32EditMessages` из функций `text_context/apply_replacement` в самостоятельный method adapter.

Слои:

1. Method adapters - реальные техники доступа:
   - v `Win32EditMessages`;
   - v `UIAutomationEditableText` (strict focused editable UIA element);
   - v `UIAutomationDocumentText` (UIA TextPattern document surface: selection + strict caret-range fallback);
   - v `UIAutomationText`;
   - v `ClipboardSelection` (risky fallback: работает только с уже выделенным текстом через clipboard copy/paste);
   - v `SendInput` (risky replacement fallback: ввод Unicode-текста в текущее выделение, не умеет читать контекст);
   - v `WordCom`;
   - v `PSReadLine`; реализован через CLI/PowerShell host
   - v `ConsoleBuffer`;
   - v `TerminalClipboardShortcut`.
2. App policies - небольшие таблицы предпочтений/запретов для app/window/process:
   - preferred context methods;
   - preferred replacement methods;
   - forbidden methods;
   - разрешение risky fallback методов;
   - timing/focus profile.
3. Runtime resolver - выбирает безопасную пару context method + replacement method по probe results и policy.
   - risky fallback методы по умолчанию запрещены и включаются только явным policy/режимом диагностики, например через `STEPLER_ALLOW_RISKY_FALLBACKS=1`.

### 3.1 Уровень 1: Capability Discovery и TextContextProvider

Назначение: получить текстовый контекст из активного приложения и явно указать method binding.

Контракт:

```text
TextContext {
  app_id,
  window_id,
  control_id,
  text_snapshot,
  caret_range,
  selection_range,
  capabilities {
    can_replace_directly,
    can_read_selection,
    can_read_caret,
    method_binding {
      context_method,
      replace_methods
    }
  }
}
```

Обязанности:

- определить активное окно;
- определить тип приложения;
- получить текст, caret и selection;
- сообщить, каким способом можно безопасно заменить текст;
- вернуть method id и список совместимых replacement methods;
- не принимать решений о том, что исправлять.

Method adapters:

- `Win32EditMessages`;
- `UIAutomationText` (первый безопасный слой: чтение через UIA `ValuePattern`/`TextPattern`, замена только через writable `ValuePattern.SetValue`);
- `ClipboardSelection` (risky fallback только для текущего выделения);
- `SendInput` (risky write-only replacement fallback);
- `WordCom`;
- `PSReadLine`;
- `ConsoleBuffer`;
- `TerminalClipboardShortcut`.

App policies:

- `Notepad`: prefer `Win32EditMessages`;
- `PowerShell`: prefer `PSReadLine`, forbid terminal clipboard shortcuts for normal operation;
- `Word`: prefer `WordCom`;
- unknown app: try safe probes first, risky clipboard/sendinput fallback только при явном разрешении.

Тесты:

- unit tests на нормализацию `TextContext`;
- integration tests на mock providers;
- Windows UI tests для Notepad/Codex/Windsurf;
- optional UI tests для Word, если Word установлен;
- terminal UI tests для PowerShell/Windows Terminal.

Критерий стабильности:

- provider всегда возвращает либо валидный `TextContext`, либо явную ошибку;
- provider не меняет текст и буфер обмена;
- provider логирует capability/method id, но не выполняет замену;
- resolver не должен скрещивать несовместимые context и replacement methods.

### 3.2 Уровень 2: CorrectionEngine

Назначение: чистая логика исправления текста.

Вход:

```text
TextContext
CorrectionMode: Pause | ScrollLock
LanguageModel
```

Выход:

```text
ReplacementPlan {
  range_start,
  range_end,
  replacement_text,
  reason,
  confidence,
  expected_before_text
}
```

Обязанности:

- определить слово/фразу для исправления;
- для `ScrollLock` распознавать текст в неверной раскладке прежде всего через статистическую модель языка: n-граммы, частотный анализ, языковой скоринг или другой сопоставимый вероятностный метод;
- использовать словари только как дополнительный сигнал, источник fixtures или вспомогательную проверку, но не как главный критерий решения;
- корректно работать с неизвестными словами, именами, техническими терминами, путями, командами и фрагментами кода;
- учитывать короткие слова и токены, например `git`, `на`, `in`; из-за низкой статистической информации внутри одного короткого токена решение по ним должно приниматься по контексту соседних слов, score всей фразы и confidence threshold;
- не знать ничего о Win32, clipboard, UI Automation, Word, Terminal;
- работать детерминированно на одинаковом входе;
- не иметь side effects.

Стартовые материалы для реализации:

- карты русской/английской раскладки из `PauseTransliterationHandler`;
- логику `ConvertLayoutText`;
- идеи из `LayoutMistypeDetector` после пересмотра в сторону частотного скоринга;
- данные `Data\NGrams` как стартовый источник n-грамм;
- словари `Data\Lexicons` только как вспомогательный ресурс, не как основной критерий `ScrollLock`;
- regression cases из ранее собранных тестовых сценариев.

Тесты:

- unit tests для `Pause`:
  - `k.,jdm` -> `любовь`;
  - выделенный текст из нескольких слов;
  - пунктуация;
  - caret в начале/середине/конце слова;
  - caret после пробелов.
- unit tests для `ScrollLock`:
  - `вальс поле long ghbdtn vbh`;
  - смешанный русский/английский текст;
  - несколько ошибочных слов подряд;
  - корректный английский префикс не меняется;
  - корректный русский текст не меняется.
- unit tests языковой модели:
  - русский текст имеет лучший русский n-граммный score, чем английский;
  - английский текст имеет лучший английский n-граммный score, чем русский;
  - текст после конвертации раскладки получает лучший score, чем исходный ошибочный текст;
  - неизвестное слово не должно автоматически считаться ошибкой только из-за отсутствия в словаре;
  - короткие токены `git`, `на`, `in` распознаются в контексте фразы и не игнорируются только из-за длины;
  - короткие токены не конвертируются агрессивно без достаточного контекстного confidence;
  - технический текст, пути и команды не должны агрессивно конвертироваться без достаточной уверенности.
- property tests:
  - диапазон замены всегда внутри текста;
  - `expected_before_text` совпадает с исходным фрагментом;
  - пустой input не падает.

Критерий стабильности:

- для всех известных багов есть отдельные test fixtures;
- любые изменения engine сначала проходят unit/property tests;
- engine не зависит от платформы.

### 3.3 Уровень 3: TextReplacer

Назначение: применить `ReplacementPlan` в конкретном приложении.

Контракт:

```text
ApplyReplacementRequest {
  context_id,
  original_snapshot_id,
  replacement_plan
}

ApplyReplacementResult {
  applied,
  actual_before_text,
  actual_after_text,
  method,
  error
}
```

Методы замены:

- прямой API контрола;
- UI Automation TextPattern/ValuePattern;
- Word COM/Office automation;
- terminal-specific input method;
- controlled clipboard fallback;
- synthetic keyboard input только как последний вариант.

Обязательная проверка перед заменой:

- активное окно не изменилось;
- control id не изменился;
- текст в `expected_before_text` всё ещё совпадает с целевым диапазоном;
- если проверка невозможна, adapter должен явно указать риск.

Тесты:

- unit tests на mock replacer;
- integration tests с fake provider + fake replacer;
- UI tests для Notepad:
  - Pause single word;
  - Pause selected text;
  - ScrollLock mixed line;
  - clipboard preserved.
- UI tests для Codex:
  - поле ввода сообщения;
  - `Pause` на последнем слове;
  - `ScrollLock` на текущей строке;
  - отсутствие двойной вставки.
- UI tests для Windsurf:
  - editor input;
  - chat/input panel, если доступен;
  - проверка, что hotkey не ломает обычные shortcuts IDE.
- UI tests для PowerShell:
  - не отправлять `Ctrl+C` как прерывание;
  - не вставлять старый clipboard;
  - безопасно отказываться, если контекст не подтвержден.
- UI tests для Word:
  - single word;
  - выделение нескольких слов;
  - строка/абзац с несколькими словами;
  - не вставлять весь документ вместо диапазона.

Критерий стабильности:

- замена либо применяется один раз, либо не применяется совсем;
- нет delayed paste после завершения операции;
- clipboard после операции равен исходному;
- adapter возвращает проверяемый результат.

### 3.4 Уровень 4: Safety/Transaction Layer

Назначение: сделать hotkey-операцию управляемой транзакцией.

Состояния:

```text
Idle
HotkeyReceived
ContextCaptured
PlanBuilt
PreflightChecked
ReplacementApplied
Verified
RolledBackOrFailed
Completed
```

Обязанности:

- блокировать повторный запуск той же операции до завершения;
- присваивать каждой операции `operation_id`;
- измерять длительность операции монотонным таймером от получения `Pause`/`ScrollLock` до финального состояния `Completed` или `RolledBackOrFailed`;
- собирать этапные метрики: получение контекста, построение плана, preflight, применение замены, verify, clipboard restore, общий duration;
- сохранять clipboard только если выбранный adapter реально использует clipboard;
- восстанавливать clipboard;
- проверять, что операция завершилась до разрешения следующего hotkey;
- логировать полный lifecycle операции.

Тесты:

- unit tests state machine;
- concurrency tests:
  - двойное нажатие hotkey;
  - hotkey во время ввода пользователя;
  - смена foreground window между чтением и заменой;
  - adapter timeout;
  - clipboard busy.
- unit tests метрик:
  - общий duration всегда не меньше суммы известных последовательных этапов;
  - failed/timeout операции тоже получают duration;
  - сбор метрик не меняет результат операции.
- integration tests с fake slow provider/replacer;
- UI tests на отсутствие двойной вставки через 1-4 секунды.

Критерий стабильности:

- нет параллельных операций для одного foreground control;
- все timeout/error paths завершаются в `Completed` или `RolledBackOrFailed`;
- в логах по `operation_id` можно восстановить весь путь обработки.

## 4. Портирование на Linux

### 4.1 Что должно быть общим

Общее для Windows и Linux:

- `stepler-core`;
- n-граммные/частотные языковые модели;
- словари как вспомогательные данные;
- алгоритм раскладки;
- алгоритм выбора диапазона;
- state machine транзакций;
- тестовые fixtures;
- часть UI.

### 4.2 Что будет платформенным

Windows:

- global keyboard hooks;
- foreground window detection;
- Win32/UI Automation;
- COM для Word;
- clipboard API;
- tray integration.

Linux:

- X11/Wayland global shortcuts;
- AT-SPI/accessibility APIs;
- clipboard через platform backend;
- app-specific adapters для терминалов и редакторов;
- tray/status notifier.

### 4.3 Риск Linux

Wayland может ограничивать глобальные hooks и synthetic input. Поэтому Linux-порт должен проектироваться как набор backend-ов:

- X11 backend;
- Wayland backend с ограничениями;
- desktop-environment-specific integrations при необходимости.

## 5. План разработки по этапам

### Этап 0. Инвентаризация стартовых сценариев

Задачи:

- выделить существующие сценарии;
- собрать все known bugs;
- собрать test phrases;
- описать текущие app-specific пути;
- выбрать, какие части переносить в Stepler.

Переиспользовать:

- n-граммные таблицы и/или другие частотные данные;
- словари как вспомогательные данные;
- таблицы раскладки;
- тестовые строки;
- known bugs как regression backlog.

Тесты:

- каждый known bug должен получить unit/smoke regression или явный статус `expected_failure` в `known_bugs.txt`.

Готово, когда:

- есть список сценариев;
- есть карта текущих модулей;
- есть backlog regression tests.

### Этап 1. Создание Rust workspace

Задачи:

- создать cargo workspace;
- добавить `stepler-core`;
- настроить CI/local commands;
- выбрать формат логов и regression-тестов.

Тесты:

- `cargo test --workspace`;
- smoke test пустого core API;
- lint/format check.

Готово, когда:

- workspace собирается;
- тестовый harness запускается;
- структура проекта не зависит от UI.

### Этап 2. Перенос CorrectionEngine

Задачи:

- перенести layout maps;
- реализовать конвертацию символов;
- реализовать языковую модель для ScrollLock на основе n-грамм/частотного скоринга русского и английского языка;
- реализовать сравнение score для исходного текста и текста после конвертации раскладки;
- ввести confidence threshold, ниже которого `ScrollLock` не должен менять текст;
- реализовать `Pause` plan builder;
- реализовать `ScrollLock` plan builder;
- подключить словари только как дополнительный сигнал и источник тестовых fixtures.

Тесты:

- regression fixtures из стартового набора;
- новые tests для `k.,jdm`, `xtnsht?`, `ghbdtn vbh`;
- tests на n-граммный скоринг и confidence threshold;
- tests на неизвестные слова, имена, команды и технические фрагменты;
- property tests на валидность диапазонов;
- regression tests из `known_bugs.txt`.

Готово, когда:

- engine на чистом тексте стабилен;
- все известные текстовые кейсы покрыты тестами;
- ни один тест не требует Windows API.

### Этап 3. Transaction state machine

Задачи:

- реализовать состояния операции;
- добавить `operation_id`;
- добавить timeout policy;
- добавить cancellation policy;
- добавить интерфейс clipboard guard.

Тесты:

- unit tests всех переходов;
- concurrency tests;
- simulated slow operation tests;
- double-hotkey tests.

Готово, когда:

- невозможно запустить две конфликтующие операции одновременно;
- timeout не оставляет операцию в подвешенном состоянии;
- состояние операции полностью отражается в логах.

### Этап 4. Windows platform skeleton

Задачи:

- реализовать foreground window detection;
- реализовать keyboard hotkey listener;
- реализовать clipboard snapshot/restore;
- реализовать логирование рядом с exe;
- подключить core engine без UI.

Тесты:

- unit tests platform wrappers через mocks;
- integration test clipboard snapshot/restore;
- manual smoke: hotkey регистрируется, но не меняет текст.

Готово, когда:

- hotkey events доходят до transaction layer;
- clipboard guard покрыт тестами;
- приложение не меняет текст без replacer.

### Этап 5. Notepad adapter

Задачи:

- реализовать provider для Win32 edit control;
- реализовать replacer по точному диапазону;
- проверять `expected_before_text` перед заменой;
- не использовать clipboard для результата.

Тесты:

- UI tests:
  - `k.,jdm` + Pause;
  - `k.,jdm` + ScrollLock;
  - `вальс поле long ghbdtn vbh` + ScrollLock;
  - выделение нескольких слов;
  - caret в середине слова;
  - clipboard preserved;
  - no delayed second paste.

Готово, когда:

- Notepad сценарии стабильны минимум 20 повторов подряд;
- лог каждой операции содержит context, plan, apply result.

### Этап 6. Codex adapter

Задачи:

- определить доступный backend: UI Automation, Chromium accessibility, fallback input;
- получить текст из активного input/editor поля Codex;
- заменить диапазон без порчи соседнего текста;
- не ломать обычные shortcuts Codex.

Тесты:

- UI tests в активном поле ввода Codex:
  - Pause last word;
  - ScrollLock current line;
  - mixed Russian/English phrase;
  - clipboard preserved;
  - no duplicate paste after 1-4 seconds.

Готово, когда:

- Stepler корректно работает в основном поле ввода Codex;
- если часть Codex UI недоступна через automation, это зафиксировано как ограничение adapter-а.

### Этап 7. Windsurf adapter

Задачи:

- определить доступ к editor и chat/input panel;
- проверить, не конфликтуют ли hotkeys с IDE shortcuts;
- реализовать безопасную замену в editor;
- реализовать безопасную замену в chat/input, если доступно.

Тесты:

- UI tests:
  - editor text correction;
  - chat/input correction;
  - selection correction;
  - clipboard preserved;
  - IDE shortcuts не ломаются.

Готово, когда:

- минимум editor path стабилен;
- chat/input path либо стабилен, либо явно отключен с логом.

### Этап 8. PowerShell/Terminal adapter

Статус: PowerShell-ветка реализована через PSReadLine adapter. Windows Terminal copy/paste эмуляция оставлена только как diagnostic/fallback, потому что в реальном Windows Terminal synthetic `Ctrl+C`/`Ctrl+Shift+C` может попадать в shell как текст и не менять clipboard.

Задачи:

- [x] не использовать `Ctrl+C` как универсальное копирование;
- [x] определить безопасный способ чтения текущего input: `PSConsoleReadLine.GetBufferState`;
- [x] определить безопасный способ замены: `PSConsoleReadLine.RevertLine` + `Insert` + `SetCursorPosition`;
- [x] при невозможности безопасной замены отказываться без порчи строки.

Тесты:

- UI tests:
  - [x] Pause на последнем слове через PSReadLine chord;
  - [x] ScrollLock-режим на хвостовой фразе через PSReadLine chord (`Ctrl+Pause` по умолчанию, потому что `ScrollLock` отсутствует в `System.ConsoleKey`);
  - [x] clipboard preserved by design: PSReadLine adapter не использует clipboard;
  - [x] команда не прерывается;
  - [x] old clipboard не вставляется.

Готово, когда:

- [x] PowerShell не получает случайные фрагменты clipboard;
- [x] нет подвисания после hotkey;
- [x] небезопасный контекст приводит к отказу, а не к порче строки.

### Этап 9. Word adapter

Задачи:

- использовать Word object model или UI Automation;
- работать с selection/range;
- не заменять весь документ;
- поддержать выделение нескольких слов.

Тесты:

- UI tests:
  - [x] Pause single word;
  - [x] Pause на слове слева от caret в середине строки;
  - [x] Pause selected multi-word text;
  - [x] ScrollLock mixed paragraph;
  - [x] clipboard preserved;
  - [x] no whole-document insertion.

Готово, когда:

- [x] Word adapter заменяет только целевой range;
- [x] ошибки Word COM/UI Automation не ломают приложение.

### Этап 10. UI и настройки

Задачи:

- [x] простой tray-only host `apps/Stepler.Tray`, который запускает общий hotkey pipeline без консольного окна;
- меню включения `Pause`, `Ctrl+Pause`, `Caps Lock`, автозапуска;
- log viewer не нужен для релиза; структурированные логи остаются в файле для отладки и тестов;
- diagnostics panel не входит в минимальный релиз; допустимы только высокоуровневые async-метрики, если они не усложняют UI;
- отображение времени последней операции допускается только в логах или в легком status UI без замедления hotkey pipeline;
- настройки app adapters.

Тесты:

- unit tests settings serialization;
- unit tests форматирования метрик для UI;
- UI smoke tests меню;
- UI smoke tests диагностической панели без проверки точных миллисекунд;
- manual test autostart;
- startup/shutdown tests.

Готово, когда:

- пользователь может включать/отключать функции без перезапуска;
- последняя операция видна в UI/логах;
- UI обновляет метрики асинхронно и не участвует в критическом пути применения замены.
- tray-приложение можно закрыть через контекстное меню, при выходе оно снимает keyboard hook и отпускает modifier-клавиши.

### Этап 11. Linux prototype

Задачи:

- собрать `stepler-core` на Linux;
- реализовать minimal global shortcut backend;
- реализовать clipboard backend;
- проверить X11;
- описать ограничения Wayland.

Тесты:

- `cargo test --workspace` на Linux;
- integration tests clipboard backend;
- manual UI smoke в выбранном Linux editor.

Готово, когда:

- core полностью переиспользуется;
- platform-linux имеет хотя бы один рабочий backend;
- ограничения Wayland документированы.

### Этап 12. Stabilization gate

Задачи:

- прогнать все regression tests;
- прогнать UI tests пакетами;
- провести ручные smoke tests;
- обновить known bugs;
- замерить задержки операций.

Тесты:

- `cargo test --workspace`;
- Windows UI suite;
- app-specific UI suite: Notepad, Codex, Windsurf, PowerShell, Word;
- repeated stress test 20-100 итераций для ключевых hotkeys;
- clipboard invariant tests.

Готово, когда:

- нет известных data-loss багов;
- clipboard invariant проходит;
- нет delayed second paste;
- все опасные unsupported contexts fail closed.

## 6. Основные инварианты стабильности

- Никакая операция не должна менять текст, если не построен валидный `ReplacementPlan`.
- Никакая операция не должна применяться, если активный control изменился после построения context.
- Замена выполняется не более одного раза на один `operation_id`.
- Clipboard после операции равен clipboard до операции, если пользователь сам не изменил его во время операции.
- Если adapter не может подтвердить `expected_before_text`, он должен отказаться от замены или перейти в явно помеченный risky mode.
- UI не содержит логики исправления текста.
- App-specific код не должен протекать в `stepler-core`.

## 7. Логирование

Каждая операция должна писать структурированный лог:

```json
{
  "operation_id": "...",
  "trigger": "Pause",
  "app": "Notepad",
  "provider": "Win32EditProvider",
  "replacer": "Win32EditReplacer",
  "state": "ReplacementApplied",
  "range": [10, 16],
  "expected_before_text": "k.,jdm",
  "replacement_text": "любовь",
  "clipboard_used": false,
  "duration_ms": 24,
  "timings_ms": {
    "context": 4,
    "plan": 1,
    "preflight": 2,
    "replace": 12,
    "verify": 3,
    "clipboard_restore": 0
  }
}
```

Требования:

- человекочитаемый лог для пользователя;
- JSONL или другой машинно-читаемый лог для тестов;
- возможность фильтровать по `operation_id`.
- обязательное измерение общего времени от получения hotkey до завершения операции;
- этапные timings должны писаться, когда их можно получить без заметной стоимости;
- UI может показывать последние timings только асинхронно, без блокировки hotkey pipeline.

## 8. Перенос стартовых материалов

Что переносить:

- карты символов раскладки;
- n-граммные таблицы и/или другие частотные данные;
- словари только как вспомогательные данные;
- тестовые примеры;
- список известных багов;
- часть логики определения ошибочных слов после ревью и переработки в сторону статистического скоринга.

Что не переносить напрямую:

- смешивание clipboard/read/replace в одном обработчике;
- фиксированные задержки как основной механизм синхронизации;
- глобальные mutable-флаги без state machine;
- app-specific ветки без единого контракта.

## 9. Приоритеты первой версии

MVP Stepler должен поддержать:

1. `stepler-core` с полным набором unit tests.
2. Transaction layer.
3. Windows hotkey listener.
4. Notepad adapter.
5. Codex adapter.
6. Windsurf adapter.
7. Clipboard invariant.
8. Structured logs.
9. Минимальный tray UI.

Word и PowerShell важны, но их лучше подключать после того, как единый pipeline доказал стабильность на Notepad/Codex/Windsurf.
