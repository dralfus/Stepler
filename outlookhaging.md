# Outlook 2016 / Zimbra Connector hangs during Stepler P/CP

Рабочая заметка по расследованию зависаний Outlook UI после нажатий Stepler `P` / `CP`.

Дата фиксации заметки: 2026-06-19. Последнее обновление: 2026-08-24.

## Актуальная спецификация: Outlook-only переключение раскладки

### Problem Statement

Outlook 2016 с Zimbra Connector периодически полностью зависает после `P`, `CP`
или одиночного `Ctrl`. Последний случай `2026-08-24` локализован точнее прежних:
две текстовые операции `Pause` через `word_com` завершились успешно, а Outlook
завис позднее при обработке отдельной команды смены раскладки.

Полный дамп зависшего процесса:

```text
F:\distr\system\outlook_diag\OUTLOOK_hang_19712_20260824_105915.dmp
```

Связанные отчеты CDB:

```text
F:\distr\system\outlook_diag\OUTLOOK_hang_19712_20260824_ui_thread.txt
F:\distr\system\outlook_diag\OUTLOOK_hang_19712_20260824_all_threads.txt
```

В `10:59:15` Stepler отправил в focused Outlook Word editor `_WwG` с HWND
`0x20D78` асинхронное `WM_INPUTLANGCHANGEREQUEST` (`0x0050`) с HKL
`0x04190419`. Stepler записал `result=ok`, потому что `PostMessageW` принял
сообщение. Дамп показывает, что UI thread Outlook `4168` затем завис в
`win32u!NtUserMessageCall` при обработке именно этого сообщения:

```text
rcx/r10 = 0x20D78
rdx     = 0x50
r9      = 0x04190419
```

Следовательно, успешный `PostMessageW` означает только доставку в очередь и не
является подтверждением безопасной смены раскладки. Текущий Outlook-specific
`post_outlook_layout_change` не устраняет зависание, а лишь отделяет его по
времени от операции Stepler.

### Solution

Для Outlook вводится отдельный транспорт смены раскладки. Он обязан:

- распознаваться только для процесса `OUTLOOK`/класса `rctrl_renwnd32`;
- работать только при editable focus (`_WwG`, `Edit`, `RichEdit*`);
- не вызывать `SendMessageTimeoutW`, `PostMessageW` или иной прямой transport
  `WM_INPUTLANGCHANGEREQUEST` в Outlook HWND;
- не иметь fallback к общему `switch_window_layout` для Outlook;
- перед действием и перед подтверждением проверять неизменность foreground HWND,
  focused HWND и process identity;
- считать успехом только наблюдаемую целевую раскладку focused thread, а не факт
  отправки input/message;
- иметь жесткий короткий deadline и завершаться fail-closed;
- писать отдельные события `outlook_layout_*` с transport, target layout,
  elapsed time и результатом проверки.

Общий transport смены раскладки для остальных приложений не меняется.

Пользовательский механизм остается единым во всем Stepler: левый `Ctrl`
переключает на русский, правый `Ctrl` - на английский. Для Outlook меняется
только внутренний transport: Stepler передает переключение системному механизму
выбора языка через `SendInput`, не посылая layout message непосредственно в
Outlook HWND.

Для `P`/`CP` layout dispatch во всех поддерживаемых приложениях начинается
после построения correction plan и preflight, непосредственно перед apply.
Проверка целевой HKL идет параллельно с replacement. Обе ветки используют один
snapshot process/foreground/focus; изменение snapshot прекращает layout-ветку и
повторная проверка foreground перед apply запрещает замену в другом контроле.
Операция считается полностью завершенной только после окончания текстовой
ветки и проверки layout-ветки.

Глобальным является только порядок `dispatch -> apply || verify`. Transport
остается surface-specific: Outlook использует системный hotkey, остальные
приложения сохраняют свой window-message transport.

### User Stories

- Как пользователь Outlook, я хочу после `P`/`CP` сразу продолжать ввод в
  раскладке исправленного текста.
