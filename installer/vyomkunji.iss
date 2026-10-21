; VyomKunji one-click installer (Inno Setup)
; Build with ISCC.exe installer\vyomkunji.iss

#define MyAppName "VyomKunji"
#define MyAppVersion "0.3.0"
#define MyAppPublisher "VyomKunji Contributors"
#define MyAppURL "https://github.com/SV-stark/vyomkunji"
#define MyAppExeName "vyomkunji.exe"

[Setup]
AppId={{B7E3F1A2-9C4D-4E5F-8A1B-2C3D4E5F6A7B}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}/issues
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
OutputDir=..\dist
OutputBaseFilename=vyomkunji-{#MyAppVersion}-setup
Compression=lzma
SolidCompression=yes
ArchitecturesInstallIn64BitMode=x64
; Allow a non-admin user to install per-user; the dialog lets them choose.
PrivilegesRequired=admin
PrivilegesRequiredOverridesAllowed=dialog
; Close a running VyomKunji before replacing its binaries; never relaunch it
; automatically because the user may have been in the middle of a signing flow.
CloseApplications=yes
CloseApplicationsFilter={#MyAppExeName},vyomkunji-host.exe
RestartApplications=no
UninstallDisplayIcon={app}\{#MyAppExeName}
UninstallDisplayName={#MyAppName}
VersionInfoVersion={#MyAppVersion}
VersionInfoProductName={#MyAppName}
VersionInfoDescription={#MyAppName} installer

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional icons:"
Name: "autostart"; Description: "Start VyomKunji when I sign in"; GroupDescription: "Startup:"; Flags: unchecked

[Files]
Source: "..\native-app\target\release\vyomkunji.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\native-app\target\release\vyomkunji-host.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\native-app\install-host.ps1"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\native-app\com.vyomkunji.host.json"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\native-app\com.vyomkunji.host.firefox.json"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\extension\*"; DestDir: "{app}\extension"; Flags: ignoreversion recursesubdirs
Source: "..\extension-firefox\*"; DestDir: "{app}\extension-firefox"; Flags: ignoreversion recursesubdirs

[Registry]
; Per-user autostart. HKCU is used even for a machine-wide install so that the
; setting follows the user who chose the task.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "VyomKunji"; ValueData: """{app}\{#MyAppExeName}"""; Flags: uninsdeletevalue; Tasks: autostart

[Run]
Filename: "powershell.exe"; Parameters: "-NoProfile -ExecutionPolicy Bypass -File ""{app}\install-host.ps1"" -ExePath ""{app}\vyomkunji-host.exe"" -Browser Both"; StatusMsg: "Registering native messaging host..."; Flags: runhidden waituntilterminated

[UninstallRun]
Filename: "powershell.exe"; Parameters: "-NoProfile -ExecutionPolicy Bypass -File ""{app}\install-host.ps1"" -Unregister"; Flags: runhidden waituntilterminated; RunOnceId: "UnregisterNativeHost"

[UninstallDelete]
Type: files; Name: "{app}\*.installed.json"
Type: files; Name: "{app}\*.firefox.installed.json"
Type: filesandordirs; Name: "{app}\extension"
Type: filesandordirs; Name: "{app}\extension-firefox"

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Code]
function InitializeUninstall(): Boolean;
var
  ResultCode: Integer;
begin
  // Best-effort: close a running tray instance before removing files. Inno's
  // CloseApplications also handles this in most cases, but an elevated
  // uninstaller cannot always see a process in another session.
  Exec('taskkill.exe', '/IM {#MyAppExeName} /F /T', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Result := True;
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
  begin
    MsgBox('VyomKunji installed. Load the unpacked extension from:' + #13#10 +
           ExpandConstant('{app}') + '\extension (Chrome/Edge) or ' +
           ExpandConstant('{app}') + '\extension-firefox (Firefox).',
           mbInformation, MB_OK);
  end;
end;
