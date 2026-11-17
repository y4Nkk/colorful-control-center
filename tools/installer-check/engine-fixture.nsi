Unicode true
!include LogicLib.nsh
!include FileFunc.nsh
!include Win\RestartManager.nsh
!define /ifndef ERROR_MORE_DATA 234
!macro CheckIfAppIsRunning executablePath productName
  !error "The project must replace the default force-shutdown check."
!macroend
!include "${HOOKS}"
Name "CCC isolated NSIS fixture"
OutFile "${OUTPUT}"
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "FileVersion" "${VERSION}.0"
VIAddVersionKey "FileDescription" "Isolated installer fixture"
VIAddVersionKey "LegalCopyright" "Test fixture"
RequestExecutionLevel user
SilentInstall silent
SetCompressor /SOLID lzma
Var TestRoot
Var Result

!macro RequireFixtureRoot
  ReadEnvStr $TestRoot "CCC_INSTALLER_CHECK_ROOT"
  ${If} $TestRoot == ""
    SetErrorLevel 90
    Abort
  ${EndIf}
  GetFullPathName $TestRoot "$TestRoot"
  System::Call 'kernel32::GetFullPathNameW(w "$INSTDIR", i ${NSIS_MAX_STRLEN}, w .r4, p 0) i .r5'
  ${If} $5 = 0
  ${OrIf} $5 >= ${NSIS_MAX_STRLEN}
    SetErrorLevel 92
    Abort
  ${EndIf}
  StrCpy $INSTDIR $4
  StrLen $0 "$TestRoot\"
  StrCpy $1 "$INSTDIR\" $0
  ${If} $1 != "$TestRoot\"
    FileOpen $3 "$TestRoot\nsis-path-error.log" w
    FileWrite $3 "ROOT=$TestRoot$\r$\nDIR=$INSTDIR$\r$\nPREFIX=$1$\r$\nLENGTH=$0$\r$\nCMD=$CMDLINE"
    FileClose $3
    SetErrorLevel 92
    Abort
  ${EndIf}
!macroend

Function .onInit
  !insertmacro RequireFixtureRoot
FunctionEnd

Section
  ReadEnvStr $Result "CCC_INSTALLER_CHECK_GATE"
  ${If} $Result == "wait"
    StrCpy $2 0
    ${DoWhile} $2 < 2250
      IfFileExists "$TestRoot\release-engine" ready
      Sleep 20
      IntOp $2 $2 + 1
    ${Loop}
    SetErrorLevel 93
    Quit
  ${EndIf}
  ready:
  !insertmacro CheckIfAppIsRunning "$INSTDIR\${APPLICATION_NAME}" "CCC fixture"
  SetOutPath $INSTDIR
  FileOpen $0 "$INSTDIR\partial-file.txt" w
  FileWrite $0 "isolated NSIS output"
  FileClose $0
  ReadEnvStr $Result "CCC_INSTALLER_CHECK_RESULT"
  ${If} $Result == "failure"
    SetErrorLevel 23
    Quit
  ${EndIf}
  ${If} $Result == "missing"
    SetErrorLevel 0
    Quit
  ${EndIf}
  SetOverwrite on
  File "/oname=${APPLICATION_NAME}" "${APPLICATION}"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  ${If} $Result == "tampered"
    FileOpen $0 "$INSTDIR\${APPLICATION_NAME}" w
    FileWrite $0 "changed bytes"
    FileClose $0
  ${EndIf}
  SetErrorLevel 0
SectionEnd

Function un.onInit
  !insertmacro RequireFixtureRoot
FunctionEnd

Section "Uninstall"
  !insertmacro CheckIfAppIsRunning "$INSTDIR\${APPLICATION_NAME}" "CCC fixture"
  Delete "$INSTDIR\${APPLICATION_NAME}"
  Delete "$INSTDIR\partial-file.txt"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
SectionEnd
