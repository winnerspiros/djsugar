; Inno Setup script for DJ Sugar's Windows setup. scripts/bundle-windows.ps1
; runs it with /DVersion=... /DArch=x64|arm64 /DSource=<folder with DJSugar.exe> /DOutput=dist.
; Installs for the current user only, so it needs no administrator.

#ifndef Version
  #define Version "0.0.0"
#endif
; x64, or arm64 for Windows on Arm.
#ifndef Arch
  #define Arch "x64"
#endif

[Setup]
; Never change the AppId: it is how Windows knows a new setup updates the
; installed DJ Sugar instead of adding a second one.
AppId={{59D890AB-0739-4D1A-987B-2E441345DC98}
AppName=DJ Sugar
AppVersion={#Version}
AppVerName=DJ Sugar {#Version}
AppPublisher=DJ Sugar
AppPublisherURL=https://github.com/winnerspiros/djsugar
AppSupportURL=https://github.com/winnerspiros/djsugar/issues
AppUpdatesURL=https://github.com/winnerspiros/djsugar/releases
DefaultDirName={autopf}\DJ Sugar
DefaultGroupName=DJ Sugar
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
#if Arch == "arm64"
ArchitecturesAllowed=arm64
ArchitecturesInstallIn64BitMode=arm64
#else
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
#endif
MinVersion=10.0
LicenseFile={#Source}\LICENSE-GPL3.txt
SetupIconFile=djsugar.ico
UninstallDisplayIcon={app}\DJSugar.exe
OutputDir={#Output}
OutputBaseFilename=DJSugar-{#Version}-windows-{#Arch}-setup
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#Source}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[InstallDelete]
; Mappings removed from a newer version shouldn't linger.
Type: filesandordirs; Name: "{app}\controllers"
Type: filesandordirs; Name: "{app}\mods"

[Icons]
Name: "{autoprograms}\DJ Sugar"; Filename: "{app}\DJSugar.exe"
Name: "{autodesktop}\DJ Sugar"; Filename: "{app}\DJSugar.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\DJSugar.exe"; Description: "{cm:LaunchProgram,DJ Sugar}"; Flags: nowait postinstall skipifsilent
; After an in-app update, which runs this setup silently with /RELAUNCH.
Filename: "{app}\DJSugar.exe"; Flags: nowait; Check: IsRelaunch

[Code]
function IsRelaunch: Boolean;
begin
  Result := Pos('/RELAUNCH', UpperCase(GetCmdTail)) > 0;
end;
