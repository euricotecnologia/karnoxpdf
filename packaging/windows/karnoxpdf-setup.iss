; Script Inno Setup para KarnoxPDF
#define MyAppName "KarnoxPDF"
#define MyAppVersion "1.0.0"
#define MyAppPublisher "KarnoxPDF"
#define MyAppURL "https://github.com/euricotecnologia/karnoxpdf"
#define MyAppExeName "karnoxpdf.exe"
#define MyAppCliExeName "karnoxpdf-cli.exe"

[Setup]
AppId={{7705C3E7-5094-4FDF-AA3D-642E40E1CB53}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}/releases
DefaultDirName=C:\KarnoX\KarnoxPDF
DefaultGroupName={#MyAppName}
AllowNoIcons=yes
LicenseFile=..\..\Copyright_KarnoxPDF.txt
OutputDir=..\..\dist\installer
OutputBaseFilename=karnoxpdf-{#MyAppVersion}-windows-x64-setup
SetupIconFile=..\..\assets\app-icon\karnoxpdf.ico
WizardSmallImageFile=..\..\img\karnox_small.bmp
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
DisableWelcomePage=no
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayIcon={app}\{#MyAppExeName}
ChangesAssociations=yes

[Languages]
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "portuguese"; MessagesFile: "compiler:Languages\Portuguese.isl"
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "french"; MessagesFile: "compiler:Languages\French.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "associatepdf"; Description: "Associar o KarnoxPDF aos arquivos .PDF (Abrir com...)"; GroupDescription: "Integração do Sistema:"

[Files]
Source: "..\..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\target\release\{#MyAppCliExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE-MIT"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE-APACHE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\Copyright_KarnoxPDF.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\img\sobre.bmp"; Flags: dontcopy

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{group}\{#MyAppName} CLI"; Filename: "{app}\{#MyAppCliExeName}"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Registry]
; Registra o executável no App Paths
Root: HKLM; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\{#MyAppExeName}"; ValueType: string; ValueData: "{app}\{#MyAppExeName}"; Flags: uninsdeletekey
Root: HKLM; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\{#MyAppCliExeName}"; ValueType: string; ValueData: "{app}\{#MyAppCliExeName}"; Flags: uninsdeletekey

; Associação de PDF (ProgID)
Root: HKLM; Subkey: "Software\Classes\KarnoxPDF.Document"; ValueType: string; ValueData: "PDF Document"; Flags: uninsdeletekey; Tasks: associatepdf
Root: HKLM; Subkey: "Software\Classes\KarnoxPDF.Document\DefaultIcon"; ValueType: string; ValueData: "{app}\{#MyAppExeName},0"; Flags: uninsdeletekey; Tasks: associatepdf
Root: HKLM; Subkey: "Software\Classes\KarnoxPDF.Document\shell\open\command"; ValueType: string; ValueData: """{app}\{#MyAppExeName}"" ""%1"""; Flags: uninsdeletekey; Tasks: associatepdf
Root: HKLM; Subkey: "Software\Classes\.pdf\OpenWithProgids"; ValueType: string; ValueName: "KarnoxPDF.Document"; ValueData: ""; Flags: uninsdeletevalue; Tasks: associatepdf

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[Code]
var
  WelcomeLogo: TBitmapImage;

procedure InitializeWizard;
var
  LogoPath: String;
begin
  ExtractTemporaryFile('sobre.bmp');
  LogoPath := ExpandConstant('{tmp}\sobre.bmp');

  // Instancia e posiciona rigorosamente dentro da área em branco (à direita da barra azul)
  WelcomeLogo := TBitmapImage.Create(WizardForm);
  WelcomeLogo.Parent := WizardForm.WelcomePage;
  WelcomeLogo.Bitmap.LoadFromFile(LogoPath);
  WelcomeLogo.Stretch := True;
  WelcomeLogo.Left := WizardForm.WelcomeLabel1.Left;
  WelcomeLogo.Top := ScaleY(12);
  WelcomeLogo.Width := ScaleX(260);
  WelcomeLogo.Height := ScaleY(85);

  // Reposiciona os rótulos de boas-vindas abaixo da imagem dentro da área branca
  WizardForm.WelcomeLabel1.Top := WelcomeLogo.Top + WelcomeLogo.Height + ScaleY(12);
  WizardForm.WelcomeLabel2.Top := WizardForm.WelcomeLabel1.Top + WizardForm.WelcomeLabel1.Height + ScaleY(8);
end;
