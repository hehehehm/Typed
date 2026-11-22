#ifndef MyAppVersion
  #define MyAppVersion "0.3.85"
#endif
#ifndef BuildDir
  #define BuildDir "..\..\target\release"
#endif
#ifndef InstallerOutputDir
  #define InstallerOutputDir "..\..\dist"
#endif

[Setup]
AppId={{08E53160-AB9C-43F4-9025-DB168DEDB7D9}
AppName=Typed
AppPublisher=Typed
AppVersion={#MyAppVersion}
VersionInfoDescription=Typed
VersionInfoProductName=Typed
DefaultDirName={localappdata}\Typed
DefaultGroupName=Typed
UsePreviousAppDir=no
PrivilegesRequired=lowest
OutputDir={#InstallerOutputDir}
OutputBaseFilename=Typed-Setup-{#MyAppVersion}
LicenseFile=..\..\LICENSE
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ShowLanguageDialog=auto
CloseApplications=yes
CloseApplicationsFilter=typed.exe,layout-keeper.exe
RestartApplications=no
SetupIconFile=..\..\assets\typed.ico
UninstallDisplayIcon={app}\typed.exe

[Languages]
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[CustomMessages]
russian.DesktopIcon=Создать значок на рабочем столе
english.DesktopIcon=Create a desktop shortcut
russian.Startup=Запускать Typed при входе в Windows
english.Startup=Start Typed when signing in to Windows
russian.StartDaemon=Запустить Typed
english.StartDaemon=Start Typed
russian.OpenSettings=Открыть Typed
english.OpenSettings=Open Typed

[Tasks]
Name: "desktopicon"; Description: "{cm:DesktopIcon}"; Flags: unchecked
Name: "startup"; Description: "{cm:Startup}"

[Files]
Source: "{#BuildDir}\typed.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "start-hidden.vbs"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Typed"; Filename: "{app}\typed.exe"; Parameters: "gui"; WorkingDir: "{app}"
Name: "{userdesktop}\Typed"; Filename: "{app}\typed.exe"; Parameters: "gui"; WorkingDir: "{app}"; Tasks: desktopicon
Name: "{userstartup}\Typed"; Filename: "{sys}\wscript.exe"; Parameters: """{app}\start-hidden.vbs"""; WorkingDir: "{app}"; Tasks: startup

[Run]
Filename: "{app}\typed.exe"; Parameters: "gui"; WorkingDir: "{app}"; Description: "{cm:OpenSettings}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "{sys}\taskkill.exe"; Parameters: "/F /IM typed.exe /T"; Flags: runhidden waituntilterminated skipifdoesntexist; RunOnceId: "CloseTypedDaemon"

[Code]
function TypedTaskExists: Boolean;
var
  ResultCode: Integer;
begin
  Result := Exec(ExpandConstant('{sys}\schtasks.exe'), '/Query /TN "Typed"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode) and (ResultCode = 0);
end;

procedure StopTypedProcesses;
var
  ResultCode: Integer;
begin
  if TypedTaskExists then
    Exec(ExpandConstant('{sys}\schtasks.exe'), '/End /TN "Typed"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM typed.exe /T', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM layout-keeper.exe /T', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Sleep(400);
end;

procedure DeleteTypedTask;
var
  ResultCode: Integer;
begin
  if not TypedTaskExists then
    Exit;
  Exec(ExpandConstant('{sys}\schtasks.exe'), '/Delete /TN "Typed" /F', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  if TypedTaskExists then
    ShellExec('runas', ExpandConstant('{sys}\schtasks.exe'), '/Delete /TN "Typed" /F', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
end;

const
  TypedUninstallKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{08E53160-AB9C-43F4-9025-DB168DEDB7D9}_is1';

function ExtractExePath(const Cmd: String): String;
var
  S: String;
  P: Integer;
begin
  S := Trim(Cmd);
  if (Length(S) > 0) and (S[1] = '"') then
  begin
    Delete(S, 1, 1);
    P := Pos('"', S);
    if P > 0 then
      S := Copy(S, 1, P - 1);
  end
  else
  begin
    P := Pos(' /', S);
    if P > 0 then
      S := Copy(S, 1, P - 1);
  end;
  Result := S;
end;

procedure UninstallFrom(const RootKey: Integer);
var
  Cmd, Exe: String;
  ResultCode, I: Integer;
begin
  Cmd := '';
  if not RegQueryStringValue(RootKey, TypedUninstallKey, 'UninstallString', Cmd) then
    RegQueryStringValue(RootKey, TypedUninstallKey, 'QuietUninstallString', Cmd);
  Exe := ExtractExePath(Cmd);
  if (Exe = '') or (not FileExists(Exe)) then
    Exit;
  Log('Removing the previous Typed version: ' + Exe);
  if Exec(Exe, '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART', '', SW_HIDE, ewWaitUntilTerminated, ResultCode) then
  begin
    I := 0;
    while (RegKeyExists(RootKey, TypedUninstallKey) or FileExists(Exe)) and (I < 120) do
    begin
      Sleep(250);
      I := I + 1;
    end;
    Log('Previous version removed, exit code ' + IntToStr(ResultCode));
  end
  else
    Log('Could not start the previous uninstaller: ' + SysErrorMessage(ResultCode));
end;

procedure RemovePreviousVersion;
begin
  UninstallFrom(HKCU);
  UninstallFrom(HKLM);
  if IsWin64 then
    UninstallFrom(HKLM64);
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  StopTypedProcesses;
  RemovePreviousVersion;
  StopTypedProcesses;
  Result := '';
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  ResultCode: Integer;
  Exe: String;
begin
  if CurStep <> ssPostInstall then
    Exit;
  Exe := ExpandConstant('{app}\typed.exe');
  if not TypedTaskExists then
    Exit;
  if Exec(Exe, 'admin-task needs-refresh', ExpandConstant('{app}'), SW_HIDE, ewWaitUntilTerminated, ResultCode) and (ResultCode = 0) then
    ShellExec('runas', Exe, 'admin-task refresh', ExpandConstant('{app}'), SW_HIDE, ewWaitUntilTerminated, ResultCode);
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
  begin
    StopTypedProcesses;
    if not UninstallSilent then
      DeleteTypedTask;
  end;
end;

[InstallDelete]
Type: files; Name: "{app}\layout-keeper.exe"
Type: files; Name: "{localappdata}\LayoutKeeper\layout-keeper.exe"
Type: files; Name: "{localappdata}\LayoutKeeper\LICENSE"
Type: files; Name: "{localappdata}\LayoutKeeper\start-hidden.vbs"
Type: files; Name: "{localappdata}\LayoutKeeper\unins000.exe"
Type: files; Name: "{localappdata}\LayoutKeeper\unins000.dat"
Type: dirifempty; Name: "{localappdata}\LayoutKeeper"
Type: files; Name: "{userprograms}\LayoutKeeper\LayoutKeeper"
Type: files; Name: "{userprograms}\LayoutKeeper\LayoutKeeper — настройки"
Type: dirifempty; Name: "{userprograms}\LayoutKeeper"
Type: files; Name: "{userdesktop}\LayoutKeeper — настройки"
Type: files; Name: "{userstartup}\LayoutKeeper"
Type: files; Name: "{group}\Typed — настройки"
Type: files; Name: "{userdesktop}\Typed — настройки"
