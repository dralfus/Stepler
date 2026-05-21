#define MyAppName "Stepler"
#define MyAppExe "dist\Stepler\Stepler.exe"
#ifndef MyAppVersion
#define MyAppVersion GetStringFileInfo(AddBackslash(SourcePath) + MyAppExe, "ProductVersion")
#endif
#define MyAppId "{{B8E43B8B-ED11-4E36-B0E2-6F92B0786E0B}}"

[Setup]
AppId={#MyAppId}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
UninstallDisplayName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppName}
UninstallDisplayIcon={app}\Stepler.exe
DefaultDirName={commonpf}\Stepler
DefaultGroupName=Stepler
OutputDir=SetupOutput
OutputBaseFilename=SteplerSetup-{#MyAppVersion}
Compression=lzma
SolidCompression=yes
PrivilegesRequired=admin
CloseApplications=yes
RestartApplications=no

[Files]
Source: "dist\Stepler\*"; DestDir: "{app}"; Flags: recursesubdirs createallsubdirs ignoreversion

[Icons]
Name: "{group}\Stepler"; Filename: "{app}\Stepler.exe"
Name: "{commonprograms}\Stepler"; Filename: "{app}\Stepler.exe"
Name: "{commondesktop}\Stepler"; Filename: "{app}\Stepler.exe"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "Create a desktop icon"; GroupDescription: "Additional icons:"; Flags: unchecked

[Registry]
Root: HKLM; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\Stepler.exe"; ValueType: string; ValueName: ""; ValueData: "{app}\Stepler.exe"; Flags: uninsdeletekey
Root: HKLM; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\Stepler.exe"; ValueType: string; ValueName: "Path"; ValueData: "{app}"

[Code]
function ExtractExecutablePath(CommandLine: string): string;
var
  QuotePos: Integer;
  SpacePos: Integer;
begin
  CommandLine := Trim(CommandLine);
  if CommandLine = '' then
  begin
    Result := '';
    exit;
  end;

  if CommandLine[1] = '"' then
  begin
    Delete(CommandLine, 1, 1);
    QuotePos := Pos('"', CommandLine);
    if QuotePos > 0 then
      Result := Copy(CommandLine, 1, QuotePos - 1)
    else
      Result := CommandLine;
  end
  else
  begin
    SpacePos := Pos(' ', CommandLine);
    if SpacePos > 0 then
      Result := Copy(CommandLine, 1, SpacePos - 1)
    else
      Result := CommandLine;
  end;
end;

function TryGetPreviousUninstaller(var UninstallerPath: string): Boolean;
var
  UninstallKey: string;
  UninstallCommand: string;
begin
  UninstallerPath := ExpandConstant('{app}\unins000.exe');
  Result := FileExists(UninstallerPath);
  if Result then
    exit;

  UninstallKey := 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{#MyAppId}_is1';
  Result :=
    RegQueryStringValue(HKLM, UninstallKey, 'QuietUninstallString', UninstallCommand) or
    RegQueryStringValue(HKLM, UninstallKey, 'UninstallString', UninstallCommand) or
    RegQueryStringValue(HKCU, UninstallKey, 'QuietUninstallString', UninstallCommand) or
    RegQueryStringValue(HKCU, UninstallKey, 'UninstallString', UninstallCommand);

  if Result then
  begin
    UninstallerPath := ExtractExecutablePath(UninstallCommand);
    Result := FileExists(UninstallerPath);
  end;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  UninstallerPath: string;
  ResultCode: Integer;
begin
  Result := '';

  Exec(ExpandConstant('{cmd}'), '/C taskkill /F /T /IM Stepler.exe >nul 2>&1', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec(ExpandConstant('{cmd}'), '/C taskkill /F /T /IM Stepler.Tray.exe >nul 2>&1', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec(ExpandConstant('{cmd}'), '/C taskkill /F /T /IM stepler-cli.exe >nul 2>&1', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);

  if TryGetPreviousUninstaller(UninstallerPath) then
  begin
    Log('Previous version detected. Running uninstaller: ' + UninstallerPath);
    if not Exec(UninstallerPath, '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART', '', SW_HIDE, ewWaitUntilTerminated, ResultCode) then
    begin
      Result := 'Не удалось запустить деинсталляцию предыдущей версии Stepler.';
      exit;
    end;

    if ResultCode <> 0 then
    begin
      Result := 'Деинсталляция предыдущей версии Stepler завершилась с кодом ' + IntToStr(ResultCode) + '.';
      exit;
    end;

    Sleep(1000);
  end;
end;

[Run]
Filename: "{app}\Stepler.exe"; Description: "Launch Stepler"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
Type: filesandordirs; Name: "{app}"
