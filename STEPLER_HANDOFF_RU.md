# Stepler handoff для нового Codex-чата

Этот файл сохраняет контекст для продолжения разработки Stepler после открытия workspace в локальной папке с проектами.

## Рабочие каталоги

- Старый проект: `<workspace>\HotkeyHandler\`
- Новый проект: `<workspace>\Stepler\`
- Рекомендуемый workspace root для Codex: `<workspace>\`

## Документы, которые нужно прочитать в новом чате

1. `<workspace>\HotkeyHandler\TECHNICAL_SPEC_RU.md`
2. `<workspace>\HotkeyHandler\STEPLER_DEVELOPMENT_PLAN_RU.md`
3. `<workspace>\HotkeyHandler\README.md`
4. `<workspace>\HotkeyHandler\known_bugs.txt`

## Цель Stepler

Stepler - новая реализация HotkeyHandler с нуля, но с возможностью переиспользования текущего проекта.

Основная цель - стабильная работа каждого модуля:

- исправление текста по `Pause`;
- исправление текста по `ScrollLock`;
- сохранение пользовательского буфера обмена;
- отсутствие двойной вставки через 1-4 секунды;
- отсутствие гонок между чтением текста, построением плана и заменой;
- поддержка Windows сначала, но с архитектурой для будущего Linux-порта.

## Технологический выбор

- Core: Rust.
- UI: тонкий desktop UI, например Tauri или Avalonia-like UI.
- UI не должен содержать бизнес-логику исправления текста.
- Архитектура должна быть пригодна для портирования на Linux.

## Архитектура из 4 уровней

1. `TextContextProvider`
   - получает активное окно, текст, caret, selection, capabilities;
   - не меняет текст и clipboard.

2. `CorrectionEngine`
   - чистая логика;
   - вход: `TextContext` + режим `Pause`/`ScrollLock`;
   - выход: `ReplacementPlan`;
   - не знает о Win32, clipboard, UI Automation, Word, Terminal.

3. `TextReplacer`
   - применяет `ReplacementPlan`;
   - проверяет `expected_before_text`;
   - заменяет текст не более одного раза на `operation_id`.

4. `Safety/Transaction Layer`
   - управляет lifecycle операции;
   - блокирует повторные операции;
   - измеряет duration/timings;
   - восстанавливает clipboard;
   - логирует полный путь операции.

## Ключевые требования к ScrollLock

`ScrollLock` не должен опираться главным образом на словари.

Основной механизм распознавания текста в неверной раскладке:

- n-граммы;
- частотный анализ;
- языковой скоринг;
- confidence threshold.

Словари допустимы только как вспомогательный сигнал, источник fixtures или дополнительная проверка.

Алгоритм должен работать с:

- неизвестными словами;
- именами;
- техническими терминами;
- путями;
- командами;
- фрагментами кода;
- короткими словами и токенами вроде `git`, `на`, `in`.

Короткие токены нельзя игнорировать только из-за длины, но нельзя и агрессивно конвертировать без контекста. Решение по ним должно приниматься по контексту соседних слов, score всей фразы и confidence threshold.

## Приложения, которые нужно поддержать

Минимально:

- Notepad;
- Codex;
- Windsurf;
- PowerShell/Windows Terminal;
- Microsoft Word.

MVP приоритет:

1. `stepler-core`;
2. transaction layer;
3. Windows hotkey listener;
4. Notepad adapter;
5. Codex adapter;
6. Windsurf adapter;
7. clipboard invariant;
8. structured logs;
9. минимальный tray UI.

Word и PowerShell важны, но лучше подключать после стабилизации общего pipeline на Notepad/Codex/Windsurf.

## Важные regression-сценарии

Notepad:

- `k.,jdm` + `Pause` -> `любовь`.
- `k.,jdm` + `ScrollLock` -> исправленное слово без обрезания.
- `вальс поле long ghbdtn vbh` + `ScrollLock` -> корректный префикс сохраняется, ошибочный хвост исправляется.
- выделение нескольких слов должно конвертироваться полностью.
- caret в середине слова не должен приводить к обрезанному результату.
- clipboard после операции должен совпадать с исходным.
- не должно быть delayed second paste.

Codex/Windsurf:

- исправление в active input/editor;
- не ломать обычные shortcuts IDE;
- не вставлять старый clipboard;
- не делать двойную вставку.

PowerShell:

- не использовать `Ctrl+C` как обычное копирование;
- не прерывать команду;
- безопасно отказываться, если контекст не подтвержден.

Word:

- не заменять весь документ;
- работать с выделением и точным range.

## Метрики и UI-диагностика

Нужно измерять время от получения `Pause`/`ScrollLock` до финального завершения операции.

Логировать:

- общий `duration_ms`;
- `context`;
- `plan`;
- `preflight`;
- `replace`;
- `verify`;
- `clipboard_restore`.

Если UI может показывать эти метрики без замедления hotkey pipeline, нужно вывести последнюю длительность операции в диагностическую область.

UI-обновление метрик должно быть асинхронным и не на критическом пути.

## Что переиспользовать из HotkeyHandler

- карты раскладки из `PauseTransliterationHandler`;
- логику `ConvertLayoutText` после ревью;
- test cases из `HotkeyHandler.Tests`;
- `Data\NGrams` как стартовый источник n-грамм;
- `Data\Lexicons` только как вспомогательный ресурс;
- known bugs как regression backlog.

## Что не переносить напрямую

- смешивание read/replace/clipboard в одном обработчике;
- фиксированные задержки как основной механизм синхронизации;
- глобальные mutable-флаги без state machine;
- app-specific ветки без единого контракта;
- вставку через clipboard там, где есть более надежный способ.

## Команда для нового чата

После открытия Codex workspace в папке с проектами можно написать:

```text
Продолжаем Stepler. Прочитай HotkeyHandler\STEPLER_HANDOFF_RU.md, TECHNICAL_SPEC_RU.md и STEPLER_DEVELOPMENT_PLAN_RU.md. Новый проект находится в Stepler\. Начни с этапа 1: Rust workspace и stepler-core, с unit tests.
```

