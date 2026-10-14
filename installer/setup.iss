; Instalador do Korvon Macro (Inno Setup 6).
; Local:  iscc /DAppVersion=1.0.0 installer\setup.iss   (após installer\prepare-installer.ps1)
; Saída:  output\KorvonMacro-Setup-<versao>.exe

#ifndef AppVersion
  #define AppVersion "1.0.0"
#endif

#define AppName      "Korvon Macro"
#define AppPublisher "Fórmula Ativa"
#define AppURL       "https://github.com/formula-ativa/korvon-macro"
#define AppExeName   "korvon-macro.exe"
#define DistDir      "..\dist\installer"

[Setup]
; GUID fixo: identifica o app para atualizações/desinstalação. Não altere entre versões.
AppId={{6B0F7C1E-3A52-4D8E-9C47-0A1B2C3D4E5F}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}/issues
AppUpdatesURL={#AppURL}/releases
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
UninstallDisplayName={#AppName}
UninstallDisplayIcon={app}\{#AppExeName}
; Windows 10 ou superior, somente 64 bits.
MinVersion=10.0
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=admin
PrivilegesRequiredOverridesAllowed=dialog
OutputDir=..\output
OutputBaseFilename=KorvonMacro-Setup-{#AppVersion}
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no
; Metadados do executável do instalador
VersionInfoVersion={#AppVersion}
VersionInfoCompany={#AppPublisher}
VersionInfoProductName={#AppName}
VersionInfoProductVersion={#AppVersion}
VersionInfoDescription=Instalador do {#AppName}
VersionInfoCopyright=© {#AppPublisher}
; Ícone do instalador: usado só se installer\korvon.ico existir.
#if FileExists(SourcePath + "korvon.ico")
SetupIconFile=korvon.ico
#endif
; Página de licença: usada só se houver LICENSE/LICENSE.txt.
#if FileExists(DistDir + "\LICENSE.txt")
LicenseFile={#DistDir}\LICENSE.txt
#elif FileExists(DistDir + "\LICENSE")
LicenseFile={#DistDir}\LICENSE
#endif

[Languages]
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Components]
Name: "main"; Description: "Korvon Macro (obrigatório)"; Types: full compact custom; Flags: fixed
Name: "docs"; Description: "Documentação (formato .mrf)"; Types: full

[Files]
Source: "{#DistDir}\{#AppExeName}"; DestDir: "{app}"; Components: main; Flags: ignoreversion
#if FileExists(DistDir + "\LICENSE")
Source: "{#DistDir}\LICENSE"; DestDir: "{app}"; Components: main; Flags: ignoreversion
#endif
#if FileExists(DistDir + "\LICENSE.txt")
Source: "{#DistDir}\LICENSE.txt"; DestDir: "{app}"; Components: main; Flags: ignoreversion
#endif
#if FileExists(DistDir + "\README.md")
Source: "{#DistDir}\README.md"; DestDir: "{app}"; Components: main; Flags: ignoreversion
#endif
#if FileExists(DistDir + "\analise-mrf.md")
Source: "{#DistDir}\analise-mrf.md"; DestDir: "{app}\docs"; Components: docs; Flags: ignoreversion
#endif
#if DirExists(DistDir + "\extras")
Source: "{#DistDir}\extras\*"; DestDir: "{app}\extras"; Components: main; Flags: ignoreversion recursesubdirs createallsubdirs
#endif

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExeName}"
Name: "{group}\Desinstalar {#AppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#AppExeName}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
; Remove as tarefas agendadas criadas pelo app (pasta "Korvon Macro" do Agendador de Tarefas).
Filename: "powershell.exe"; Parameters: "-NoProfile -ExecutionPolicy Bypass -Command ""Get-ScheduledTask -TaskPath '\Korvon Macro\' -ErrorAction SilentlyContinue | Unregister-ScheduledTask -Confirm:$false"""; Flags: runhidden; RunOnceId: "RemoveScheduledTasks"

[Code]
// Pergunta se as macros do usuário (%APPDATA%\Korvon Macro) também devem ser apagadas.
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  DataDir: String;
begin
  if CurUninstallStep = usPostUninstall then
  begin
    DataDir := ExpandConstant('{userappdata}\Korvon Macro');
    if DirExists(DataDir) and (not UninstallSilent) then
      if MsgBox('Deseja remover também suas macros salvas e configurações?' + #13#10 + DataDir,
                mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES then
        DelTree(DataDir, True, True, True);
  end;
end;