- Как пользователь Outlook с Zimbra Connector, я не хочу, чтобы смена раскладки
  Stepler блокировала UI thread Outlook.
- Как пользователь остальных приложений, я хочу сохранить прежний способ и
  скорость переключения раскладки без Outlook-specific изменений.
- Как диагност, я хочу отличать принятую команду от подтвержденной смены
  раскладки и видеть точную причину fail-closed результата.

### Implementation Decisions

- Outlook определяется до выбора transport, а не внутри общего fallback.
- Прямой `WM_INPUTLANGCHANGEREQUEST` для Outlook запрещен контрактом.
- Текстовый `WordCom`/`Win32EditMessages` transport не смешивается с transport
  смены раскладки.
- Outlook layout transport не должен жить в COM worker: зависание связано с
  UI message loop, а не с временем жизни COM worker.
- Zimbra Connector не обновляется и не удаляется в рамках решения.
- Пользователь не получает новую hotkey: остаются только left/right `Ctrl`.
- Outlook-only transport использует системное переключение языка через
  `SendInput`; другие приложения сохраняют прежний message-based transport.
- Если target layout не подтвержден до deadline, текстовая замена сохраняется,
  повторный опасный fallback не запускается, а overlay показывает
  `текст исправлен, язык не переключён`.
- Layout dispatch и apply во всех поддерживаемых приложениях запускаются как
  две координированные ветки: dispatch происходит перед apply, verification
  продолжается параллельно.

### Testing Decisions

- Unit: Outlook classification выбирает только Outlook transport.
- Unit: Outlook transport никогда не вызывает общий direct-message fallback.
- Unit: non-editable Outlook focus завершается fail-closed без input/message.
- Unit: foreground/focus change до подтверждения отменяет операцию.
- Contract: Word, Notepad, браузеры, PowerShell и другие non-Outlook surfaces
  сохраняют прежний layout transport.
- Manual smoke: Outlook search (`RICHEDIT60W`) и compose (`_WwG`), отдельно
  `P`, `CP`, одиночные left/right `Ctrl`, минимум 30 повторов для каждого пути.
- Acceptance: ни одного `OUTLOOK.EXE (Не отвечает)`; target layout подтвержден
  не позже завершения P/CP; соседний текст, caret и focus сохранены.
- Hang diagnostics: при повторении сначала снимается full dump, затем Outlook
  можно перезапустить; `PostMessageW result=ok` не принимается за verification.

### Out of Scope

- Обновление или замена Zimbra Connector.
- Изменение layout transport для приложений, не относящихся к Outlook.
- Полный перенос `WordCom` в worker в рамках этого узкого исправления.
- Постоянный демон, автоматически создающий большие full dump без явного
  диагностического режима и лимитов хранения.

### Further Notes

- Загруженные Zimbra DLL присутствуют в дампе, но просмотренные Zimbra threads
  находились в ожидании. Дамп не доказывает, что блокировку создал Zimbra.
- Непосредственный триггер последнего зависания доказан: обработка Outlook UI
  thread сообщения смены раскладки, отправленного Stepler.
- Исторические гипотезы и предыдущие эксперименты сохранены ниже; при конфликте
  эта спецификация имеет приоритет.

## Краткий вывод

Зависания Outlook 2016 наблюдались именно после срабатывания `P` / `CP` в Stepler/HotkeyHandler, а не при обычном ручном изменении текста в Outlook search. Полного доказательства причины пока нет, но главный подозреваемый путь - взаимодействие Stepler с Outlook/Zimbra в момент замены текста.

После первых safety-изменений зависания некоторое время не воспроизводились, но `2026-06-19`
был пойман новый случай: Outlook снова перешел в состояние `(Не отвечает)` после `P` / `CP`.
В свежем логе операция шла не через `WordCom`, а через `Win32EditMessages` по Outlook
`RICHEDIT60W`. Поэтому текущая рабочая гипотеза уточнена: для Outlook/Zimbra опасен не только
Word COM, но и прямой Win32/RichEdit путь замены текста.

## Термины

