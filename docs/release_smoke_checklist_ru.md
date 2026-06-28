# Release smoke checklist

Чек-лист для первого Windows-релиза Stepler 1.0. Цель - быстро поймать регрессии, которые реально портят пользовательский сценарий: неправильная замена, сломанный clipboard, зависшие modifier-клавиши, неработающий tray или неверный method resolver.

## 0. Карта проверок

Adapter contracts и продуктовый smoke проверяют разные риски, поэтому их не
нужно смешивать:

- adapter policy/classifier/resolver: `cargo test -p stepler-platform`, contract
  fixtures `probe_contracts.tsv` и `resolver_contracts.tsv`, policy invariants;
- replacement behavior: `cargo test -p stepler-core`;
- Windows runtime boundaries и adapter-specific protection:
  `cargo test -p stepler-platform-windows`;
- hotkey/layout/tray lifecycle: ручной smoke через tray из `dist` или debug
  build, плюс проверка логов;
- Qwen input/workspace: ручной smoke окна ввода, workspace attach/focus и
  отправки текста;
- installer/release package: ручной install smoke из `SetupOutput`.

Если меняется adapter policy или classifier, сначала должны пройти contract
tests. Если меняется tray, installer, Qwen input/workspace или запуск процесса,
нужен ручной smoke соответствующего продукта. Не добавлять большую матрицу
ручных проверок без повторяющейся регрессии.

## 1. Подготовка

Закрыть старые экземпляры Stepler:

```powershell
Get-Process -Name Stepler,Stepler.Tray,stepler-cli -ErrorAction SilentlyContinue | Stop-Process -Force
```

Собрать debug-версию для smoke:

```powershell
cargo build -p stepler-cli
dotnet build .\apps\Stepler.Tray\Stepler.Tray.csproj -nologo -c Debug
```

Запустить tray из debug-сборки:

```powershell
Start-Process .\apps\Stepler.Tray\bin\Debug\net9.0-windows\Stepler.exe
```

Для release/runtime изменений дополнительно проверить запуск из релизной папки
вне sandbox:

```powershell
Start-Process .\dist\Stepler\Stepler.exe
```

В tray menu должны быть доступны:

- статус;
- включить/выключить обработчик;
- перезапуск обработчика;
- настройки `Pause`, `Ctrl+Pause`, `Left/Right Ctrl`, `Menu/Caps`, `Risky fallback adapters`;
- окно управления;
- выход.

## 2. Автоматические проверки

Обязательные команды:

```powershell
cargo fmt --all -- --check
cargo test --workspace
dotnet build .\apps\Stepler.Tray\Stepler.Tray.csproj -nologo -c Debug
```

Критерий: все команды проходят без ошибок.

Минимальный набор для adapter-only правок:

```powershell
cargo test -p stepler-core -p stepler-platform -p stepler-platform-windows
```

Для tray/installer/Qwen workspace этот набор не заменяет ручной smoke: он только
подтверждает, что adapter contracts и runtime boundaries не сломаны.

Если `dotnet build` ругается на занятый `Stepler.exe`/`Stepler.Tray.exe`, закрыть tray через меню или выполнить команду из раздела подготовки.

## 3. Общие инварианты

Перед каждым ручным приложением проверить:

- clipboard содержит заранее известный текст, например `COPYME`;
- после `Pause`/`Ctrl+Pause` `Ctrl+V` вставляет исходный `COPYME`, если сценарий не должен менять clipboard;
- после выхода из Stepler клавиатура не остается в состоянии зажатых `Ctrl`, `Alt`, `Win` или `Shift`;
- `Win+Space` продолжает переключать раскладку Windows;
- `Left Ctrl` отдельно переключает на RU, `Right Ctrl` отдельно переключает на EN, если включена настройка `Left/Right Ctrl`.

Блокер релиза:

