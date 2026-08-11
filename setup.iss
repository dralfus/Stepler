#define MyAppName "Stepler"
#ifndef MyAppDistDir
#define MyAppDistDir "dist\Stepler"
#endif
#ifndef MyAppOutputDir
#define MyAppOutputDir "SetupOutput"
#endif
#define MyAppExe MyAppDistDir + "\Stepler.exe"
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
OutputDir={#MyAppOutputDir}
OutputBaseFilename=SteplerSetup-{#MyAppVersion}
Compression=lzma
SolidCompression=yes
PrivilegesRequired=admin
CloseApplications=yes
RestartApplications=no

[Files]
Source: "{#MyAppDistDir}\*"; DestDir: "{app}"; Flags: recursesubdirs createallsubdirs ignoreversion

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
const
  SteplerProfileBeginMarker = '# >>> Stepler PSReadLine adapter >>>';
  SteplerProfileEndMarker = '# <<< Stepler PSReadLine adapter <<<';

function EscapePowerShellSingleQuotedString(Value: string): string;
begin
  Result := Value;
  StringChangeEx(Result, '''', '''''', True);
end;

procedure AppendPowerShellProfileLine(var Lines: TArrayOfString; Value: string);
var
  LineCount: Integer;
begin
  LineCount := GetArrayLength(Lines);
  SetArrayLength(Lines, LineCount + 1);
  Lines[LineCount] := Value;
end;

procedure AppendSteplerPowerShellProfileBlock(var Lines: TArrayOfString);
var
  AdapterPath: string;
  CliPath: string;
begin
  AdapterPath := EscapePowerShellSingleQuotedString(ExpandConstant('{app}\scripts\Stepler.PSReadLine.ps1'));
  CliPath := EscapePowerShellSingleQuotedString(ExpandConstant('{app}\stepler-cli.exe'));
  AppendPowerShellProfileLine(Lines, SteplerProfileBeginMarker);
  AppendPowerShellProfileLine(Lines, 'try {');
  AppendPowerShellProfileLine(Lines, '    $steplerPsReadLine = ''' + AdapterPath + '''');
  AppendPowerShellProfileLine(Lines, '    $steplerCli = ''' + CliPath + '''');
  AppendPowerShellProfileLine(Lines, '    if (Test-Path -LiteralPath $steplerPsReadLine) {');
  AppendPowerShellProfileLine(Lines, '        Import-Module PSReadLine -ErrorAction SilentlyContinue');
  AppendPowerShellProfileLine(Lines, '        . $steplerPsReadLine -SteplerCli $steplerCli -Quiet');
  AppendPowerShellProfileLine(Lines, '    }');
  AppendPowerShellProfileLine(Lines, '} catch {');
  AppendPowerShellProfileLine(Lines, '}');
  AppendPowerShellProfileLine(Lines, SteplerProfileEndMarker);
end;

function FindPowerShellProfileMarker(const Lines: TArrayOfString; StartIndex: Integer; Marker: string): Integer;
var
  Index: Integer;
begin
  Result := -1;
  for Index := StartIndex to GetArrayLength(Lines) - 1 do
  begin
    if Lines[Index] = Marker then
    begin
      Result := Index;
      exit;
    end;
  end;
end;

procedure EnsureSteplerPowerShellProfile(ProfilePath: string);
var
  Existing: TArrayOfString;
  Next: TArrayOfString;
  Index: Integer;
  EndIndex: Integer;
begin
  if not ForceDirectories(ExtractFileDir(ProfilePath)) then
  begin
    Log('Stepler PowerShell profile directory unavailable: ' + ProfilePath);
    exit;
  end;

  if FileExists(ProfilePath) then
  begin
    if not LoadStringsFromFile(ProfilePath, Existing) then
    begin
      Log('Stepler PowerShell profile read failed: ' + ProfilePath);
      exit;
    end;
  end;

  SetArrayLength(Next, 0);
  Index := 0;
  while Index < GetArrayLength(Existing) do
  begin
    if Existing[Index] = SteplerProfileBeginMarker then
    begin
      EndIndex := FindPowerShellProfileMarker(Existing, Index + 1, SteplerProfileEndMarker);
      if EndIndex >= 0 then
      begin
        Index := EndIndex + 1;
        continue;
      end;
    end;

    AppendPowerShellProfileLine(Next, Existing[Index]);
    Index := Index + 1;
  end;

  while (GetArrayLength(Next) > 0) and (Trim(Next[GetArrayLength(Next) - 1]) = '') do
    SetArrayLength(Next, GetArrayLength(Next) - 1);
  if GetArrayLength(Next) > 0 then
    AppendPowerShellProfileLine(Next, '');
  AppendSteplerPowerShellProfileBlock(Next);

  if not SaveStringsToUTF8File(ProfilePath, Next, False) then
    Log('Stepler PowerShell profile write failed: ' + ProfilePath)
  else
    Log('Stepler PowerShell profile ensured: ' + ProfilePath);
end;

procedure EnsureSteplerPowerShellProfiles();
var
  DocumentsPath: string;
begin
  DocumentsPath := ExpandConstant('{userdocs}');
  EnsureSteplerPowerShellProfile(AddBackslash(DocumentsPath) + 'PowerShell\profile.ps1');
  EnsureSteplerPowerShellProfile(AddBackslash(DocumentsPath) + 'PowerShell\Microsoft.PowerShell_profile.ps1');
  EnsureSteplerPowerShellProfile(AddBackslash(DocumentsPath) + 'WindowsPowerShell\profile.ps1');
  EnsureSteplerPowerShellProfile(AddBackslash(DocumentsPath) + 'WindowsPowerShell\Microsoft.PowerShell_profile.ps1');
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
    EnsureSteplerPowerShellProfiles();
end;

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
