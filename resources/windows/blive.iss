[Setup]
AppId=BLive
AppName=BLive
AppPublisher=starknt
AppPublisherURL=https://github.com/starknt
AppSupportURL=https://github.com/starknt/BLive/issues
AppUpdatesURL=https://github.com/starknt/BLive/releases
AppVersion={#AppVersion}
DefaultDirName={autopf}\BLive
DefaultGroupName=BLive
UninstallDisplayIcon={app}\blive.exe
Compression=lzma
SolidCompression=yes
SetupIconFile=resources\windows\icon.ico
ChangesEnvironment=yes
ChangesAssociations=yes
OutputBaseFilename=BLiveInstaller
WizardStyle=modern
CloseApplications=force

ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest

[Languages]
Name: "simplifiedChinese"; MessagesFile: "resources\windows\messages\Default.zh-cn.isl,resources\windows\messages\zh-cn.isl";

[Tasks]
Name: "desktopicon"; Description: "创建桌面快捷方式"; GroupDescription: "附加任务："

[Files]
Source: "target\release\blive.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "resources\windows\icon.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "resources\sidecar\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\BLive"; Filename: "{app}\blive.exe"
Name: "{autodesktop}\BLive"; Filename: "{app}\blive.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\blive.exe"; Description: "运行 BLive"; Flags: nowait postinstall skipifsilent

[Code]
function WizardNotSilent(): Boolean;
begin
  Result := not WizardSilent();
end;

function IsWindows11OrLater(): Boolean;
begin
  Result := (GetWindowsVersion >= $0A0055F0);
end;

// https://stackoverflow.com/a/23838239/261019
procedure Explode(var Dest: TArrayOfString; Text: String; Separator: String);
var
  i, p: Integer;
begin
  i := 0;
  repeat
    SetArrayLength(Dest, i+1);
    p := Pos(Separator,Text);
    if p > 0 then begin
      Dest[i] := Copy(Text, 1, p-1);
      Text := Copy(Text, p + Length(Separator), Length(Text));
      i := i + 1;
    end else begin
      Dest[i] := Text;
      Text := '';
    end;
  until Length(Text)=0;
end;

function NeedsAddToPath(path: string): boolean;
var
  OrigPath: string;
begin
  if not RegQueryStringValue(HKCU, 'Environment', 'Path', OrigPath)
  then begin
    Result := True;
    exit;
  end;
  Result := Pos(';' + path + ';', ';' + OrigPath + ';') = 0;
end;

function AddToPath(path: string): string;
var
  OrigPath: string;
begin
  RegQueryStringValue(HKCU, 'Environment', 'Path', OrigPath)

  if (Length(OrigPath) > 0) and (OrigPath[Length(OrigPath)] = ';') then
    Result := OrigPath + path
  else
    Result := OrigPath + ';' + path
end;