- потеря clipboard в Notepad/Word/PowerShell;
- зависание Word или terminal;
- случайные символы `c`/`с`, `1`, `Сс` после hotkey;
- зажатый modifier после выхода;
- двойная вставка через 1-4 секунды после операции.

## 4. Notepad

Открыть Notepad и вставить тестовый текст:

```text
k.,jdm
one two
вальс поле long ghbdtn vbh
house dfkmc поле long привет мир
```

Проверки:

- `k.,jdm` + `Pause` -> `любовь`;
- `k.,jdm` + `Ctrl+Pause` -> `любовь`;
- `вальс поле long ghbdtn vbh` + `Ctrl+Pause` -> `вальс поле long привет мир`;
- `house dfkmc поле long привет мир` + `Ctrl+Pause` -> `house вальс поле long привет мир`;
- корректная фраза + `Ctrl+Pause` не меняется и не получает лишний символ в конце;
- caret после операции остается в ожидаемом месте около замененного фрагмента;
- clipboard после каждой операции сохраняется.

## 5. PowerShell через PSReadLine

Stepler tray должен автоматически установить загрузчик adapter-а в user profile PowerShell:

```powershell
Get-SteplerPsReadLineStatus
```

Если окно PowerShell было открыто до запуска/обновления Stepler, перезапустить окно. Ручной fallback для диагностики:

```powershell
Import-Module PSReadLine
$adapter = Join-Path (Get-Location) "dist\Stepler\scripts\Stepler.PSReadLine.ps1"
. $adapter
Get-SteplerPsReadLineStatus
```

Проверки:

- `k.,jdm` + `Pause` -> `любовь`;
- `пше` + `Ctrl+Pause` -> `git`;
- `ghbdtn vbh` + `Ctrl+Pause` -> `привет мир`;
- команда не выполняется сама по себе;
- в prompt не появляются `Сс`, `1`, `COPYME` или фрагменты clipboard;
- clipboard сохраняется;
- после закрытия PowerShell системная клавиатура работает нормально.

Важно: пользовательский shortcut для умного режима строки в PowerShell - `Ctrl+Pause`.

## 6. Microsoft Word

Перед автоматическими Word smoke-тестами закрыть все окна Word.

Ручные проверки:

- ввести `k.,jdm`, поставить caret в конец, нажать `Pause` -> `любовь`;
- выделить два слова в неверной раскладке, нажать `Pause` -> выделенный фрагмент конвертируется целиком;
- ввести `вальс поле long ghbdtn vbh`, поставить caret в конец, нажать `Ctrl+Pause` -> `вальс поле long привет мир`;
- `Right Ctrl` переключает на EN, `Left Ctrl` переключает на RU;
- clipboard сохраняется;
- Word не зависает и не остается с поврежденным выделением.

Опциональный UI smoke:

```powershell
cargo test -p stepler-cli --test word_smoke -- word_com_direct_cli_replaces_pause_and_scrolllock_text --ignored --nocapture --test-threads=1
```

## 7. UIAutomation fixture

Запустить тестовое поле ввода:

```powershell
cargo run -p stepler-cli -- uia-fixture
```

В открывшемся окне проверить:

- `k.,jdm` + `Pause` -> `любовь`;
- `house dfkmc поле long привет мир` + `Ctrl+Pause` -> `house вальс поле long привет мир`;
- корректная строка + `Ctrl+Pause` не меняется;
- caret не прыгает в начало строки;
- не появляется лишний `c`/`с`.

## 8. Browser/Codex/risky fallback

По умолчанию risky fallback должен быть выключен.

Проверка fail-closed:

```powershell
cargo run -p stepler-cli -- diagnose-focus --delay 3 --methods
```

Для неизвестного `Chrome_WidgetWin_*` resolver может показать `ClipboardSelection`/`SendInput` как risky probes, но без явного разрешения normal hotkey не должен портить текст или clipboard.

