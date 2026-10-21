; VyomKunji one-click installer (Inno Setup)
; Build with ISCC.exe installer\vyomkunji.iss

#define MyAppName "VyomKunji"
#define MyAppVersion "0.2.1"
#define MyAppPublisher "VyomKunji Contributors"
#define MyAppURL "https://github.com/SV-stark/vyomkunji"

[Setup]
AppId={{B7E3F1A2-9C4D-4E5F-8A1B-2C3D4E5F6A7B}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
DefaultDirName={pf}\{#MyAppName}
DefaultGroupName={#MyAppName}
OutputDir=..\dist
OutputBaseFilename=vyomkunji-{#MyAppVersion}-setup
Compression=lzma
SolidCompression=yes
ArchitecturesInstallIn64BitMode=x64

[Files]
Source: "..\native-app\target\release\vyomkunji.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\native-app\target\release\vyomkunji-host.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\native-app\install-host.ps1"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\native-app\com.vyomkunji.host.json"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\native-app\com.vyomkunji.host.firefox.json"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\extension\*"; DestDir: "{app}\extension"; Flags: ignoreversion recursesubdirs
Source: "..\extension-firefox\*"; DestDir: "{app}\extension-firefox"; Flags: ignoreversion recursesubdirs

[Run]
Filename: "powershell.exe"; Parameters: "-ExecutionPolicy Bypass -File ""{app}\install-host.ps1"" -ExePath ""{app}\vyomkunji-host.exe"" -Browser Both"; StatusMsg: "Registering native messaging host..."; Flags: runhidden waituntilterminated

[Icons]
Name: "{group}\VyomKunji"; Filename: "{app}\vyomkunji.exe"
Name: "{commondesktop}\VyomKunji"; Filename: "{app}\vyomkunji.exe"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional icons:"

[Code]
procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
  begin
    MsgBox('VyomKunji installed. Load the unpacked extension from:' + #13#10 +
           '{app}\extension (Chrome/Edge) or {app}\extension-firefox (Firefox).',
           mbInformation, MB_OK);
  end;
end;