- `P` - hotkey Stepler для конвертации слова около курсора.
- `CP` - hotkey Stepler для конвертации более широкого фрагмента/фразы.
- `stepler-cli.exe` - процесс, который реально держит hotkeys и выполняет операции.
- `Stepler.exe` - tray-приложение, показывает иконку, настройки, запускает `stepler-cli`.
- `hotkey-log` - внутренний лог Stepler, не системный Windows log.

## Окружение

- Outlook: Microsoft Outlook 2016.
- Версия Outlook из WER/diagnostics: `16.0.5507.1000`.
- Zimbra Connector for Microsoft Outlook: `8.8.15.1837`.
- Установленная Zimbra дата: `20260602`.
- Основной Zimbra data file:
  - `C:\Users\alexey.andreev\AppData\Local\Microsoft\Outlook\NEW.zdb`
  - Размер на момент диагностики: около 5.5 GB.

## Наблюдаемые симптомы

- Outlook UI зависал после нажатия `P` / `CP`.
- Простое ручное изменение текста в Outlook search зависание не вызывало.
- При зависании окно могло выглядеть как UI hang, но процесс `OUTLOOK.EXE` иногда оставался `Responding=True`.
- В свежем наблюдении `2026-06-19` окно Outlook явно показывало `(Не отвечает)` в заголовке.
- Попытка обратиться к Outlook через COM могла падать:

```powershell
(New-Object -ComObject Outlook.Application).ActiveExplorer().Activate()
```

Ошибка:

```text
80080005 CO_E_SERVER_EXEC_FAILURE
```

Интерпретация: текущий экземпляр Outlook настолько занят/завис, что Windows не может получить или поднять COM-сервер Outlook. Это не ошибка Stepler-команды, а диагностический признак более глубокого зависания Outlook/COM/message loop.

## Собранные дампы и диагностика

### Первый дамп

```text
F:\distr\system\outlook_diag\OUTLOOK_hang_17688_20260616_104056.dmp
```

Размер: около 1.1 GB.

### Второй дамп

```text
F:\distr\system\outlook_diag\OUTLOOK_hang_26472_20260616_115811.dmp
```

Размер: `1,013,682,639` bytes.

На момент второго зависания:

- `OUTLOOK.EXE` PID: `26472`.
- `Responding=True`.
- `MainWindowTitle` пустой.
- Start time: `16.06.2026 11:21:27`.
- Threads в основном были в состояниях `Wait UserRequest` / `EventPairLow`.

## Zimbra modules в процессе Outlook

В Outlook были загружены Zimbra модули версии `8.8.15.1837`:

- `LSLIB32.dll`
- `LSMIME32.dll`
- `LSMSCFG32.DLL`
- `LSMSSP32.DLL`
- `LSMSUTIL32.dll`
- `LSMSXP32.DLL`
- `SharingAddin.dll`
- `ShutdownAddin.dll`

Старые WER reports показывали повторяющиеся `APPCRASH` в `LSLIB32.dll`.

## Windows Event Log / WER

На `2026-06-16 11:21:23` был зафиксирован предыдущий Outlook hang:

- Event type: `Application Hang`
- WER signature: `AppHangB1`
- Process: `OUTLOOK.EXE`
- Outlook version: `16.0.5507.1000`

Текущий Outlook после этого был запущен в `11:21:27`.

Старые WER archives содержали майские `AppCrash_OUTLOOK...` отчеты с `LSLIB32.dll`.

## Outlook add-ins

В реестре были видны add-ins:

- `gpg4o`
- `Microsoft.VbaAddinForOutlook.1`
- OneNote add-in
- Outlook Social Connector
- `SharingAddin.Addin`
- `ShutdownAddin.Addin`
- `UmOutlookAddin.FormRegionAddin`
- WOW6432Node:
  - `gpg4o`
  - `PDFMOutlook.PDFMOutlook`

## Что было изменено в Stepler для снижения риска

### Outlook policy

Идея policy: ограничивать методы Stepler внутри Outlook.

