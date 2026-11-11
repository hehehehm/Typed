#ifndef MyAppVersion
  #define MyAppVersion "0.3.67"
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
CloseApplicationsFilter=layout-keeper.exe
RestartApplications=no
SetupIconFile=..\..\assets\typed.ico
UninstallDisplayIcon={app}\layout-keeper.exe
; User data is NEVER deleted, neither by this setup nor by its uninstaller, nor by the
; previous-version removal in [Code]: config.json, vault.bin and typed.chk live in
; %APPDATA%\typed (legacy: %APPDATA%\layout-keeper), i.e. AppData\Roaming, while the
; program goes to AppData\Local. There is deliberately no [UninstallDelete] section and
; [InstallDelete] below only lists old program files and shortcuts.

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
Source: "{#BuildDir}\layout-keeper.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "start-hidden.vbs"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
; Settings ensure the background daemon is running; never start a second visible daemon.
Name: "{group}\Typed"; Filename: "{app}\layout-keeper.exe"; Parameters: "gui"; WorkingDir: "{app}"
Name: "{userdesktop}\Typed"; Filename: "{app}\layout-keeper.exe"; Parameters: "gui"; WorkingDir: "{app}"; Tasks: desktopicon
Name: "{userstartup}\Typed"; Filename: "{sys}\wscript.exe"; Parameters: """{app}\start-hidden.vbs"""; WorkingDir: "{app}"; Tasks: startup

[Run]
Filename: "{app}\layout-keeper.exe"; Parameters: "gui"; WorkingDir: "{app}"; Description: "{cm:OpenSettings}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
; Safety net after files are removed. The real stop happens in [Code] below,
; before deletion: the daemon has no window, so only /F actually ends it.
Filename: "{sys}\taskkill.exe"; Parameters: "/F /IM layout-keeper.exe /T"; Flags: runhidden waituntilterminated skipifdoesntexist; RunOnceId: "CloseTypedDaemon"

[Code]
// «Запуск администратором» (0.3.67+) registers the Task Scheduler task "Typed".
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
  // A daemon started by the task runs as administrator: taskkill from this
  // non-elevated setup cannot end it, the task itself can.
  if TypedTaskExists then
    Exec(ExpandConstant('{sys}\schtasks.exe'), '/End /TN "Typed"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM layout-keeper.exe /T', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Sleep(400);
end;

// Uninstall: delete the task. It was created with administrator rights, so if a plain
// delete fails, ask Windows once (UAC) to delete it as administrator.
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

// Uninstall entry written by every Typed / LayoutKeeper installer.
// Must match AppId in [Setup] (the doubled brace there is just Inno escaping).
const
  TypedUninstallKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{08E53160-AB9C-43F4-9025-DB168DEDB7D9}_is1';

// Path of the uninstaller from an UninstallString like "C:\...\unins000.exe" /SILENT.
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

// Silently run the previous version's uninstaller registered under RootKey, wherever
// it was installed (also the old %LOCALAPPDATA%\LayoutKeeper folder), and wait for it.
// The old uninstallers only remove program files and shortcuts: user data in
// %APPDATA%\typed and %APPDATA%\layout-keeper is never touched.
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
    // The uninstaller finishes in a copy of itself in %TEMP%: wait (up to ~30 s) until
    // its registry entry and its exe are gone, so it cannot delete the new files.
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
  // Stop the running daemon first (it has no window, so only taskkill /F ends it),
  // then remove the previous version before any new file is copied.
  StopTypedProcesses;
  RemovePreviousVersion;
  StopTypedProcesses;
  Result := '';
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
  begin
    StopTypedProcesses;
    // A new setup removes the previous version silently: keep the task then, so an
    // update does not switch «Запуск администратором» off (same exe path).
    if not UninstallSilent then
      DeleteTypedTask;
  end;
end;

[InstallDelete]
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