Если включить `Risky fallback adapters` в tray, проверять только вручную и считать режим экспериментальным. Для релиза 1.0 это не основной поддерживаемый путь.

## 9. Tray/Qwen lifecycle smoke

Проверки:

- выключить `Pause` в tray menu, убедиться, что `Pause` больше не обрабатывается Stepler и не блокируется невидимо;
- включить `Pause` обратно, убедиться, что он снова работает;
- выключить `Left/Right Ctrl`, проверить что одиночные Ctrl больше не переключают раскладку Stepler;
- перезапустить tray, убедиться, что настройки сохранились;
- файл настроек существует в `%APPDATA%\Stepler\settings.json`;
- открыть `Qwen input...`, проверить ввод текста, `Pause`/`Ctrl+Pause`,
  отображение результата P/CP и отправку в Qwen;
- открыть `Qwen workspace`, проверить, что окно терминала прикрепилось,
  фокус остается в Stepler Qwen input, а перезапуск Stepler не закрывает
  существующую Qwen-сессию.

## 10. Логи

После ручного smoke проверить, что логи не содержат частых ошибок:

```powershell
Get-Content "$env:LOCALAPPDATA\Stepler\logs\stepler_hotkey_log.jsonl" -Tail 30 -ErrorAction SilentlyContinue
Get-Content "$env:LOCALAPPDATA\Stepler\logs\Stepler.Tray.log" -Tail 30 -ErrorAction SilentlyContinue
```

Разовые `UnsupportedControl` допустимы для неподдержанных приложений. Частые `ClipboardUnavailable`, `ForegroundUnavailable`, panic, COM exception или restore failure - повод остановить релиз.

## 11. Известные ограничения 1.0

- Полноценная Linux desktop-версия отложена; поддержан только SSH/Bash remote helper.
- Classic `cmd.exe`/`conhost.exe` и `cmd.exe` внутри Windows Terminal не считаются поддержанными сценариями.
- Browser/Codex/Windsurf через generic fallback не являются основным поддерживаемым путем.
- PowerShell должен использовать PSReadLine adapter; terminal clipboard shortcut остается diagnostic/fallback.
- Word smoke зависит от установленного Microsoft Word и отсутствия уже открытых `WINWORD` процессов для автоматического теста.

## 12. Решение по релизу

Релизная папка собирается командой:

```powershell
.\scripts\build-release.ps1
```

Результат: `dist\Stepler\Stepler.exe` и side-by-side `dist\Stepler\stepler-cli.exe`.

Перед пересборкой закрыть Stepler, если он запущен из `dist\Stepler`; иначе Windows может заблокировать очистку релизной папки.

Инсталлятор собирается командой:

```powershell
.\scripts\build-installer.ps1
```

Результат: `SetupOutput\SteplerSetup-<version>.exe`. Инсталлятор устанавливает Stepler в `Program Files\Stepler`, создает ярлыки, регистрирует `App Paths\Stepler.exe`, закрывает старые процессы Stepler перед обновлением и запускает приложение после установки, если пользователь оставил post-install launch включенным.

Минимальный install smoke:

- запустить `SetupOutput\SteplerSetup-<version>.exe` от администратора;
- оставить post-install launch включенным;
- убедиться, что появилась иконка tray;
- открыть tray menu и проверить окно управления;
- включить/выключить один пункт настройки и перезапустить Stepler;
- проверить, что `%APPDATA%\Stepler\settings.json` сохранился;
- выполнить Notepad smoke из раздела 4;
- удалить Stepler через Windows Settings -> Apps -> Installed apps.

Релиз можно собирать, если:

- автоматические проверки проходят;
- Notepad, PowerShell PSReadLine, Word и UIA fixture проходят ручной smoke;
- clipboard сохраняется в основных сценариях;
- tray закрывается через меню и снимает обработчик;
- после выхода не остаются зажатые modifier-клавиши;
- все найденные ограничения записаны в release notes.