Важно: policy-настройка может отключать или разрешать работу `P` / `CP` в отдельных зонах Outlook. Пока полное отключение `P` / `CP` в Outlook не делалось, потому что пользователь хочет сохранить конвертацию.

Текущая политика для Outlook:

- Runtime probe policy разрешает:
  - `Win32EditMessages` для Outlook search / простых edit controls.
  - `WordCom` для Outlook compose/editor, где это действительно Word editor.
  - `Win32EditMessages`, затем `WordCom` для Outlook shell/explorer fallback boundary.
- Runtime probe policy не дает Outlook generic fallback methods:
  - UIA (`uia_editable_text`, `uia_document_text`, `uia_text`)
  - terminal clipboard
  - clipboard selection
  - SendInput fallback
- Resolver policy для `OutlookWordEditor` тоже fail-closed: если в resolver
  искусственно передать generic UIA/clipboard/send_input probes без `WordCom`,
  они должны быть rejected by policy.

Контракты этой границы зафиксированы здесь:

- `crates/stepler-platform/src/surface.rs`:
  - `SurfacePolicy` для `OutlookSearch`, `OutlookWordEditor`, `OutlookShell`
  - `ProbePolicy` для этих же surfaces
- `crates/stepler-platform/tests/fixtures/probe_contracts.tsv`
- `crates/stepler-platform/tests/fixtures/resolver_contracts.tsv`
- `crates/stepler-platform-windows/src/tests.rs`:
  - `outlook_runtime_stacks_do_not_include_generic_fallbacks`

### Word COM ограничен только Outlook Word editor

Файл:

```text
F:\distr\system\Stepler\crates\stepler-platform-windows\src\word_com.rs
```

Изменение: `WordComMethod.probe` теперь использует более узкую проверку:

```text
is_outlook_word_editor_target(target)
```

вместо широкой проверки Outlook window.

Критерий Outlook Word editor:

- process `OUTLOOK`
- focused class `_WwG`

Это нужно, чтобы не лезть Word COM в Outlook Explorer/search windows.

### Win32EditMessages запрещен для Outlook RichEdit

Файл:

```text
F:\distr\system\Stepler\crates\stepler-platform-windows\src\win32_edit.rs
```

После нового зависания `2026-06-19` сначала был добавлен fail-closed guard:

- если foreground принадлежит Outlook (`OUTLOOK` / `rctrl_renwnd32`);
- и focused class начинается с `RichEdit`;
- то `Win32EditMessagesMethod.probe` не возвращает метод.

Причина: свежий hang был связан с Outlook `RICHEDIT60W`, где Stepler выбрал:

```text
replacer: win32_edit_messages
```

Для Outlook RichEdit это признано небезопасным. Лучше получить `P fail` / `CP fail`, чем
подвесить Outlook/Zimbra. Обычные Outlook `Edit` controls, например простые search-поля,
пока не заблокированы.

Позже проверка показала, что Outlook search тоже фокусируется как `rctrl_renwnd32/RICHEDIT60W`.
Полный запрет `Win32EditMessages` поэтому ломал P/CP в поиске писем. Guard был заменен на более
узкий экспериментальный вариант:

- `Win32EditMessages` снова разрешен для Outlook `RICHEDIT60W`;
- такие контексты помечаются как `rctrl_renwnd32/RICHEDIT60W`;
- после успешной замены Stepler пропускает автоматическую смену раскладки именно для этого
  контекста, чтобы не отправлять Outlook/Zimbra дополнительные layout-сообщения.

Гипотеза: зависание могло запускаться не самой быстрой Win32-заменой, а последующим
`WM_INPUTLANGCHANGEREQUEST`/layout sync в зависимый Outlook/Zimbra window. Это еще нужно
проверить руками.

### Убрано Inspector.Activate()

Файл:

```text
F:\distr\system\Stepler\crates\stepler-platform-windows\src\powershell_scripts.rs
```

Из Outlook Word capture/apply scripts убрано:

```powershell
$inspector.Activate()
```

