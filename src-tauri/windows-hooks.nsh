!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr HKCU "Software\DalZip" "Executable" "$INSTDIR\DalZip.exe"
  WriteRegStr HKCU "Software\Classes\CLSID\{627ED496-2F22-459E-A0EC-D2459E46C191}\InprocServer32" "" "$INSTDIR\shell\DalZipShell-${VERSION}.dll"
  WriteRegStr HKCU "Software\Classes\CLSID\{627ED496-2F22-459E-A0EC-D2459E46C191}\InprocServer32" "ThreadingModel" "Apartment"
  WriteRegStr HKCU "Software\Classes\*\shell\DalZip" "ExplorerCommandHandler" "{627ED496-2F22-459E-A0EC-D2459E46C191}"
  WriteRegStr HKCU "Software\Classes\*\shell\DalZip" "MultiSelectModel" "Player"
  WriteRegStr HKCU "Software\Classes\Directory\shell\DalZip" "ExplorerCommandHandler" "{627ED496-2F22-459E-A0EC-D2459E46C191}"
  WriteRegStr HKCU "Software\Classes\Directory\shell\DalZip" "MultiSelectModel" "Player"
  WriteRegStr HKCU "Software\Classes\CLSID\{627ED496-2F22-459E-A0EC-D2459E46C192}\InprocServer32" "" "$INSTDIR\shell\DalZipShell-${VERSION}.dll"
  WriteRegStr HKCU "Software\Classes\CLSID\{627ED496-2F22-459E-A0EC-D2459E46C192}\InprocServer32" "ThreadingModel" "Apartment"
  WriteRegStr HKCU "Software\Classes\*\shell\DalZipExtract" "ExplorerCommandHandler" "{627ED496-2F22-459E-A0EC-D2459E46C192}"
  WriteRegStr HKCU "Software\Classes\*\shell\DalZipExtract" "MultiSelectModel" "Player"
!macroend
!macro NSIS_HOOK_PREUNINSTALL
  DeleteRegKey HKCU "Software\Classes\*\shell\DalZipExtract"
  DeleteRegKey HKCU "Software\Classes\CLSID\{627ED496-2F22-459E-A0EC-D2459E46C192}"
  DeleteRegKey HKCU "Software\Classes\*\shell\DalZip"
  DeleteRegKey HKCU "Software\Classes\Directory\shell\DalZip"
  DeleteRegKey HKCU "Software\Classes\CLSID\{627ED496-2F22-459E-A0EC-D2459E46C191}"
  DeleteRegValue HKCU "Software\RegisteredApplications" "DalZip"
  DeleteRegKey HKCU "Software\DalZip"
!macroend
