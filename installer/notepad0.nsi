; Notepad Zero installer
; Copyright (C) 2026 Claude Henchoz
; SPDX-License-Identifier: GPL-3.0-or-later
;
; Build: makensis /DVERSION=1.2.3 installer\notepad0.nsi
;
; Per-user install (no UAC prompt) into %LOCALAPPDATA%\Programs\Notepad Zero.
; One-click: no welcome/license/directory pages; the window closes when done
; and the app is launched. /S installs silently, /D=<dir> overrides the path.

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!define APP      "Notepad Zero"
!define EXE      "notepad0.exe"
!define PROGID   "Notepad0.File"
!define UNINST   "Software\Microsoft\Windows\CurrentVersion\Uninstall\Notepad0"
!define APPPATHS "Software\Microsoft\Windows\CurrentVersion\App Paths\${EXE}"
!define CLASSES  "Software\Classes"

Unicode true
SetCompressor /FINAL lzma
ManifestDPIAware true
ManifestSupportedOS Win7 Win8 Win8.1 Win10
RequestExecutionLevel user
XPStyle on

Name "${APP}"
Caption "${APP} ${VERSION} Setup"
OutFile "notepad0-${VERSION}-setup.exe"
Icon "..\res\installer.ico"
UninstallIcon "..\res\installer.ico"
InstallDir "$LOCALAPPDATA\Programs\${APP}"
BrandingText " "
ShowInstDetails nevershow
ShowUninstDetails nevershow
AutoCloseWindow true

Page instfiles
UninstPage instfiles

; Text extensions that get "Open with > Notepad Zero" directly in the menu.
; Every other file type still lists it under Open with (via *\OpenWithList).
!macro ForEachExt MACRO
  !insertmacro ${MACRO} ".txt"
  !insertmacro ${MACRO} ".log"
  !insertmacro ${MACRO} ".ini"
  !insertmacro ${MACRO} ".cfg"
  !insertmacro ${MACRO} ".conf"
  !insertmacro ${MACRO} ".md"
  !insertmacro ${MACRO} ".csv"
  !insertmacro ${MACRO} ".json"
  !insertmacro ${MACRO} ".xml"
  !insertmacro ${MACRO} ".yml"
  !insertmacro ${MACRO} ".yaml"
  !insertmacro ${MACRO} ".toml"
  !insertmacro ${MACRO} ".bat"
  !insertmacro ${MACRO} ".cmd"
  !insertmacro ${MACRO} ".ps1"
  !insertmacro ${MACRO} ".reg"
  !insertmacro ${MACRO} ".nfo"
!macroend
!macro AddExt EXT
  WriteRegStr HKCU "${CLASSES}\${EXT}\OpenWithProgids" "${PROGID}" ""
  WriteRegStr HKCU "${CLASSES}\Applications\${EXE}\SupportedTypes" "${EXT}" ""
!macroend
!macro RemoveExt EXT
  DeleteRegValue HKCU "${CLASSES}\${EXT}\OpenWithProgids" "${PROGID}"
  DeleteRegKey /ifempty HKCU "${CLASSES}\${EXT}\OpenWithProgids"
  DeleteRegKey /ifempty HKCU "${CLASSES}\${EXT}"
!macroend

Section
  SetOutPath "$INSTDIR"
  File "..\target\release\${EXE}"
  File "..\LICENSE"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  ; Start > Run / Win+R / cmd "start notepad0" resolve the exe via App Paths.
  WriteRegStr HKCU "${APPPATHS}" "" "$INSTDIR\${EXE}"
  WriteRegStr HKCU "${APPPATHS}" "Path" "$INSTDIR"

  ; Application registration (Open with dialog, friendly name, icon).
  WriteRegStr HKCU "${CLASSES}\Applications\${EXE}" "FriendlyAppName" "${APP}"
  WriteRegStr HKCU "${CLASSES}\Applications\${EXE}\DefaultIcon" "" "$INSTDIR\${EXE},0"
  WriteRegStr HKCU "${CLASSES}\Applications\${EXE}\shell\open\command" "" '"$INSTDIR\${EXE}" "%1"'

  ; ProgID referenced by the per-extension OpenWithProgids entries.
  WriteRegStr HKCU "${CLASSES}\${PROGID}" "" "Text Document"
  WriteRegStr HKCU "${CLASSES}\${PROGID}" "FriendlyTypeName" "Text Document"
  WriteRegStr HKCU "${CLASSES}\${PROGID}\DefaultIcon" "" "$INSTDIR\${EXE},0"
  WriteRegStr HKCU "${CLASSES}\${PROGID}\shell\open\command" "" '"$INSTDIR\${EXE}" "%1"'
  !insertmacro ForEachExt AddExt

  ; All files: "Open with > Notepad Zero" (Windows 11 top-level menu) and a
  ; classic "Edit with Notepad Zero" verb (under "Show more options").
  WriteRegStr HKCU "${CLASSES}\*\OpenWithList\${EXE}" "" ""
  WriteRegStr HKCU "${CLASSES}\*\shell\Notepad0" "" "Edit with ${APP}"
  WriteRegStr HKCU "${CLASSES}\*\shell\Notepad0" "Icon" "$INSTDIR\${EXE},0"
  WriteRegStr HKCU "${CLASSES}\*\shell\Notepad0\command" "" '"$INSTDIR\${EXE}" "%1"'

  CreateShortcut "$SMPROGRAMS\${APP}.lnk" "$INSTDIR\${EXE}"

  ; Apps & features entry.
  WriteRegStr HKCU "${UNINST}" "DisplayName" "${APP}"
  WriteRegStr HKCU "${UNINST}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST}" "Publisher" "Claude Henchoz"
  WriteRegStr HKCU "${UNINST}" "DisplayIcon" "$INSTDIR\${EXE},0"
  WriteRegStr HKCU "${UNINST}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINST}" "URLInfoAbout" "https://github.com/claudehenchoz/notepad0"
  WriteRegStr HKCU "${UNINST}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${UNINST}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKCU "${UNINST}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST}" "NoRepair" 1
  WriteRegDWORD HKCU "${UNINST}" "EstimatedSize" 300

  ; SHCNE_ASSOCCHANGED: refresh Explorer's association cache.
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'

  IfSilent +2
    Exec '"$INSTDIR\${EXE}"'
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\${EXE}"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\${APP}.lnk"

  DeleteRegKey HKCU "${APPPATHS}"
  DeleteRegKey HKCU "${CLASSES}\Applications\${EXE}"
  DeleteRegKey HKCU "${CLASSES}\${PROGID}"
  !insertmacro ForEachExt RemoveExt
  DeleteRegKey HKCU "${CLASSES}\*\OpenWithList\${EXE}"
  DeleteRegKey HKCU "${CLASSES}\*\shell\Notepad0"
  DeleteRegKey HKCU "${UNINST}"
  DeleteRegKey HKCU "Software\Notepad0"

  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
SectionEnd