Причина: дополнительная активация inspector могла дергать Outlook/Zimbra UI и повышать риск hang.

### Timestamp в hotkey-log

Файлы:

```text
F:\distr\system\Stepler\crates\stepler-core\src\log_event.rs
F:\distr\system\Stepler\crates\stepler-cli\src\main.rs
```

В `stepler_hotkey_log.jsonl` добавлено поле:

```json
"timestamp_unix_ms": 1718000000123
```

Это timestamp записи результата операции. Старт операции можно примерно оценивать как:

```text
timestamp_unix_ms - duration_ms
```

### Timing overlay

Файл:

```text
F:\distr\system\Stepler\apps\Stepler.Tray\src\Program.cs
```

Добавлен overlay на 1 секунду после `P` / `CP`, например:

```text
P 14 ms
CP 1620 ms
P failed
CP failed
```

Добавлена настройка в tray:

```text
Показывать время P/CP
```

Настройка хранится как:

```json
"ShowTimingOverlay": true
```

## Важные наблюдения из hotkey-log

До добавления timestamp точная корреляция с зависанием была затруднена.

Наблюдались быстрые Outlook search операции через:

```text
app: rctrl_renwnd32
replacer: win32_edit_messages
duration: 8-11 ms
```

Примеры:

```text
Pause Completed rctrl_renwnd32 win32_edit_messages 9 примус -> ghbvec
Pause Completed rctrl_renwnd32 win32_edit_messages 10 cnjkt -> столе
ScrollLock Completed rctrl_renwnd32 win32_edit_messages 11 ghbvec yf cnjkt -> примус на столе
```

Свежий случай `2026-06-19`:

```text
Pause Completed app=rctrl_renwnd32 replacer=win32_edit_messages range=[7,11] vbbh -> миир duration=20 ms
Pause Completed app=rctrl_renwnd32 replacer=win32_edit_messages range=[7,15] миир -> vbbh duration=20 ms
```

В `hotkey_signal.log` вокруг этого момента:

```text
hook_terminal_detect kind=None app="rctrl_renwnd32" focused="RICHEDIT60W" title="Inbox - Zimbra - Alexey Andreev - Outlook"
layout_post hwnd=... layout=... sent=0 posted=1
```

После этого пользователь прислал скриншот Outlook с заголовком:

```text
Inbox - Zimbra - Alexey Andreev - Outlook (Не отвечает)
```

Вывод: даже если сама операция Stepler формально завершилась быстро и успешно, программная
замена текста через Win32/RichEdit могла запустить внутренний hang Outlook/Zimbra уже после
возврата из Stepler.

Outlook compose/editor операции через Word COM были медленнее:

```text
app: rctrl_renwnd32/_WwG
replacer: word_com
duration: about 1442-1505 ms
```

Пользователь отдельно заметил:

- Outlook search после фикса работал быстро, около `14 ms`.
- В письме Outlook было медленнее, около `1620 ms`.
- В Codex app около `1055 ms`.

## Собранные/собираемые версии Stepler

### Версия с Outlook safety changes

```text
SteplerSetup-0.1.1-alpha.20260616.t1137.exe
```

### Версия с timing overlay

```text
F:\distr\system\Stepler\SetupOutput\SteplerSetup-0.1.1-alpha.20260616.t1154.exe
```

### Версия с timestamp в hotkey-log

```text
F:\distr\system\Stepler\SetupOutput\SteplerSetup-0.1.1-alpha.20260616.t1208.exe
```

### Позже рабочая версия с Rocket.Chat fix

```text
F:\distr\system\Stepler\SetupOutput\SteplerSetup-0.1.1-alpha.20260616.t1546.exe
```

Эта версия также содержит предыдущие изменения по Outlook/timestamp/overlay.

### Версия с запретом Win32EditMessages для Outlook RichEdit

```text
BuildVersion: 1.0.20260619.t1152
Dist: F:\distr\system\Stepler\dist\Stepler
```

Содержит:

