; Instalador do Korvon Macro (Inno Setup 6).
; Uso: ISCC /DAppVersion=1.0.0 installer\korvon-macro.iss   (gera dist\korvon-macro-setup-<versão>.exe)

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
AppId={{6B0F7C1E-3A52-4D8E-9C47-0A1B2C3D4E5F}
AppName=Korvon Macro
AppVersion={#AppVersion}
AppPublisher=Fórmula Ativa
DefaultDirName={autopf}\Korvon Macro
DefaultGroupName=Korvon Macro
DisableProgramGroupPage=yes
UninstallDisplayIcon={app}\korvon-macro.exe
OutputDir=..\dist
OutputBaseFilename=korvon-macro-setup-{#AppVersion}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; Instala só para o usuário por padrão (sem administrador); o assistente permite escolher para todos.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog

[Languages]
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[Tasks]
Name: "desktopicon"; Description: "Criar atalho na Área de Trabalho"; Flags: unchecked

[Files]
Source: "..\target\release\korvon-macro.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Korvon Macro"; Filename: "{app}\korvon-macro.exe"
Name: "{autodesktop}\Korvon Macro"; Filename: "{app}\korvon-macro.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\korvon-macro.exe"; Description: "Abrir o Korvon Macro"; Flags: nowait postinstall skipifsilent
