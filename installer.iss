; Cleaner — Inno Setup installer script.
; Build with: ISCC.exe installer.iss   (produces dist\Cleaner-Setup-<ver>.exe)

#define AppVersion "2.9.5"

[Setup]
AppId={{B8C7A1E2-4F3D-4A5B-9C6E-2D1F0A3B5C7D}
AppName=Cleaner
AppVersion={#AppVersion}
AppVerName=Cleaner {#AppVersion}
AppPublisher=Emre
AppPublisherURL=https://github.com/w0wzahh/cleaner
AppSupportURL=https://github.com/w0wzahh/cleaner/issues
AppUpdatesURL=https://github.com/w0wzahh/cleaner/releases
DefaultDirName={autopf}\Cleaner
DefaultGroupName=Cleaner
; Per-user install into %LOCALAPPDATA%\Programs\Cleaner — no admin prompt.
; That folder is writable, so the app keeps settings/log/reports next to the
; exe and a full uninstall removes everything in one shot. If the exe ever
; lands somewhere read-only, the app automatically moves its data to
; %APPDATA%\Cleaner instead (see data_dir() in settings.rs).
DisableProgramGroupPage=yes
LicenseFile=LICENSE
OutputDir=dist
OutputBaseFilename=Cleaner-Setup-{#AppVersion}
SetupIconFile=assets\icon.ico
UninstallDisplayIcon={app}\cleaner.exe
UninstallDisplayName=Cleaner
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Additional icons:"; Flags: unchecked

[Files]
Source: "target\release\cleaner.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Cleaner"; Filename: "{app}\cleaner.exe"; Comment: "Lightweight system cleanup"
Name: "{group}\Uninstall Cleaner"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Cleaner"; Filename: "{app}\cleaner.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\cleaner.exe"; Description: "Launch Cleaner"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; Whatever remains in the install dir goes away on uninstall.
Type: filesandordirs; Name: "{app}"

[Code]
// Offer a truly complete uninstall: remove the scheduled task (if the user
// registered one), then ask whether the data folder (settings, history log,
// exported reports) should go too.
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  ResultCode: Integer;
begin
  if CurUninstallStep = usPostUninstall then
  begin
    Exec('schtasks.exe', '/Delete /F /TN CleanerScheduledScan', '',
         SW_HIDE, ewWaitUntilTerminated, ResultCode);
    if DirExists(ExpandConstant('{userappdata}\Cleaner')) then
      if MsgBox('Also delete Cleaner''s data folder?' + #13#10 + #13#10 +
                ExpandConstant('{userappdata}\Cleaner') + #13#10 +
                '(settings, history log, exported reports)',
                mbConfirmation, MB_YESNO) = IDYES then
        DelTree(ExpandConstant('{userappdata}\Cleaner'), True, True, True);
  end;
end;