- фикс возврата caret для `Win32EditMessages`;
- запрет `Win32EditMessages` для Outlook `RichEdit*`;
- сохранение `Win32EditMessages` для Outlook plain `Edit` controls.

### Версия с возвратом Outlook search и пропуском layout-sync

```text
BuildVersion: 1.0.20260619.t1223
Dist: F:\distr\system\Stepler\dist\Stepler
```

Содержит:

- `Win32EditMessages` снова разрешен для Outlook `RICHEDIT60W`, чтобы работал поиск писем;
- автоматическая смена раскладки после P/CP пропускается для `rctrl_renwnd32/RICHEDIT60W`;
- Outlook письма с focused `_WwG` продолжают идти через `WordCom`.

## Проверенные команды

### Проверить процессы Stepler и Outlook

```powershell
Get-Process OUTLOOK,Stepler,stepler-cli -ErrorAction SilentlyContinue |
  Select-Object Id,ProcessName,Responding,CPU,StartTime,MainWindowTitle,Path
```

### Проверить, какая версия Stepler установлена

```powershell
Get-Content "F:\soft\sys\Stepler\BUILD_INFO.txt"
```

### Проверить последние hotkey events

```powershell
Get-Content "$env:LOCALAPPDATA\Stepler\logs\stepler_hotkey_log.jsonl" -Tail 20
```

### Проверить tray log

```powershell
Get-Content "$env:LOCALAPPDATA\Stepler\logs\Stepler.Tray.log" -Tail 80
```

### Запустить tray Stepler

```powershell
Start-Process "F:\soft\sys\Stepler\Stepler.exe"
```

### Если запущен только cli без tray

```powershell
Get-Process stepler-cli -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Process "F:\soft\sys\Stepler\Stepler.exe"
```

## Быстрые попытки оживить Outlook UI без перезапуска Windows

### 1. Остановить hotkey runner

```powershell
Get-Process stepler-cli -ErrorAction SilentlyContinue | Stop-Process -Force
```

Это убирает Stepler из цепочки, если зависание связано с hook/hotkey/overlay.

### 2. Minimize/restore Outlook window

```powershell
$p = Get-Process OUTLOOK -ErrorAction SilentlyContinue | Select-Object -First 1
Add-Type 'using System; using System.Runtime.InteropServices; public static class W { [DllImport("user32.dll")] public static extern bool ShowWindowAsync(IntPtr hWnd, int nCmdShow); [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd); }'
[W]::ShowWindowAsync($p.MainWindowHandle, 6) | Out-Null
Start-Sleep -Milliseconds 500
[W]::ShowWindowAsync($p.MainWindowHandle, 9) | Out-Null
[W]::SetForegroundWindow($p.MainWindowHandle) | Out-Null
```

Если `MainWindowHandle` равен `0`, этот способ может не сработать.

### 3. Попробовать COM activate

```powershell
(New-Object -ComObject Outlook.Application).ActiveExplorer().Activate()
```

Если получаем:

```text
80080005 CO_E_SERVER_EXEC_FAILURE
```

то текущий Outlook COM server не обслуживает запросы. Это сильный признак глубокого hang.

### 4. Если мягко не оживает

Перезапускать только Outlook, не Windows:

```powershell
Stop-Process -Name OUTLOOK -Force
```

После этого запустить Outlook обычным способом и вернуть Stepler tray:

```powershell
Start-Process "F:\soft\sys\Stepler\Stepler.exe"
```

## Что делать при следующем зависании

Не перезапускать Outlook сразу. Сначала собрать состояние.

### 1. Процессы

```powershell
Get-Process OUTLOOK,Stepler,stepler-cli -ErrorAction SilentlyContinue |
  Select-Object Id,ProcessName,Responding,CPU,StartTime,MainWindowTitle,Path
```

### 2. Последние hotkey events

```powershell
Get-Content "$env:LOCALAPPDATA\Stepler\logs\stepler_hotkey_log.jsonl" -Tail 50
```

Нужно смотреть:

- `timestamp_unix_ms`
- `trigger`
- `state`
- `app`
- `provider`
- `replacer`
- `range`
- `expected_before_text`
- `replacement_text`
- `duration_ms`
- `timings_ms`

