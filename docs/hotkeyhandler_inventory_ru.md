# Инвентаризация донорского проекта для Stepler

Дата: 2026-05-08.

Источник: локальный донорский workspace.

## Что есть в доноре

- WinForms-приложение на .NET 9 для Windows.
- Глобальные keyboard hooks/native hotkeys.
- Tray UI и настройки:
  - обработка `Pause`;
  - обработка `ScrollLock`;
  - обработка `Caps Lock`;
  - автозапуск.
- App-specific обработка:
  - Notepad;
  - Microsoft Word;
  - PowerShell/Windows Terminal;
  - generic fallback.
- Unit tests для layout conversion и mistype detection.
- UI tests для Notepad, Word и PowerShell.
- Runtime data:
  - `Data\NGrams\ru-3gram.tsv`;
  - `Data\NGrams\en-3gram.tsv`;
  - `Data\Lexicons\ru-words.txt`;
  - `Data\Lexicons\en-words.txt`.

## Модули для переиспользования

### `PauseTransliterationHandler.cs`

Полезно перенести в Stepler:

- таблицы раскладки RU <-> EN;
- базовую функцию `ConvertLayoutText`;
- идею token-wise conversion для выделенного текста;
- regression-cases из тестов;
- часть логики выбора хвостовой фразы после пересмотра контракта range/plan.

Не переносить напрямую:

- чтение текста через clipboard;
- восстановление clipboard внутри engine;
- app-specific ветки Word/PowerShell;
- fixed sleeps и SendKeys/SendInput как часть core logic.

### `LayoutMistypeDetector.cs`

Полезно перенести:

- формат `MistypeDetectionResult`;
- загрузку n-граммных TSV;
- scoring слов и фраз;
- fallback-лексиконы только как bootstrap/test data;
- идею `IsConvertedPhraseMoreLikely`.

Нужно переработать:

- словари не должны быть главным критерием `ScrollLock`;
- confidence должен быть явным числом в `ReplacementPlan`;
- короткие токены должны оцениваться по контексту фразы, а не агрессивно по одному слову.

### `Data`

Переносить как стартовые данные:

- `Data\NGrams\*.tsv` в runtime/test data Stepler;
- `Data\Lexicons\*.txt` только как вспомогательный ресурс.

### Unit tests

Переносить как unit/regression fixtures:

- `k.,jdm` -> `любовь`;
- `четыре,` -> `xtnsht?`;
- selected phrase `раз два три. xtnsht?` -> `hfp ldf nhb/ четыре,`;
- mixed tail `вальс поле long ghbdtn vbh` -> `вальс поле long привет мир`;
- SQL-like phrase `ORDER BY скуфеув ВУЫС` -> `ORDER BY created DESC`;
- short function phrase `d nf,kbwt exntyj` -> `в таблице учтено`;
- unknown mistyped English in Russian layout `ыеи ьфекшч` -> `stb matrix`.

### UI tests

Переносить как сценарии, не как реализацию:

- Notepad smoke для `Pause`;
- Notepad smoke для `ScrollLock`;
- clipboard preserved invariant;
- delayed second paste check через ожидание 2.5 секунды;
- PowerShell fail-closed behavior;
- Word range safety.

## Известные баги для regression backlog

- ScrollLock исторически портил текст в Notepad.
- Shared UI suite падал при привязке к PowerShell.
- Shared UI suite мог скрывать накопленные Notepad/Word failures.
- UI tests могли оставлять процессы и заблокированные runtime-файлы.
- Историческая порча многострочного текста через ScrollLock не закрыта надежным прогоном.
- Clipboard после ScrollLock в Notepad требовал отдельного восстановления.

## Вывод для Stepler

Первым переносим не UI и не обработчики hotkey, а чистую модель:

1. `TextContext`.
2. `CorrectionMode`.
3. `ReplacementPlan`.
4. Layout conversion.
5. N-gram backed language scoring.
6. Regression fixtures.

Clipboard, Win32, Word COM, PowerShell и synthetic input должны появиться только после того, как `stepler-core` научится строить проверяемый `ReplacementPlan` без side effects.
