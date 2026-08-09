# Stepler handoff: Outlook 2016/Zimbra

Дата handoff: 2026-08-09  
Рабочий каталог: `F:\distr\system\Stepler`

## Цель продолжения

Безопасно расследовать периодические зависания Microsoft Outlook 2016 с Zimbra Connector после нажатия P/CP. Не менять код вслепую: сначала собрать свежие доказательства и сопоставить зависание Outlook с конкретным путем Stepler.

## Текущее состояние репозитория

- Рабочая копия чистая.
- Ветка `develop` опережает `origin/develop` на один локальный коммит.
- `HEAD`: `4b6e88f Avoid Outlook layout sync after correction`.
- Локальный коммит пока не отправлен на remote.
- Последний инсталлятор: `F:\distr\system\Stepler\SetupOutput\SteplerSetup-1.0.20260806.t1347.exe`.
- В `dist\Stepler` файл `BUILD_INFO` не найден. Версию runtime нельзя считать подтвержденной только по наличию файлов в `dist`.

## Последние изменения

В `crates/stepler-cli/src/main.rs` функция `should_skip_layout_after_replacement` теперь пропускает автоматическую смену раскладки после замены для `rctrl_renwnd32/_WwG`, а также для ранее защищенного `RICHEDIT60W`.

WordCom-текстовая замена сохранена. В Outlook Word editor после P/CP автоматическая смена раскладки пока намеренно не выполняется как защитная мера. Тест `outlook_word_com_skips_layout_after_replacement` добавлен. История и ограничения описаны в `outlookhaging.md`.

Ранее, в коммите `5564597`, отдельный watchdog `Stepler.exe` был убран, чтобы в штатном состоянии оставались ровно один tray-процесс `Stepler.exe` и один `stepler-cli.exe`. Автоматический перезапуск `stepler-cli` при завершении сохранен.

## Последние проверки

- `cargo test -p stepler-cli`: 10 тестов прошли.
- `cargo fmt --all` выполнен.
- Release-сборка прошла.
- Новая защита в реальном Outlook пока не считается доказанно проверенной.

## Актуальный runtime при создании handoff

Проверка процессов показала:

- `Stepler.exe`, PID `18412`, command line: `F:\soft\sys\Stepler\Stepler.exe`;
- `stepler-cli.exe`, PID `24572`, command line: `F:\soft\sys\Stepler\stepler-cli.exe run-hotkeys`.

Это runtime из `F:\soft\sys\Stepler`, а не подтвержденная сборка из `F:\distr\system\Stepler\dist\Stepler`. Перед тестированием последнего кода необходимо проверить путь каждого процесса.

## Что проверять при новом зависании Outlook

1. Не убивать Outlook без необходимости. Сначала получить его PID, состояние `Responding`, command line и время начала зависания.
2. Снять свежий хвост `%LOCALAPPDATA%\Stepler\logs\stepler_hotkey_log.jsonl`.
3. Снять свежий хвост `%LOCALAPPDATA%\Stepler\logs\hotkey_signal.log`.
4. Проверить Windows Event Viewer/WER для `OUTLOOK.EXE` и событие `Application Hang` за тот же интервал.
5. Определить, какой путь выполнялся: `word_com`, `win32_edit_messages`, layout-sync либо операция не дошла до `Completed`.
6. Сопоставить время зависания Outlook с событиями Stepler. Сам факт, что зависание произошло после P/CP, недостаточен для вывода о причине.

## Возможное следующее решение

Если зависание повторится после отключения layout-sync, рассмотреть временную fail-closed policy для Outlook Word editor: P/CP возвращает понятный `fail`, но не выполняет опасную операцию и не подвешивает Outlook. Альтернатива: отдельный безопасный Outlook adapter с минимальным COM-взаимодействием.

Нельзя возвращать layout-sync в Outlook без нового доказательства безопасности. Нужно сохранить поддержку Outlook search, Word/Outlook editor и остальных адаптеров.

## Правила изменений

- Изменения должны быть узкими и покрыты unit/contract tests.
- Сначала сравнить текущий код, логи и runtime; не переписывать архитектуру вслепую.
- После любого изменения выполнить:

```text
cargo fmt --all
cargo test -p stepler-cli
release build
```

- После пересборки запускать Stepler вне песочницы из `F:\distr\system\Stepler\dist\Stepler\Stepler.exe`.
- Через несколько секунд проверить наличие ровно одного `Stepler.exe` и одного `stepler-cli.exe`, а также их command line/path.
- Инсталлятор пересобирать только по запросу пользователя или если изменился release payload.
- Не выполнять `git push` без отдельной просьбы пользователя.

## Suggested skills

- `F:\soft\AI_Skills\mattpocock-skills\skills\engineering\diagnosing-bugs\SKILL.md` для следующего Outlook hang.
- `C:\Users\alexey.andreev\.codex\skills\karpathy-guidelines\SKILL.md` для хирургических изменений и явных критериев проверки.
- `F:\soft\AI_Skills\mattpocock-skills\skills\engineering\codebase-design\SKILL.md`, если потребуется отдельный Outlook adapter.

## Ограничение запуска handoff

Команда `claude --bg`, предусмотренная skill `claude-handoff`, в текущей среде недоступна. Поэтому этот файл является переносимым handoff-артефактом для следующего агента Codex.
