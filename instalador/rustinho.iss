; Instalador do Rustinho para Windows 10 e 11 (64 bits), feito com Inno Setup 6.
; A montagem automática (GitHub Actions) passa a versão e a pasta com os
; arquivos: ISCC /DVersao=0.8.0 /DOrigem=..\dist rustinho.iss

#ifndef Versao
  #define Versao "0.0.0"
#endif
#ifndef Origem
  #define Origem "..\dist"
#endif

[Setup]
AppId={{6E2C4B1A-7F3D-4C8E-9A51-2B7D0E3F9C14}
AppName=Rustinho
AppVersion={#Versao}
AppVerName=Rustinho {#Versao}
AppPublisher=Rustinho (código livre, licença MIT)
AppPublisherURL=https://github.com/nrenanjo/Rustinho
AppSupportURL=https://github.com/nrenanjo/Rustinho
DefaultDirName={autopf}\Rustinho
DefaultGroupName=Rustinho
DisableProgramGroupPage=yes
UninstallDisplayName=Rustinho
UninstallDisplayIcon={app}\rustinho.exe
SetupIconFile=rustinho.ico
WizardStyle=modern
WizardSizePercent=110
OutputDir=..\saida
OutputBaseFilename=Rustinho-{#Versao}-Windows-instalador
Compression=lzma2/max
SolidCompression=yes
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; Windows 10 versão 1809 ou mais novo (exigência do Qt 6).
MinVersion=10.0.17763
CloseApplications=yes
RestartApplications=no
LicenseFile=..\LICENSE

[Languages]
Name: "pt"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[Messages]
pt.ConfirmUninstall=Remover agora o %1 deste computador?
pt.WelcomeLabel2=Isto vai instalar o [name/ver] neste computador.%n%nDepois é só abrir o Rustinho pelo menu Iniciar.

[Tasks]
Name: "atalho"; Description: "Criar um atalho na área de trabalho"; GroupDescription: "Atalhos:"

[Files]
Source: "{#Origem}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "rustinho.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Rustinho"; Filename: "{app}\rustinho.exe"; IconFilename: "{app}\rustinho.ico"
Name: "{group}\Desinstalar o Rustinho"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Rustinho"; Filename: "{app}\rustinho.exe"; IconFilename: "{app}\rustinho.ico"; Tasks: atalho

[Run]
; Libera o Rustinho no firewall do Windows: os caixas da loja se acham e
; trocam dados pelo Wi-Fi (portas 47810 e 47811).
Filename: "{sys}\netsh.exe"; Parameters: "advfirewall firewall delete rule name=""Rustinho"""; Flags: runhidden waituntilterminated
Filename: "{sys}\netsh.exe"; Parameters: "advfirewall firewall add rule name=""Rustinho"" dir=in action=allow program=""{app}\rustinho.exe"" enable=yes profile=private,domain"; Flags: runhidden waituntilterminated
Filename: "{app}\rustinho.exe"; Description: "Abrir o Rustinho agora"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "{sys}\netsh.exe"; Parameters: "advfirewall firewall delete rule name=""Rustinho"""; Flags: runhidden waituntilterminated; RunOnceId: "firewall"

[Code]
var
  ApagarDados: Boolean;

// Antes de desinstalar: pergunta, bem à vista, se apaga também os dados da loja.
function InitializeUninstall(): Boolean;
var
  Janela: TSetupForm;
  Texto, Aviso: TNewStaticText;
  Marca: TNewCheckBox;
  Continuar, Cancelar: TNewButton;
begin
  Result := True;
  ApagarDados := False;
  if UninstallSilent then
    Exit;

  Janela := CreateCustomForm(ScaleX(480), ScaleY(250), False, False);
  try
    Janela.Caption := 'Desinstalar o Rustinho';

    Texto := TNewStaticText.Create(Janela);
    Texto.Parent := Janela;
    Texto.AutoSize := False;
    Texto.WordWrap := True;
    Texto.Left := ScaleX(20);
    Texto.Top := ScaleY(18);
    Texto.Width := Janela.ClientWidth - ScaleX(40);
    Texto.Height := ScaleY(48);
    Texto.Caption := 'O programa Rustinho vai ser removido deste computador.' + #13#10 +
      'Os dados da loja ficam guardados, a não ser que você marque abaixo.';

    Marca := TNewCheckBox.Create(Janela);
    Marca.Parent := Janela;
    Marca.Left := ScaleX(20);
    Marca.Top := ScaleY(76);
    Marca.Width := Janela.ClientWidth - ScaleX(40);
    Marca.Height := ScaleY(24);
    Marca.Font.Style := [fsBold];
    Marca.Font.Size := 11;
    Marca.Caption := 'Apagar também os dados da loja';
    Marca.Checked := False;

    Aviso := TNewStaticText.Create(Janela);
    Aviso.Parent := Janela;
    Aviso.AutoSize := False;
    Aviso.WordWrap := True;
    Aviso.Left := ScaleX(40);
    Aviso.Top := ScaleY(104);
    Aviso.Width := Janela.ClientWidth - ScaleX(60);
    Aviso.Height := ScaleY(70);
    Aviso.Font.Color := $003A4494;
    Aviso.Font.Style := [fsBold];
    Aviso.Caption := 'ATENÇÃO: apaga vendas, peças, clientes, caixa e os backups deste computador, para sempre. ' +
      'Se quiser guardar, faça antes um backup (em Configurações, Backup) e copie para um pen drive.';

    Continuar := TNewButton.Create(Janela);
    Continuar.Parent := Janela;
    Continuar.Caption := 'Desinstalar';
    Continuar.Width := ScaleX(110);
    Continuar.Height := ScaleY(28);
    Continuar.Left := Janela.ClientWidth - ScaleX(250);
    Continuar.Top := Janela.ClientHeight - ScaleY(46);
    Continuar.ModalResult := mrOk;
    Continuar.Default := True;

    Cancelar := TNewButton.Create(Janela);
    Cancelar.Parent := Janela;
    Cancelar.Caption := 'Cancelar';
    Cancelar.Width := ScaleX(110);
    Cancelar.Height := ScaleY(28);
    Cancelar.Left := Janela.ClientWidth - ScaleX(130);
    Cancelar.Top := Janela.ClientHeight - ScaleY(46);
    Cancelar.ModalResult := mrCancel;
    Cancelar.Cancel := True;

    Result := Janela.ShowModal() = mrOk;
    if Result then
      ApagarDados := Marca.Checked;
  finally
    Janela.Free;
  end;

  if Result and ApagarDados then
    Result := MsgBox('Tem certeza? Os dados da loja deste computador vão ser apagados e não dá para desfazer.',
      mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if (CurUninstallStep = usPostUninstall) and ApagarDados then
    DelTree(ExpandConstant('{localappdata}\Rustinho'), True, True, True);
end;