### 3. Tray log

```powershell
Get-Content "$env:LOCALAPPDATA\Stepler\logs\Stepler.Tray.log" -Tail 100
```

### 4. Windows Event Log после зависания

```powershell
Get-WinEvent -FilterHashtable @{LogName='Application'; StartTime=(Get-Date).AddMinutes(-30)} |
  Where-Object { $_.ProviderName -match 'Application Hang|Windows Error Reporting|Application Error' -or $_.Message -match 'OUTLOOK|Zimbra|LSLIB32' } |
  Select-Object TimeCreated,ProviderName,Id,LevelDisplayName,Message
```

### 5. Проверить Zimbra modules в Outlook

```powershell
$p = Get-Process OUTLOOK -ErrorAction SilentlyContinue | Select-Object -First 1
$p.Modules |
  Where-Object { $_.ModuleName -match 'LS|Zimbra|SharingAddin|ShutdownAddin' } |
  Select-Object ModuleName,FileName,FileVersionInfo
```

Если доступ к modules будет запрещен, можно повторить PowerShell от администратора.

## Гипотезы

### Более вероятные

1. Outlook/Zimbra зависает после программной замены текста в поле Outlook, особенно если Zimbra в этот момент обрабатывает search/compose/editor state.
2. Прямой `Win32EditMessages` по Outlook `RichEdit*` может быть опаснее, чем казалось: операция завершается быстро, но Outlook/Zimbra зависает уже после нее.
3. Word COM в compose editor может быть медленным и иногда может дергать Outlook/Zimbra message loop.
4. Даже быстрый `Win32EditMessages` в Outlook search/plain edit может запускать внутреннюю реакцию Outlook/Zimbra после изменения текста, но пока он оставлен разрешенным только для plain `Edit`.

### Менее вероятные после фиксов

1. `Inspector.Activate()` как причина: убрано.
2. Broad Word COM probe по любым Outlook windows: сужено до `_WwG`.
3. UIA/clipboard risky fallback в Outlook: запрещены policy.

## Текущий статус

После изменений до `2026-06-19`:

- `P` в Outlook search быстрый.
- `P` в Outlook compose медленнее, но работает.
- Stepler tray и timing overlay работают.
- Для будущих hang есть timestamp в hotkey-log, чтобы привязать событие к точному времени.

После нового зависания `2026-06-19`:

- подтверждено, что Outlook зависает именно после `P` / `CP`, если включен Stepler или старый HotkeyHandler;
- свежий проблемный путь: `rctrl_renwnd32/RICHEDIT60W` через `win32_edit_messages`;
- полный fail-closed guard оказался слишком грубым: он отключил P/CP в поиске писем;
- текущий эксперимент: вернуть `Win32EditMessages` для Outlook `RICHEDIT60W`, но не выполнять layout-sync после замены;
- следующий безопасный шаг, если нужно вернуть поддержку этих Outlook-полей: отдельный адаптер для Outlook inline compose/reading-pane, вероятно через более точный Word/Outlook object model, а не через прямые Win32 messages.

После повторного зависания Outlook в августе 2026 добавлено дополнительное ограничение:

- для Outlook Word editor (`rctrl_renwnd32/_WwG`, replacer `word_com`) текстовая замена сохраняется;
- автоматическая смена раскладки после P/CP пропускается, чтобы не отправлять в Outlook/Zimbra
  повторные control/window/foreground layout-сообщения;
- это осознанное ограничение безопасности: после P/CP раскладку в Outlook editor при необходимости
  нужно переключить отдельно.

## Важное замечание

Новый Zimbra Connector ставить нельзя/нежелательно: пользователь отметил, что новый connector глючит с Outlook, а текущий старый работает и нужен для основной почты.

Поэтому стратегия не в обновлении Zimbra Connector, а в том, чтобы Stepler максимально безопасно работал рядом с Outlook/Zimbra и давал достаточно логов для диагностики редких зависаний.
